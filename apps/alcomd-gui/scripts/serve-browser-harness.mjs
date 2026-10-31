import { lstat, mkdir, readdir, realpath } from "node:fs/promises";
import { isAbsolute, join, relative, resolve, sep } from "node:path";
import { performance } from "node:perf_hooks";
import { fileURLToPath } from "node:url";

const startedAt = performance.now();
const guiRoot = await realpath(fileURLToPath(new URL("../", import.meta.url)));
const workspaceRoot = await realpath(resolve(guiRoot, "../.."));
const targetRoot = join(workspaceRoot, "target");
const outDir = join(targetRoot, "gui-browser-fixture");

function assertWithin(parent, child) {
    const path = relative(parent, child);
    if (!path || path === ".." || path.startsWith(`..${sep}`) || isAbsolute(path)) {
        throw new Error(`Browser fixture path escapes its dedicated directory: ${child}`);
    }
}

async function ensureDirectory(path) {
    let entry;
    try {
        entry = await lstat(path);
    } catch (error) {
        if (error.code !== "ENOENT") throw error;
        await mkdir(path);
        entry = await lstat(path);
    }
    if (entry.isSymbolicLink() || !entry.isDirectory() || relative(path, await realpath(path)) !== "") {
        throw new Error(`Browser fixture directory must not be a symbolic link or junction: ${path}`);
    }
}

async function assertOutputTree(path) {
    for (const entry of await readdir(path, { withFileTypes: true })) {
        const child = join(path, entry.name);
        const state = await lstat(child);
        if (state.isSymbolicLink()) throw new Error(`Browser fixture output contains a symbolic link or junction: ${child}`);
        assertWithin(outDir, await realpath(child));
        if (state.isDirectory()) await assertOutputTree(child);
    }
}

assertWithin(workspaceRoot, targetRoot);
assertWithin(targetRoot, outDir);
await ensureDirectory(targetRoot);
await ensureDirectory(outDir);
await assertOutputTree(outDir);

// Preserve the existing development React / StrictMode checks in a static bundle.
process.env.NODE_ENV = "development";
const { build, preview } = await import("vite");
const configFile = join(guiRoot, "vite.config.ts");
const shared = { configFile, root: guiRoot, mode: "development", appType: "mpa" };

console.info(`Browser fixture output: ${outDir}`);
await build({
    ...shared,
    build: {
        outDir,
        emptyOutDir: true,
        sourcemap: true,
        rollupOptions: {
            input: {
                harness: join(guiRoot, "browser-harness.html"),
                pagedItems: join(guiRoot, "tests/browser/paged-items.html"),
                utilityWorkspace: join(guiRoot, "tests/browser/utility-workspace.html")
            }
        }
    }
});
console.info(`Browser fixture build completed after ${Math.round(performance.now() - startedAt)} ms`);

await ensureDirectory(targetRoot);
await ensureDirectory(outDir);
const server = await preview({
    ...shared,
    build: { outDir },
    preview: { host: "127.0.0.1", port: 4173, strictPort: true }
});
console.info(`Browser fixture ready after ${Math.round(performance.now() - startedAt)} ms: http://127.0.0.1:4173/browser-harness.html`);

let stopping = false;
function shutdown() {
    if (stopping) return;
    stopping = true;
    server.httpServer.close((error) => {
        if (error) {
            console.error(error);
            process.exitCode = 1;
        }
    });
}
process.once("SIGINT", shutdown);
process.once("SIGTERM", shutdown);
