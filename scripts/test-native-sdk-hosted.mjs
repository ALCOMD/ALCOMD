import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { once } from "node:events";
import { chmod, copyFile, lstat, mkdir, mkdtemp, readFile, realpath, readdir, rm, rmdir, unlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, dirname, isAbsolute, join } from "node:path";
import { createInterface } from "node:readline";
import { setTimeout as delay } from "node:timers/promises";
import { fileURLToPath } from "node:url";

const workspace = await realpath(fileURLToPath(new URL("../", import.meta.url)));
const { platform } = process;
const windows = platform === "win32";
const macos = platform === "darwin";
const npmPath = process.env.npm_execpath;
const runnerWorkspace = process.env.GITHUB_WORKSPACE;
if (process.env.GITHUB_ACTIONS !== "true" || process.env.RUNNER_ENVIRONMENT !== "github-hosted"
    || !runnerWorkspace || !samePath(await realpath(runnerWorkspace), workspace)
    || !npmPath || !isAbsolute(npmPath)) {
    throw new Error("Run the native SDK staging test through npm on a GitHub-hosted runner.");
}

const suffix = windows ? ".exe" : "";
const node = process.execPath;
const localAppData = process.env.LOCALAPPDATA;
if (windows && (!localAppData || !isAbsolute(localAppData))) {
    throw new Error("Windows LocalAppData is unavailable.");
}
const stageParent = await realpath(windows ? localAppData : macos ? "/private/tmp" : tmpdir());
const stage = await mkdtemp(join(stageParent, "ac-m10-host-"));
const runtime = join(stage, "runtime");
const external = join(stage, "consumer");
const profileBase = windows ? stageParent : macos ? join(stage, "home") : stage;
const environment = { ...process.env };
delete environment.ALCOMD_MCP_CLIENT_CREDENTIAL;
delete environment.NODE_OPTIONS;
if (macos) {
    environment.TMPDIR = stage;
} else if (!windows) {
    environment.XDG_RUNTIME_DIR = stage;
    environment.XDG_DATA_HOME = join(stage, "xdg-data");
}
// Core resolves its normal macOS paths from HOME. Keep npm, Cargo and Rustup
// on the runner's original HOME so the build still uses its installed caches.
const fixtureEnvironment = { ...environment, ...(macos ? { HOME: profileBase } : {}) };
const target = join(workspace, "target");
const cargo = windows ? "cargo.exe" : "cargo";
const bridge = join(target, "debug", `alcomd-sdk-bridge${suffix}`);
const daemon = join(runtime, `alcomd${suffix}`);
const packageEntry = join(external, "node_modules", "@alcomd", "sdk", "dist", "node.js");
const sourceExample = join(workspace, "sdk", "rust", "examples", "native-client");
const rustExample = join(target, "debug", `alcomd-native-sdk-example${suffix}`);
let ownedData;
const createdProfileParents = [];
let canClean = true;
let repositoryProxy;
const startedAt = performance.now();

function phase(label) {
    process.stdout.write(`Native SDK fixture +${Math.round(performance.now() - startedAt)} ms | ${label}\n`);
}

async function isolateRepositoryNetwork() {
    // The default System proxy remains active. Reject external repository refreshes
    // immediately so their network deadlines cannot extend the Core idle check.
    // Run the rejecting listener separately so synchronous probes cannot block it.
    const child = start(node, ["--input-type=module", "--eval", `
        import { createServer } from "node:net";
        const listener = createServer((socket) => socket.destroy());
        listener.listen(0, "127.0.0.1", () => console.log(listener.address().port));
        process.stdin.on("end", () => listener.close());
        process.stdin.resume();
    `], ["pipe", "pipe", "inherit"]);
    const exited = observeExit(child);
    repositoryProxy = { child, exited };
    const port = Number(await readReadyLine(child, exited, 5_000, "Repository fixture proxy"));
    assert(Number.isInteger(port) && port > 0 && port <= 65_535, "Repository fixture proxy must use a valid loopback port.");
    for (const key of Object.keys(fixtureEnvironment)) {
        if (["ALL_PROXY", "HTTPS_PROXY", "HTTP_PROXY", "NO_PROXY"].includes(key.toUpperCase())) {
            delete fixtureEnvironment[key];
        }
    }
    const proxy = `http://127.0.0.1:${port}`;
    fixtureEnvironment.HTTPS_PROXY = proxy;
    fixtureEnvironment.HTTP_PROXY = proxy;
}

function samePath(first, second) {
    return windows ? first.toLowerCase() === second.toLowerCase() : first === second;
}

async function metadata(path) {
    try {
        return await lstat(path);
    } catch (error) {
        if (error.code === "ENOENT") return undefined;
        throw error;
    }
}

async function verifyDirectory(path, parent) {
    const info = await lstat(path);
    const actual = await realpath(path);
    assert(info.isDirectory() && !info.isSymbolicLink());
    assert(samePath(actual, path) && samePath(dirname(actual), parent));
}

function run(command, args, options = {}) {
    const result = spawnSync(command, args, {
        cwd: options.cwd ?? workspace,
        env: options.env ?? environment,
        windowsHide: true,
        stdio: options.capture ? "pipe" : "inherit",
        encoding: "utf8",
        timeout: options.timeout ?? 45_000
    });
    if (result.error) throw result.error;
    return result;
}

function success(result, step) {
    assert.equal(result.status, 0, `${step} failed: ${result.stderr ?? ""}`);
    return result.stdout;
}

function rustProbe(mode) {
    return run(rustExample, [mode], { cwd: external, capture: true, env: fixtureEnvironment });
}

function nodeProbe(mode, capture = false) {
    return run(node, ["probe.mjs", mode], { cwd: external, capture, env: fixtureEnvironment });
}

function probeResult(output) {
    const probe = JSON.parse(output);
    assert.equal(probe.hello.rpcVersion, 1);
    assert(Number.isInteger(probe.hello.dataSchema) && probe.hello.dataSchema > 0);
    assert(Number.isInteger(probe.hello.configSchema) && probe.hello.configSchema > 0);
    assert.equal(probe.status.state, "ready");
    assert.equal(probe.projectCount, 0);
    assert.equal(typeof probe.instanceId, "string");
    assert(probe.instanceId.length > 0);
    return probe;
}

function assertStopped() {
    const result = rustProbe("probe-running");
    assert.equal(result.status, 1, "Expected no accessible Core on the fixture endpoint.");
    assert.match(result.stderr, /did not become ready within five seconds/);
}

async function assertIdleExit() {
    // Leave the disconnected grace interval alone before the no-start probe.
    await delay(2_000);
    assertStopped();
}

function start(command, args, stdio, cwd = external) {
    return spawn(command, args, { cwd, env: fixtureEnvironment, stdio, windowsHide: true });
}

async function within(promise, timeout, label) {
    const timer = new AbortController();
    try {
        return await Promise.race([promise, delay(timeout, undefined, { signal: timer.signal }).then(() => {
            throw new Error(`${label} did not finish within ${timeout} ms.`);
        })]);
    } finally {
        timer.abort();
    }
}

function observeExit(child) {
    const exited = once(child, "exit");
    // A spawn error may arrive before the caller reaches the exit check.
    exited.catch(() => {});
    return exited;
}

async function expectExit(exited, timeout, label) {
    const [code, signal] = await within(exited, timeout, label);
    assert.equal(code, 0, `${label} exited with code ${code} and signal ${signal}.`);
}

async function stopChild(child, exited, label) {
    if (child.pid !== undefined && child.exitCode === null && child.signalCode === null) {
        child.kill();
        await within(exited, 5_000, label).catch(() => {});
    }
}

async function readReadyLine(child, exited, timeout, label) {
    const ready = createInterface({ input: child.stdout });
    try {
        const output = Promise.race([
            once(ready, "line"),
            once(ready, "close").then(() => { throw new Error(`${label} closed stdout before reporting readiness.`); })
        ]);
        const [line] = await within(Promise.race([
            output,
            exited.then(([code, signal]) => {
                if (code !== 0) throw new Error(`${label} exited before reporting readiness (code ${code}, signal ${signal}).`);
                return output;
            })
        ]), timeout, `${label} readiness`);
        return line;
    } finally {
        ready.close();
        child.stdout.destroy();
    }
}

async function openNodeConsumer(mode) {
    const child = start(node, ["probe.mjs", mode], ["pipe", "pipe", "inherit"]);
    const exited = observeExit(child);
    try {
        const line = await readReadyLine(child, exited, 35_000, "Node cold consumer");
        return { child, exited, result: probeResult(line) };
    } catch (error) {
        await stopChild(child, exited, "Stop failed Node cold consumer");
        throw error;
    }
}

async function connectColdRust() {
    const child = start(rustExample, ["probe"], ["ignore", "pipe", "inherit"]);
    const exited = observeExit(child);
    try {
        // The Core started by the Rust consumer can outlive the probe. Read its
        // single JSON report and wait for the consumer process, not pipe EOF.
        const [line] = await Promise.all([
            readReadyLine(child, exited, 35_000, "Rust cold consumer"),
            expectExit(exited, 45_000, "Rust cold consumer")
        ]);
        return probeResult(line);
    } finally {
        await stopChild(child, exited, "Stop Rust cold consumer");
    }
}

async function prepareProfile() {
    if (macos) {
        await mkdir(profileBase, { mode: 0o700 });
        await verifyDirectory(profileBase, stage);
    }
    const registration = success(rustProbe("registration-path"), "Locate Core registration").trim();
    const expected = windows ? join(profileBase, "ALCOMD", "data", "core-installation-v1")
        : macos ? join(profileBase, "Library", "Application Support", "ALCOMD", "core-installation-v1")
        : join(environment.XDG_DATA_HOME, "alcomd", "core-installation-v1");
    assert(samePath(registration, expected), "The registration resolves outside the expected fixture data location.");
    assertStopped();
    const data = dirname(registration);
    assert.equal(await metadata(data), undefined, "Refusing to start with existing Core data or registration.");
    const components = windows ? ["ALCOMD"] : macos ? ["Library", "Application Support"] : ["xdg-data"];
    let parent = await realpath(profileBase);
    assert(samePath(join(parent, ...components), dirname(data)), "The registration's parent resolves outside the fixture.");
    for (const component of components) {
        const directory = join(parent, component);
        if (!await metadata(directory)) {
            await mkdir(directory, { mode: 0o700 });
            if (windows) createdProfileParents.push(directory);
        }
        await verifyDirectory(directory, parent);
        parent = directory;
    }
    // Claim the previously absent directory with an exclusive creation, so an
    // existing account's Core data can never be mistaken for our fixture.
    await mkdir(data, { mode: 0o700 });
    ownedData = data;
    return registration;
}

try {
    await mkdir(runtime, { mode: 0o700 });
    await mkdir(external, { mode: 0o700 });
    const buildEnv = { ...environment, CARGO_TARGET_DIR: target };
    success(run(cargo, ["build", "--locked", "--offline", "--package", "alcomd", "--package", "alcomd-client",
        "--bin", "alcomd", "--bin", "alcomd-sdk-bridge"], { env: buildEnv, timeout: 600_000 }), "Build native SDK and Core");
    await copyFile(join(target, "debug", basename(daemon)), daemon);
    if (!windows) await chmod(daemon, 0o700);

    const archive = join(stage, "archive");
    success(run(node, [npmPath, "run", "pack:local", "--workspace", "@alcomd/sdk", "--", "--bridge", bridge,
        "--output", archive], { env: buildEnv, timeout: 120_000 }), "Build external npm archive");
    const tarballs = (await readdir(archive)).filter((name) => name.endsWith(".tgz"));
    assert.equal(tarballs.length, 1);
    await writeFile(join(external, "package.json"), '{"private":true,"type":"module"}\n');
    success(run(node, [npmPath, "install", "--prefix", external, "--offline", "--ignore-scripts", "--no-audit", "--no-fund",
        join(archive, tarballs[0])], { cwd: external, timeout: 120_000 }), "Install packed npm SDK");
    assert.equal((await lstat(packageEntry)).isFile(), true);
    const templates = new URL("../packages/alcomd-sdk/tests/hosted-consumer/", import.meta.url);
    await copyFile(new URL("probe.mjs", templates), join(external, "probe.mjs"));
    await copyFile(new URL("types.ts", templates), join(external, "types.ts"));
    success(run(node, [join(workspace, "node_modules", "typescript", "bin", "tsc"), "--ignoreConfig", "--noEmit",
        "--strict", "--target", "es2022", "--module", "nodenext", "types.ts"], { cwd: external }), "Typecheck installed SDK");

    const rustSource = join(external, "src");
    await mkdir(rustSource);
    await copyFile(join(sourceExample, "src", "main.rs"), join(rustSource, "main.rs"));
    const manifest = (await readFile(join(sourceExample, "Cargo.toml"), "utf8")).replace(
        /"\.\.\/\.\.\/\.\.\/\.\.\/crates\/([^"]+)"/g,
        (_all, dependency) => JSON.stringify(join(workspace, "crates", dependency).replaceAll("\\", "/"))
    );
    assert(!manifest.includes("../../../../crates/"));
    await writeFile(join(external, "Cargo.toml"), manifest);
    await copyFile(join(workspace, "Cargo.lock"), join(external, "Cargo.lock"));
    await copyFile(join(workspace, "rust-toolchain.toml"), join(external, "rust-toolchain.toml"));
    success(run(cargo, ["build", "--offline"], {
        cwd: external, env: buildEnv, timeout: 600_000
    }), "Build external Rust example");

    phase("Check that the fixture endpoint and profile are unused");
    await isolateRepositoryNetwork();
    const registration = await prepareProfile();
    canClean = false;
    phase("Start staged Core before registration and probe readiness");
    const initial = start(daemon, [], ["ignore", "ignore", "inherit"], runtime);
    const initialExit = observeExit(initial);
    try {
        const initialized = probeResult(success(rustProbe("probe-running"), "Initialize staged Core"));
        phase("Wait for initialized Core to exit after disconnecting");
        await expectExit(initialExit, 45_000, "Initialized Core idle shutdown");
        assert((await metadata(join(ownedData, "state.db")))?.isFile(), "Core must initialize the fixture state before registration.");
        assert.equal(await metadata(registration), undefined);
        phase("Verify missing and stale registration cannot start Core");
        success(nodeProbe("unavailable"), "Node rejects a missing registration");

        const missing = join(stage, "missing-runtime");
        await writeFile(registration, `${missing}\n`, { flag: "wx", mode: 0o600 });
        assert.notEqual(rustProbe("probe").status, 0, "Rust accepted a stale registration.");
        success(nodeProbe("unavailable"), "Node rejects stale registration");
        assertStopped();
        await unlink(registration);
        await writeFile(registration, `${runtime}\n`, { flag: "wx", mode: 0o600 });

        phase("Connect Node and Rust in parallel from a cold start");
        const nodeReady = openNodeConsumer("hold");
        const parallelRust = connectColdRust();
        parallelRust.catch(() => {});
        const held = await nodeReady.catch(async (error) => {
            await parallelRust.catch(() => {});
            throw error;
        });
        try {
            const parallel = await parallelRust;
            assert.equal(parallel.instanceId, held.result.instanceId);
            assert.notEqual(parallel.instanceId, initialized.instanceId);
            phase("Attach Rust to the running Core during the Node event subscription");
            assert.equal(probeResult(success(rustProbe("probe-running"), "Running Core connection")).instanceId, parallel.instanceId);
        } finally {
            phase("Release Node event subscription and wait for the consumer to exit");
            held.child.stdin.end();
            try {
                await expectExit(held.exited, 25_000, "Node cold consumer shutdown");
            } finally {
                await stopChild(held.child, held.exited, "Stop Node cold consumer");
            }
        }
        phase("Verify Core exited after both consumers disconnected");
        await assertIdleExit();

        phase("Restart Core from the installed Node package and verify it idles again");
        const restart = probeResult(success(nodeProbe("probe", true), "Restart from Node"));
        assert.notEqual(restart.instanceId, held.result.instanceId);
        await assertIdleExit();
        canClean = true;
    } finally {
        await stopChild(initial, initialExit, "Stop initial Core fixture");
        if (!canClean) {
            phase("Verify Core has stopped before cleaning its fixture data");
            try { await assertIdleExit(); canClean = true; } catch { /* Keep state while Core may own it. */ }
        }
    }

    phase("Exercise packaged consumers against a separate isolated Core");
    success(run(cargo, ["test", "--locked", "--offline", "--package", "alcomd-testing", "--test", "m10_native_sdk"], {
        env: { ...buildEnv, ALCOMD_M10_NODE_PACKAGE_CONSUMER: external }, timeout: 600_000
    }), "Cross-language workflows on isolated Core");
    const verifiedArchives = join(target, "m10-sdk");
    await mkdir(verifiedArchives, { recursive: true });
    await copyFile(join(archive, tarballs[0]), join(verifiedArchives, tarballs[0]));
    process.stdout.write("Native SDK archives, external consumers, installation startup and idle restart passed.\n");
} finally {
    if (repositoryProxy) await stopChild(repositoryProxy.child, repositoryProxy.exited, "Stop Repository fixture proxy");
    if (canClean) {
        if (windows && ownedData) {
            await verifyDirectory(ownedData, dirname(ownedData));
            await rm(ownedData, { recursive: true, force: true });
        }
        for (const parent of createdProfileParents.reverse()) {
            await verifyDirectory(parent, dirname(parent));
            await rmdir(parent);
        }
        await verifyDirectory(stage, stageParent);
        await rm(stage, { recursive: true, force: true });
    }
}
