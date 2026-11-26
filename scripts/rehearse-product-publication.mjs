import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { createHash, createPrivateKey, createPublicKey, sign } from "node:crypto";
import { once } from "node:events";
import { createReadStream } from "node:fs";
import { copyFile, lstat, mkdir, mkdtemp, readFile, readdir, realpath, rm } from "node:fs/promises";
import { createServer } from "node:https";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs, promisify } from "node:util";

const syntheticKey = createPrivateKey({
    key: Buffer.concat([Buffer.from("302e020100300506032b657004220420", "hex"), Buffer.alloc(32, 37)]),
    format: "der",
    type: "pkcs8",
});
const domain = Buffer.from("ALCOMD product update manifest v1\0");
const targetByFormat = {
    inno: "x86_64-pc-windows-msvc",
    appimage: "x86_64-unknown-linux-gnu",
    dmg: "aarch64-apple-darwin",
};
const required = ["probe", "asset", "source-sha", "asset-sha256", "payload-sha256", "format",
    "channel", "version", "installed-version", "tls-key", "tls-cert", "tls-ca"];
const { values: options } = parseArgs({
    options: {
        ...Object.fromEntries(required.map((name) => [name, { type: "string" }])),
        help: { type: "boolean" },
        "public-key": { type: "boolean" },
    },
});
if (options.help) {
    console.log(`Build with the synthetic ALCOMD_RELEASE_PUBLIC_KEY_HEX from --public-key:\n` +
        `  cargo build --locked --release -p alcomd-updater --example product_publication_rehearsal\n` +
        `Run: node scripts/rehearse-product-publication.mjs\n  ${required.map((name) => `--${name} <value>`).join(" ")}\n` +
        `Use beta or stable and inno, appimage or dmg. Supply a TLS certificate for localhost\n` +
        `and its PEM CA. On Windows, run inside an offline guest that trusts the fixture CA. Port 443 must be free.\n` +
        `The result is a JSON receipt for this isolated publication run.`);
} else if (options["public-key"]) {
    const key = createPublicKey(syntheticKey).export({ format: "jwk" });
    console.log(Buffer.from(key.x, "base64url").toString("hex"));
} else {
    await rehearse(options);
}

function sha256(bytes) {
    return createHash("sha256").update(bytes).digest("hex");
}

async function fileSha256(path) {
    const hash = createHash("sha256");
    for await (const bytes of createReadStream(path)) hash.update(bytes);
    return hash.digest("hex");
}

async function snapshot(directory) {
    const files = (await readdir(directory)).sort();
    return Promise.all(files.map(async (name) => [name, await fileSha256(join(directory, name))]));
}

async function rehearse(input) {
    for (const option of required) assert(input[option], `Missing --${option}`);
    assert(/^[0-9a-f]{40}$/.test(input["source-sha"]), "Pin the candidate source commit.");
    for (const option of ["asset-sha256", "payload-sha256"]) {
        assert(/^[0-9a-f]{64}$/.test(input[option]), `Invalid --${option}`);
    }
    const { channel, format, version } = input;
    assert(channel === "stable" || channel === "beta", "Unknown update channel.");
    assert(Object.hasOwn(targetByFormat, format), "Unknown self-update format.");
    const target = targetByFormat[format];
    const otherChannel = channel === "stable" ? "beta" : "stable";
    const endpoint = (stream) => `/api/v1/product-updates/${stream}/${target}/${format}.json`;
    const asset = resolve(input.asset);
    const name = basename(asset);
    assert(/^ALCOMD_[A-Za-z0-9_.-]+$/.test(name), "Invalid product asset name.");
    const stat = await lstat(asset);
    assert(stat.isFile() && stat.size > 0 && stat.size <= 512 * 1024 * 1024, "Invalid product asset.");
    const assetRoute = `/candidate/${name}`;
    const utc = (offset) => new Date(Date.now() + offset).toISOString().replace(/\.\d{3}Z$/, "Z");
    const issuedAt = utc(-60_000);
    const expiresAt = utc(1_800_000);
    const manifest = (serial) => Buffer.from(JSON.stringify({
        schema: 1,
        productId: "com.vrclearn.alcomd",
        channel,
        target,
        format,
        version,
        serial,
        issuedAt,
        expiresAt,
        payloadManifestSha256: input["payload-sha256"],
        asset: { name, url: `https://localhost${assetRoute}`, bytes: stat.size, sha256: input["asset-sha256"] },
    }));
    const envelope = (bytes) => Buffer.from(JSON.stringify({
        manifest: bytes.toString("base64url"),
        signature: sign(null, Buffer.concat([domain, bytes]), syntheticKey).toString("hex"),
    }));

    const workspace = await realpath(fileURLToPath(new URL("../", import.meta.url)));
    const targetDirectory = join(workspace, "target");
    await mkdir(targetDirectory, { recursive: true });
    assert.equal(await realpath(targetDirectory), targetDirectory, "Expected a local target directory.");
    const temporary = await mkdtemp(join(targetDirectory, "m12-publication-"));
    const published = join(temporary, name);
    const requests = [];
    const checks = [];
    let feed;
    let corrupt = false;
    let isPublished = false;
    let server;
    try {
        await copyFile(asset, published);
        assert.equal(await fileSha256(published), input["asset-sha256"], "Publication input changed.");
        assert.equal((await lstat(published)).size, stat.size, "Publication input size changed.");
        const head = createReadStream(published, { start: 0, end: 0 });
        const [firstByte] = await once(head, "data");
        server = createServer({ cert: await readFile(input["tls-cert"]), key: await readFile(input["tls-key"]) },
            (request, response) => {
                requests.push(request.url);
                response.setHeader("Cache-Control", "no-store");
                if (request.method !== "GET") {
                    response.writeHead(405).end();
                } else if (request.url === endpoint(channel) && feed) {
                    response.writeHead(200, { "Content-Type": "application/json" }).end(feed);
                } else if (request.url === endpoint(channel) || request.url === endpoint(otherChannel)) {
                    response.writeHead(204).end();
                } else if (request.url === assetRoute && isPublished) {
                    response.writeHead(200, { "Content-Length": stat.size, "Content-Type": "application/octet-stream" });
                    if (corrupt) response.write(Buffer.from([firstByte[0] ^ 1]));
                    if (corrupt && stat.size === 1) return response.end();
                    const stream = createReadStream(published, { start: corrupt ? 1 : 0 });
                    stream.on("error", (error) => response.destroy(error)).pipe(response);
                    response.on("close", () => stream.destroy());
                } else {
                    response.writeHead(404).end();
                }
            });
        server.listen(443, "127.0.0.1");
        await once(server, "listening");
        const current = manifest(2);
        const currentEnvelope = envelope(current);
        const expected = { version, serial: 2, manifestSha256: sha256(current) };
        const stage = await prepare("accepted");
        const fresh = await prepare("fresh");
        await expectProbe("not-published", stage, channel, null, [endpoint(channel)]);
        assert.deepEqual(await state(stage), [[], []]);

        isPublished = true;
        feed = currentEnvelope;
        await expectProbe("published-download", stage, channel, expected, [endpoint(channel), assetRoute]);
        await expectVerifiedFiles(stage);
        const accepted = await state(stage);
        await expectProbe("channel-isolation", stage, otherChannel, null, [endpoint(otherChannel)]);
        assert.deepEqual(await state(stage), accepted);

        feed = undefined;
        await expectProbe("stop-keeps-accepted-bytes", stage, channel, null, [endpoint(channel)]);
        assert.deepEqual(await state(stage), accepted);
        await expectProbe("stop-hides-update-from-new-client", fresh, channel, null, [endpoint(channel)]);
        assert.deepEqual(await state(fresh), [[], []]);

        feed = envelope(manifest(1));
        await expectProbe("stale-serial-rejected", stage, channel, /replay state rejected/, [endpoint(channel)]);
        assert.deepEqual(await state(stage), accepted);

        feed = currentEnvelope;
        await expectProbe("resume-reuses-verified-download", stage, channel, expected, [endpoint(channel)]);
        assert.deepEqual(await state(stage), accepted);
        corrupt = true;
        await expectProbe("same-size-tamper-rejected", fresh, channel, /signed length and digest/,
            [endpoint(channel), assetRoute]);
        // Opening a signed feed can create an empty exclusive lock before checking its asset.
        assert.deepEqual(await state(fresh), [[], [[`update-${channel}-${format}.lock`, sha256(Buffer.alloc(0))]]]);
        corrupt = false;
        await expectProbe("fresh-download-after-resume", fresh, channel, expected, [endpoint(channel), assetRoute]);
        await expectVerifiedFiles(fresh);

        console.log(JSON.stringify({
            status: "passed",
            sourceCommit: input["source-sha"],
            channel,
            target,
            format,
            version,
            asset: { name, bytes: stat.size, sha256: input["asset-sha256"] },
            payloadManifestSha256: input["payload-sha256"],
            acceptedManifestSha256: expected.manifestSha256,
            checks,
        }, null, 4));

        async function expectVerifiedFiles(paths) {
            assert.equal(await fileSha256(join(paths.download, name)), input["asset-sha256"]);
            assert.deepEqual(await readFile(join(paths.download, `update-${sha256(currentEnvelope)}.json`)), currentEnvelope);
            const replay = JSON.parse(await readFile(join(paths.replay, `update-${channel}-${format}-v1.json`), "utf8"));
            assert.equal(replay.schema, 1);
            assert.equal(replay.accepted.serial, expected.serial);
            assert.equal(Buffer.from(replay.accepted.manifestSha256).toString("hex"), expected.manifestSha256);
        }
    } finally {
        if (server?.listening) {
            server.close();
            server.closeAllConnections();
            await once(server, "close");
        }
        const actual = await realpath(temporary);
        assert.equal(actual, temporary, "Unexpected rehearsal directory location.");
        assert.equal(dirname(actual), targetDirectory, "Unexpected rehearsal directory parent.");
        await rm(actual, { recursive: true });
    }

    async function prepare(label) {
        const download = join(temporary, `${label}-download`);
        const replay = join(temporary, `${label}-replay`);
        await mkdir(download, { mode: 0o700 });
        await mkdir(replay, { mode: 0o700 });
        return { download, replay };
    }

    function state(paths) {
        return Promise.all([snapshot(paths.download), snapshot(paths.replay)]);
    }

    async function expectProbe(label, paths, stream, expected, routes) {
        const before = requests.length;
        const env = Object.fromEntries(Object.entries(process.env).filter(([key]) =>
            !["HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY", "NO_PROXY", "SSL_CERT_DIR", "SSL_CERT_FILE"]
                .includes(key.toUpperCase())));
        env.NO_PROXY = "localhost,127.0.0.1";
        env.SSL_CERT_FILE = resolve(input["tls-ca"]);
        const probe = promisify(execFile)(resolve(input.probe), [
            "--channel", stream, "--target", target, "--format", format,
            "--installed-version", input["installed-version"],
            "--download-directory", paths.download, "--replay-directory", paths.replay,
        ], { env, timeout: 90_000, maxBuffer: 64 * 1024 });
        if (expected instanceof RegExp) {
            await assert.rejects(probe, (error) => error.code === 1 && expected.test(error.stderr), label);
        } else {
            const { stdout } = await probe;
            assert.deepEqual(JSON.parse(stdout), expected, label);
        }
        assert.deepEqual(requests.slice(before), routes, label);
        checks.push(label);
    }
}
