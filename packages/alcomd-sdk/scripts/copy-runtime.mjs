import { copyFile } from "node:fs/promises";

for (const name of ["node-runtime.mjs", "node-runtime.d.mts"]) {
    await copyFile(new URL(`../src/${name}`, import.meta.url), new URL(`../dist/${name}`, import.meta.url));
}
