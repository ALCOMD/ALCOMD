import { readdir, readFile } from "node:fs/promises";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import ts from "typescript";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "src");
const failures = [];
const messages = new Map();
const placeholders = (value) => [...value.matchAll(/\{([a-zA-Z][a-zA-Z0-9]*)\}/g)].map((match) => match[1]).sort().join(",");

for (const file of await readdir(join(root, "locales"))) {
    if (!file.endsWith(".ts")) continue;
    const path = join(root, "locales", file);
    const ast = parse(path, await readFile(path, "utf8"));
    for (const statement of ast.statements) {
        if (!ts.isVariableStatement(statement)) continue;
        for (const declaration of statement.declarationList.declarations) {
            if (!declaration.initializer || !ts.isObjectLiteralExpression(declaration.initializer)) continue;
            const local = new Set();
            for (const property of declaration.initializer.properties) {
                if (!ts.isPropertyAssignment(property) || !ts.isStringLiteral(property.name) || !ts.isObjectLiteralExpression(property.initializer)) {
                    failures.push(`${file}: catalog entries must be literal messages`); continue;
                }
                const key = property.name.text;
                if (local.has(key)) failures.push(`${file}: duplicate message ${key}`);
                local.add(key);
                const values = {};
                for (const entry of property.initializer.properties) {
                    if (ts.isPropertyAssignment(entry) && ts.isStringLiteral(entry.name) && ts.isStringLiteral(entry.initializer)) values[entry.name.text] = entry.initializer.text;
                }
                for (const locale of ["zh-CN", "ja-JP"]) {
                    if (!values[locale]?.trim()) failures.push(`${file}: missing ${locale}: ${key}`);
                    else if (placeholders(key) !== placeholders(values[locale])) failures.push(`${file}: ${locale} placeholder mismatch: ${key}`);
                }
                if (Object.keys(values).some((locale) => !["zh-CN", "ja-JP"].includes(locale))) failures.push(`${file}: unexpected locale: ${key}`);
                const previous = messages.get(key);
                if (previous && JSON.stringify(previous) !== JSON.stringify(values)) failures.push(`${file}: conflicting translation: ${key}`);
                messages.set(key, values);
            }
        }
    }
}

for (const path of await sourceFiles(root)) {
    if (path.includes(`${join(root, "locales")}`) || path.endsWith(".test.ts")) continue;
    const ast = parse(path, await readFile(path, "utf8"));
    const label = relative(root, path);
    const requireMessage = (key) => { if (!messages.has(key)) failures.push(`${label}: missing translated message: ${key}`); };
    function visit(node) {
        if (ts.isCallExpression(node) && ts.isIdentifier(node.expression) && node.expression.text === "t" && node.arguments[0] && ts.isStringLiteral(node.arguments[0])) requireMessage(node.arguments[0].text);
        if (ts.isVariableDeclaration(node) && ts.isIdentifier(node.name) && node.name.text === "labels" && node.initializer && ts.isObjectLiteralExpression(node.initializer)) {
            for (const entry of node.initializer.properties) if (ts.isPropertyAssignment(entry) && ts.isStringLiteral(entry.initializer)) requireMessage(entry.initializer.text);
        }
        if (ts.isJsxAttribute(node) && node.name.getText(ast) === "message" && node.initializer && ts.isStringLiteral(node.initializer)) requireMessage(node.initializer.text);
        if (ts.isJsxText(node)) {
            const text = node.text.trim();
            if (/[A-Za-z]/.test(text) && !["Unity", "ALCOMD", "ALCOMD3", "UTC", "VPM", "SHA-256", "THIRD_PARTY_NOTICES.md"].includes(text)) failures.push(`${label}:${ast.getLineAndCharacterOfPosition(node.pos).line + 1}: untranslated JSX: ${text}`);
        }
        if (ts.isJsxAttribute(node) && ["title", "label", "aria-label", "placeholder", "supportingText", "detail", "refreshingLabel"].includes(node.name.getText(ast)) && node.initializer && ts.isStringLiteral(node.initializer) && /[A-Za-z]/.test(node.initializer.text) && !["Unity", "ALCOMD", "ALCOMD3", "VPM", "SHA-256", "RPC"].includes(node.initializer.text)) failures.push(`${label}: untranslated ${node.name.getText(ast)}: ${node.initializer.text}`);
        ts.forEachChild(node, visit);
    }
    visit(ast);
}

if (failures.length) {
    console.error(failures.join("\n"));
    process.exitCode = 1;
} else console.log(`Locale boundary passed (${messages.size} messages, en-US / zh-CN / ja-JP).`);

function parse(path, source) { return ts.createSourceFile(path, source, ts.ScriptTarget.Latest, true, path.endsWith(".tsx") ? ts.ScriptKind.TSX : ts.ScriptKind.TS); }
async function sourceFiles(directory) {
    const paths = [];
    for (const entry of await readdir(directory, { withFileTypes: true })) {
        const path = join(directory, entry.name);
        if (entry.isDirectory()) paths.push(...await sourceFiles(path));
        else if (/\.tsx?$/.test(entry.name)) paths.push(path);
    }
    return paths;
}
