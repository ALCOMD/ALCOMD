import { readdir, readFile } from "node:fs/promises";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "src");
const nativeControl = /<\s*(button|input|select|textarea|progress|dialog)\b/g;
const createdNativeControl = /(?:createElement|React\.createElement)\(\s*["'](button|input|select|textarea|progress|dialog)["']/g;
const buttonSpacingOverride = /--md-(?:filled|filled-tonal|outlined|text)-button-(?:(?:with-leading-icon|with-trailing-icon)-)?(?:leading|trailing)-space["']?\s*:/g;
// Material Web has no color picker. Only this exact shared adapter may invoke
// the browser's color dialog; all other native controls remain prohibited.
const approvedColorPickerCreation = `createElement("input", {
        type: "color",
        id,
        "aria-label": label,
        className: "alcomd-color-picker",
        value,
        disabled,
        onInput: (event: FormEvent<HTMLInputElement>) => onInput(event.currentTarget.value)
    })`;
const failures = [];

for (const path of await sourceFiles(root)) {
    const source = (await readFile(path, "utf8")).replaceAll("\r\n", "\n");
    const displayPath = relative(root, path).replaceAll("\\", "/");
    const matches = [...source.matchAll(nativeControl), ...source.matchAll(createdNativeControl)];
    const colorPickerIndex = displayPath === "Material.tsx" ? source.indexOf(approvedColorPickerCreation) : -1;
    for (const match of matches) {
        if (match[1] === "input" && match.index === colorPickerIndex && colorPickerIndex >= 0) continue;
        failures.push(`${displayPath}: unapproved native ${match[1]} control`);
    }
    for (const _match of source.matchAll(buttonSpacingOverride)) {
        failures.push(`${displayPath}: button spacing belongs in the shared @alcomd/ui theme`);
    }
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
        else if (entry.isFile() && (path.endsWith(".tsx") || path.endsWith(".css"))) files.push(path);
    }
    return files;
}
