import { readdir, readFile } from "node:fs/promises";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "src");
const nativeControl = /<\s*(button|input|select|textarea|progress|dialog)\b/g;
const createdNativeControl = /(?:createElement|React\.createElement)\(\s*["'](button|input|select|textarea|progress|dialog)["']/g;
const failures = [];

for (const path of await sourceFiles(root)) {
    const source = await readFile(path, "utf8");
    const displayPath = relative(root, path).replaceAll("\\", "/");
    const matches = [...source.matchAll(nativeControl), ...source.matchAll(createdNativeControl)];
    for (const match of matches) failures.push(`${displayPath}: unapproved native ${match[1]} control`);
}

if (failures.length > 0) {
    console.error("Material control boundary failed:");
    for (const failure of failures) console.error(`- ${failure}`);
    process.exitCode = 1;
} else {
    console.log("Material control boundary passed.");
}

async function sourceFiles(directory) {
    const files = [];
    for (const entry of await readdir(directory, { withFileTypes: true })) {
        const path = join(directory, entry.name);
        if (entry.isDirectory()) files.push(...await sourceFiles(path));
        else if (entry.isFile() && path.endsWith(".tsx")) files.push(path);
    }
    return files;
}
