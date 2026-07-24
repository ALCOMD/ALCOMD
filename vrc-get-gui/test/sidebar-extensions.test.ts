import { describe, expect, test } from "vitest";
import {
	DEFAULT_SIDEBAR_EXTENSION_ORDER,
	sortSidebarExtensionsByDefaultOrder,
} from "@/lib/sidebar-extensions";

describe("sidebar extensions", () => {
	test("uses the configured default order", () => {
		expect(DEFAULT_SIDEBAR_EXTENSION_ORDER).toEqual([
			"projects",
			"packages",
			"mcp",
			"theme",
			"settings",
			"log",
		]);
	});

	test("restores known extensions and preserves unknown extension order", () => {
		const extensions = [
			{ id: "log" },
			{ id: "custom-b" },
			{ id: "theme" },
			{ id: "projects" },
			{ id: "custom-a" },
			{ id: "mcp" },
		];

		expect(
			sortSidebarExtensionsByDefaultOrder(extensions).map(({ id }) => id),
		).toEqual(["projects", "mcp", "theme", "log", "custom-b", "custom-a"]);
	});

	test("does not mutate the current order", () => {
		const extensions = [{ id: "log" }, { id: "projects" }];

		sortSidebarExtensionsByDefaultOrder(extensions);

		expect(extensions.map(({ id }) => id)).toEqual(["log", "projects"]);
	});
});
