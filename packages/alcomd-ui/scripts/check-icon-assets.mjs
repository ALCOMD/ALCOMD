import { createHash } from "node:crypto";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const assetRoot = join(packageRoot, "assets", "material-symbols");
const manifest = readFileSync(join(assetRoot, "manifest.toml"), "utf8");
const iconSource = readFileSync(join(packageRoot, "src", "icons.ts"), "utf8");
const themeSource = readFileSync(join(packageRoot, "src", "theme.css"), "utf8");
const packageManifest = readFileSync(join(packageRoot, "package.json"), "utf8");

assertContains(manifest, 'upstream_repository = "https://github.com/google/material-design-icons"');
assertContains(manifest, 'upstream_commit = "e083cc60a0828fdd3b404cea0cb8a5b900e9c23e"');
assertContains(manifest, 'license = "Apache-2.0"');
assertContains(manifest, 'style = "Rounded"');
assertContains(manifest, "weight = 400");
assertContains(manifest, "grade = 0");
assertContains(manifest, "fill = 0");

const entries = manifest.split("[[asset]]").slice(1).map((block) => ({
    name: readString(block, "name"),
    opticalSize: readNumber(block, "optical_size"),
    path: readString(block, "path"),
    upstreamPath: readString(block, "upstream_path"),
    sha256: readString(block, "sha256")
}));
if (entries.length === 0) fail("asset manifest is empty");

const expectedFiles = new Set();
for (const entry of entries) {
    if (entry.opticalSize !== 20 && entry.opticalSize !== 24) {
        fail(`${entry.name}: unsupported optical size ${entry.opticalSize}`);
    }
    const expectedPath = `${entry.opticalSize}/${entry.name}.svg`;
    if (entry.path !== expectedPath) fail(`${entry.name}: path must be ${expectedPath}`);
    const expectedUpstream = `symbols/web/${entry.name}/materialsymbolsrounded/${entry.name}_${entry.opticalSize}px.svg`;
    if (entry.upstreamPath !== expectedUpstream) fail(`${entry.name}: upstream path mismatch`);
    if (expectedFiles.has(entry.path)) fail(`${entry.name}: duplicate asset path`);
    expectedFiles.add(entry.path);

    const svg = readFileSync(join(assetRoot, entry.path), "utf8");
    const canonicalSvg = svg.trimEnd();
    const digest = createHash("sha256").update(canonicalSvg, "utf8").digest("hex");
    if (digest !== entry.sha256) fail(`${entry.name}: SHA-256 mismatch`);
    assertContains(canonicalSvg, `height="${entry.opticalSize}"`);
    assertContains(canonicalSvg, `width="${entry.opticalSize}"`);
    assertContains(canonicalSvg, 'viewBox="0 -960 960 960"');
    if (!iconSource.includes(`../assets/material-symbols/${entry.path}?url`)) {
        fail(`${entry.name}: asset is not exposed by the closed @alcomd/ui icon source`);
    }
}

const actualFiles = new Set([20, 24].flatMap((size) => readdirSync(join(assetRoot, String(size)))
    .filter((name) => name.endsWith(".svg"))
    .map((name) => `${size}/${name}`)));
if (!setsEqual(expectedFiles, actualFiles)) fail("vendored SVG set differs from the manifest");
if (/@material-symbols\/(?:svg|font|svg-\d+|font-\d+)/.test(packageManifest + iconSource)) {
    fail("static Material Symbols npm package must not be used");
}
assertContains(themeSource, "mask-size: 100%");
assertContains(themeSource, "-webkit-mask-size: 100%");
if (/mask-size:\s*1(?:0[1-9]|[1-9]\d)%|transform:\s*scale|clip-path/.test(themeSource)) {
    fail("icon geometry must not crop or scale the upstream asset");
}
if (/more_vert(?:Scale|Padding|Weight)|moreVert(?:Scale|Padding|Weight)|icon\s*===\s*["']more_vert["']/.test(iconSource + themeSource)) {
    fail("per-icon more_vert geometry overrides are forbidden");
}

console.log(`Material Symbols asset boundary passed (${entries.length} exact assets).`);

function readString(block, key) {
    const match = block.match(new RegExp(`^${key} = "([^"]+)"$`, "m"));
    if (match === null) fail(`missing ${key}`);
    return match[1];
}

function readNumber(block, key) {
    const match = block.match(new RegExp(`^${key} = (\\d+)$`, "m"));
    if (match === null) fail(`missing ${key}`);
    return Number(match[1]);
}

function assertContains(value, expected) {
    if (!value.includes(expected)) fail(`missing required value: ${expected}`);
}

function setsEqual(left, right) {
    return left.size === right.size && [...left].every((value) => right.has(value));
}

function fail(message) {
    throw new Error(`Material Symbols asset boundary failed: ${message}`);
}
