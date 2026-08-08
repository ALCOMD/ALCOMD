import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repositoryRoot = path.resolve(__dirname, "../..");
const serverSourcePath = path.join(repositoryRoot, "alcomd3-mcp/src/main.rs");

const references = [
	{
		path: "docs/mcp/tools.md",
		inputMarker: "**Input:**",
		outputMarker: "**Success output:**",
	},
	{
		path: "docs/mcp/tools.zh-CN.md",
		inputMarker: "**输入：**",
		outputMarker: "**成功输出：**",
	},
	{
		path: "docs/mcp/tools.zh-TW.md",
		inputMarker: "**輸入：**",
		outputMarker: "**成功輸出：**",
	},
	{
		path: "docs/mcp/tools.ja.md",
		inputMarker: "**入力:**",
		outputMarker: "**成功出力:**",
	},
];

const mainGuideLinks = [
	["docs/mcp.md", "mcp/tools.md"],
	["docs/mcp/mcp.zh-CN.md", "tools.zh-CN.md"],
	["docs/mcp/mcp.zh-TW.md", "tools.zh-TW.md"],
	["docs/mcp/mcp.ja.md", "tools.ja.md"],
];

function troubleshootingHeadings(documentPath, content) {
	const sectionStart = content.indexOf("\n### `mcp_disabled`");
	if (sectionStart < 0) {
		console.error(`${documentPath}: missing MCP troubleshooting section.`);
		hasFailure = true;
		return [];
	}

	const nextSection = content.indexOf("\n## ", sectionStart + 1);
	const section = content.slice(
		sectionStart,
		nextSection < 0 ? content.length : nextSection,
	);
	return [...section.matchAll(/^### (.+)$/gm)].map((match) => match[1]);
}

function validateMarkdownTables(documentPath, content) {
	const lines = content.split(/\r?\n/);
	let expectedPipes = null;

	for (let index = 0; index < lines.length; index += 1) {
		const line = lines[index];
		if (!line.startsWith("|")) {
			expectedPipes = null;
			continue;
		}

		const pipeCount = (line.match(/(?<!\\)\|/g) ?? []).length;
		if (expectedPipes == null) {
			expectedPipes = pipeCount;
		} else if (pipeCount !== expectedPipes) {
			console.error(
				`${documentPath}:${index + 1}: Markdown table has ${pipeCount} columns separators; expected ${expectedPipes}.`,
			);
			hasFailure = true;
		}
	}
}

const source = await readFile(serverSourcePath, "utf8");
const toolNames = [
	...source.matchAll(/\basync fn (alcomd3_[a-z0-9_]+)\s*\(/g),
].map((match) => match[1]);
const uniqueToolNames = [...new Set(toolNames)];
let hasFailure = false;

if (toolNames.length !== uniqueToolNames.length) {
	console.error("MCP server contains duplicate public tool function names.");
	hasFailure = true;
}

if (uniqueToolNames.length !== 33) {
	console.error(
		`Expected 33 public MCP tools, found ${uniqueToolNames.length}. ` +
			"Update the references and this expected count together.",
	);
	hasFailure = true;
}

for (const reference of references) {
	const referencePath = path.join(repositoryRoot, reference.path);
	const content = await readFile(referencePath, "utf8");
	validateMarkdownTables(reference.path, content);
	const documentedNames = [
		...content.matchAll(/^### `(alcomd3_[a-z0-9_]+)`\s*$/gm),
	].map((match) => match[1]);
	const documentedSet = new Set(documentedNames);

	if (documentedNames.length !== documentedSet.size) {
		console.error(`${reference.path}: duplicate tool reference heading.`);
		hasFailure = true;
	}

	const missing = uniqueToolNames.filter((name) => !documentedSet.has(name));
	const unknown = documentedNames.filter(
		(name) => !uniqueToolNames.includes(name),
	);

	if (missing.length > 0) {
		console.error(`${reference.path}: missing tools: ${missing.join(", ")}`);
		hasFailure = true;
	}

	if (unknown.length > 0) {
		console.error(`${reference.path}: unknown tools: ${unknown.join(", ")}`);
		hasFailure = true;
	}

	for (const toolName of uniqueToolNames) {
		const heading = `### \`${toolName}\``;
		const sectionStart = content.indexOf(heading);
		if (sectionStart < 0) continue;
		const nextSection = content.indexOf(
			"\n### ",
			sectionStart + heading.length,
		);
		const section = content.slice(
			sectionStart,
			nextSection < 0 ? content.length : nextSection,
		);

		if (!section.includes(reference.inputMarker)) {
			console.error(`${reference.path}: ${toolName} has no input section.`);
			hasFailure = true;
		}

		if (!section.includes(reference.outputMarker)) {
			console.error(
				`${reference.path}: ${toolName} has no success-output section.`,
			);
			hasFailure = true;
		}
	}
}

for (const [guide, referenceLink] of mainGuideLinks) {
	const content = await readFile(path.join(repositoryRoot, guide), "utf8");
	if (!content.includes(`](${referenceLink})`)) {
		console.error(`${guide}: missing link to ${referenceLink}.`);
		hasFailure = true;
	}
}

const guideContents = await Promise.all(
	mainGuideLinks.map(async ([guide]) => [
		guide,
		await readFile(path.join(repositoryRoot, guide), "utf8"),
	]),
);
const expectedTroubleshootingHeadings = troubleshootingHeadings(
	guideContents[0][0],
	guideContents[0][1],
);

for (const [guide, content] of guideContents.slice(1)) {
	const actualHeadings = troubleshootingHeadings(guide, content);
	if (
		actualHeadings.length !== expectedTroubleshootingHeadings.length ||
		actualHeadings.some(
			(heading, index) => heading !== expectedTroubleshootingHeadings[index],
		)
	) {
		console.error(
			`${guide}: troubleshooting headings differ from docs/mcp.md.\n` +
				`Expected: ${expectedTroubleshootingHeadings.join(", ")}\n` +
				`Actual: ${actualHeadings.join(", ")}`,
		);
		hasFailure = true;
	}
}

if (hasFailure) {
	process.exitCode = 1;
} else {
	console.log(
		`MCP documentation coverage OK (${uniqueToolNames.length} tools, ${references.length} languages).`,
	);
}
