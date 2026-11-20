import { execFileSync } from "node:child_process";
import { chmod, copyFile, mkdir, mkdtemp, readFile, readdir, rm, stat, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { isAbsolute, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";

const { values } = parseArgs({
    options: { bridge: { type: "string" }, output: { type: "string" } }
});
const { platform, arch } = process;
const target = `${platform}-${arch}`;
if (!["win32-x64", "linux-x64", "darwin-arm64"].includes(target)) {
    throw new Error("Build on a supported SDK package target.");
}
if (!values.bridge || !(await stat(resolve(values.bridge))).isFile()) {
    throw new Error("Pass the compiled alcomd-sdk-bridge executable using --bridge.");
}
const npmPath = process.env.npm_execpath;
if (!npmPath || !isAbsolute(npmPath)) {
    throw new Error("Run this script through npm run pack:local.");
}

const workspace = new URL("../../../", import.meta.url);
const output = resolve(values.output ?? fileURLToPath(new URL("target/m10-sdk", workspace)));
await mkdir(output, { recursive: true });
const staging = await mkdtemp(join(tmpdir(), "alcomd-sdk-"));
try {
    const packageRoot = join(staging, "package");
    const workspaceManifest = JSON.parse(await readFile(new URL("../package.json", import.meta.url), "utf8"));
    const manifest = {
        name: workspaceManifest.name,
        version: workspaceManifest.version,
        description: workspaceManifest.description,
        license: workspaceManifest.license,
        private: true,
        type: "module",
        engines: { node: ">=24 <25" },
        os: [platform],
        cpu: [arch],
        exports: {
            ".": { types: "./dist/index.d.ts", import: "./dist/index.js" },
            "./node": { types: "./dist/node.d.ts", node: { import: "./dist/node.js" } }
        }
    };
    await mkdir(join(packageRoot, "bin"), { recursive: true });
    const executable = join(packageRoot, "bin", platform === "win32" ? "alcomd-sdk-bridge.exe" : "alcomd-sdk-bridge");
    await copyFile(resolve(values.bridge), executable);
    if (platform !== "win32") await chmod(executable, 0o755);
    const sources = await readdir(new URL("../src", import.meta.url), { withFileTypes: true });
    const outputs = sources
        .filter((source) => source.isFile() && source.name.endsWith(".ts") && !source.name.endsWith(".d.ts"))
        .flatMap(({ name }) => [`${name.slice(0, -3)}.js`, `${name.slice(0, -3)}.d.ts`]);
    outputs.push("node-runtime.mjs", "node-runtime.d.mts");
    const destination = join(packageRoot, "dist");
    await mkdir(destination);
    for (const name of outputs.sort()) {
        await copyFile(new URL(`../dist/${name}`, import.meta.url), join(destination, name));
    }
    await copyFile(new URL("LICENSE", workspace), join(packageRoot, "LICENSE"));
    await copyFile(new URL("../README.md", import.meta.url), join(packageRoot, "README.md"));
    await writeFile(join(packageRoot, "package.json"), `${JSON.stringify(manifest, null, 4)}\n`);

    const result = execFileSync(process.execPath, [
        npmPath, "pack", "--ignore-scripts", "--json", "--pack-destination", staging, packageRoot
    ], { encoding: "utf8", windowsHide: true });
    const tarball = JSON.parse(result)[0]?.filename;
    const expected = `alcomd-sdk-${manifest.version}.tgz`;
    if (tarball !== expected) throw new Error("npm returned an unexpected tarball name.");
    const archive = join(output, `alcomd-sdk-${manifest.version}-${target}.tgz`);
    await copyFile(join(staging, tarball), archive);
    process.stdout.write(`${archive}\n`);
} finally {
    // staging is a private directory created by this invocation under the temp root.
    await rm(staging, { recursive: true, force: true });
}
