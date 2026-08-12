import { readdir, readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import JSON5 from "json5";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const localeDir = path.resolve(__dirname, "../locales");
const baseLocaleFile = "en.json5";

const localeFiles = (await readdir(localeDir))
	.filter((fileName) => fileName.endsWith(".json5"))
	.sort();

const translations = new Map();
let hasFailure = false;

function reportList(localeFile, description, values) {
	if (values.length === 0) return;

	console.error(`${localeFile}: ${description}:`);
	for (const value of values) {
		console.error(`  - ${value}`);
	}
	hasFailure = true;
}

function collectDuplicateKeys(source) {
	const occurrences = new Map();
	const keyPattern = /^\s{4}(["'])(.*?)\1\s*:/gm;

	for (const match of source.matchAll(keyPattern)) {
		const key = match[2];
		const line = source.slice(0, match.index).split("\n").length;
		const lines = occurrences.get(key) ?? [];
		lines.push(line);
		occurrences.set(key, lines);
	}

	return [...occurrences.entries()]
		.filter(([, lines]) => lines.length > 1)
		.map(([key, lines]) => `${key} (lines ${lines.join(", ")})`);
}

function collectInterpolationVariables(value) {
	const variables = new Set();
	const interpolationPattern = /{{\s*-?\s*([^},\s]+)(?:\s*,[^}]*)?}}/g;

	for (const match of value.matchAll(interpolationPattern)) {
		variables.add(match[1]);
	}

	return variables;
}

function collectFunctionalTags(value) {
	const functionalTags = new Map([
		["externallink", 0],
		["l", 0],
		["path", 0],
	]);
	const tagPattern = /<\/?([A-Za-z][\w-]*)\b[^>]*>/g;

	for (const match of value.matchAll(tagPattern)) {
		const tag = match[1].toLowerCase();
		if (functionalTags.has(tag)) {
			functionalTags.set(tag, functionalTags.get(tag) + 1);
		}
	}

	return [...functionalTags.entries()]
		.filter(([, count]) => count !== 0)
		.map(([tag, count]) => `${tag}:${count}`)
		.sort();
}

function collectUnbalancedTags(value) {
	const balancedTagNames = new Set(["b", "code", "externallink", "l", "path"]);
	const tagPattern = /<(\/)?([A-Za-z][\w-]*)\b[^>]*>/g;
	const stack = [];
	const errors = [];

	for (const match of value.matchAll(tagPattern)) {
		const tag = match[2].toLowerCase();
		if (!balancedTagNames.has(tag)) continue;

		if (match[1] == null) {
			stack.push(tag);
		} else if (stack.pop() !== tag) {
			errors.push(`unexpected closing tag </${tag}>`);
		}
	}

	for (const tag of stack.reverse()) {
		errors.push(`missing closing tag </${tag}>`);
	}

	return errors;
}

function collectTechnicalVersions(value) {
	return [...value.matchAll(/\d+(?:\.\d+)+|\d{4,}/g)]
		.map((match) => match[0])
		.sort();
}

function difference(left, right) {
	return [...left].filter((value) => !right.has(value));
}

for (const localeFile of localeFiles) {
	const localePath = path.join(localeDir, localeFile);
	const source = await readFile(localePath, "utf8");
	reportList(localeFile, "duplicate locale keys", collectDuplicateKeys(source));

	const locale = JSON5.parse(source);
	const translation = locale.translation;

	if (typeof translation !== "object" || translation == null) {
		console.error(`${localeFile}: missing translation object`);
		hasFailure = true;
		continue;
	}

	translations.set(localeFile, translation);
}

const baseTranslation = translations.get(baseLocaleFile);

if (baseTranslation == null) {
	console.error(`${baseLocaleFile}: base locale is missing`);
	process.exit(1);
}

const baseKeys = Object.keys(baseTranslation).sort();
const baseKeySet = new Set(baseKeys);

for (const localeFile of localeFiles) {
	const translation = translations.get(localeFile);
	if (translation == null) continue;

	const missingKeys = baseKeys.filter((key) => !(key in translation));
	reportList(
		localeFile,
		`missing ${missingKeys.length} locale keys from ${baseLocaleFile}`,
		missingKeys,
	);

	const unexpectedKeys = Object.keys(translation)
		.filter((key) => !baseKeySet.has(key))
		.sort();
	reportList(
		localeFile,
		`contains ${unexpectedKeys.length} keys absent from ${baseLocaleFile}`,
		unexpectedKeys,
	);

	for (const key of baseKeys) {
		if (!(key in translation)) continue;

		const baseValue = baseTranslation[key];
		const localizedValue = translation[key];
		if (typeof baseValue !== "string" || typeof localizedValue !== "string") {
			if (typeof baseValue !== typeof localizedValue) {
				console.error(
					`${localeFile}: ${key} has type ${typeof localizedValue}; expected ${typeof baseValue}`,
				);
				hasFailure = true;
			}
			continue;
		}

		const baseVariables = collectInterpolationVariables(baseValue);
		const localizedVariables = collectInterpolationVariables(localizedValue);
		const missingVariables = difference(
			baseVariables,
			localizedVariables,
		).filter((variable) => !(variable === "count" && key.endsWith("_one")));
		const unexpectedVariables = difference(localizedVariables, baseVariables);
		if (missingVariables.length > 0 || unexpectedVariables.length > 0) {
			console.error(`${localeFile}: interpolation mismatch for ${key}`);
			if (missingVariables.length > 0) {
				console.error(`  missing: ${missingVariables.join(", ")}`);
			}
			if (unexpectedVariables.length > 0) {
				console.error(`  unexpected: ${unexpectedVariables.join(", ")}`);
			}
			hasFailure = true;
		}

		const baseTags = collectFunctionalTags(baseValue);
		const localizedTags = collectFunctionalTags(localizedValue);
		if (baseTags.join("|") !== localizedTags.join("|")) {
			console.error(`${localeFile}: functional tag mismatch for ${key}`);
			console.error(`  expected: ${baseTags.join(", ") || "none"}`);
			console.error(`  actual: ${localizedTags.join(", ") || "none"}`);
			hasFailure = true;
		}

		const unbalancedTags = collectUnbalancedTags(localizedValue);
		if (unbalancedTags.length > 0) {
			console.error(`${localeFile}: unbalanced tags for ${key}`);
			for (const error of unbalancedTags) {
				console.error(`  - ${error}`);
			}
			hasFailure = true;
		}

		const baseVersions = collectTechnicalVersions(baseValue);
		const localizedVersions = collectTechnicalVersions(localizedValue);
		if (baseVersions.join("|") !== localizedVersions.join("|")) {
			console.error(`${localeFile}: technical version mismatch for ${key}`);
			console.error(`  expected: ${baseVersions.join(", ") || "none"}`);
			console.error(`  actual: ${localizedVersions.join(", ") || "none"}`);
			hasFailure = true;
		}

		if (baseValue.trimEnd().endsWith("?") && !/[?？]/.test(localizedValue)) {
			console.error(`${localeFile}: question semantics missing for ${key}`);
			hasFailure = true;
		}

		const visibleBaseWords = baseValue
			.replace(/<[^>]+>|{{[^}]+}}/g, " ")
			.match(/[A-Za-z]{2,}/g);
		const isTechnicalIdentifier = /^[A-Za-z0-9_.-]+$/.test(baseValue.trim());
		if (
			localeFile !== baseLocaleFile &&
			localizedValue === baseValue &&
			!isTechnicalIdentifier &&
			visibleBaseWords != null &&
			visibleBaseWords.length >= 3
		) {
			console.error(`${localeFile}: probable English fallback for ${key}`);
			hasFailure = true;
		}
	}
}

if (hasFailure) {
	process.exitCode = 1;
} else {
	console.log(
		`Locale consistency OK (${localeFiles.length} locales, ${baseKeys.length} keys).`,
	);
}
