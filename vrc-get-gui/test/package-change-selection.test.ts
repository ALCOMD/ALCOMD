import { describe, expect, test } from "vitest";
import { shouldClearBulkUpdateSelection } from "@/app/_main/projects/manage/-package-change-selection";

describe("package change selection", () => {
	test("keeps the bulk selection when confirmation is cancelled", () => {
		expect(shouldClearBulkUpdateSelection("cancelled")).toBe(false);
	});

	test("clears the bulk selection after confirmation or an error", () => {
		expect(shouldClearBulkUpdateSelection("settled")).toBe(true);
		expect(shouldClearBulkUpdateSelection(undefined)).toBe(true);
	});
});
