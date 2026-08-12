import { describe, expect, test } from "vitest";
import { packageApplyProgressPercent } from "@/app/_main/projects/manage/-package-apply-progress";

describe("package apply progress", () => {
	test("uses at most 99 percent before backend finalization completes", () => {
		expect(packageApplyProgressPercent("applying", 0, 6)).toBe(0);
		expect(packageApplyProgressPercent("applying", 3, 6)).toBe(50);
		expect(packageApplyProgressPercent("finalizing", 6, 6)).toBe(99);
	});

	test("reaches 100 percent only after completion", () => {
		expect(packageApplyProgressPercent("completed", 6, 6)).toBe(100);
		expect(packageApplyProgressPercent("failed", 6, 6)).toBe(99);
	});

	test("handles an operation without measurable package steps", () => {
		expect(packageApplyProgressPercent("applying", 0, 0)).toBe(0);
		expect(packageApplyProgressPercent("completed", 0, 0)).toBe(100);
	});
});
