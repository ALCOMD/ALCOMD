import { describe, expect, test } from "vitest";
import {
	countProcessedSteps,
	progressWithFinalStep,
} from "@/lib/operation-progress";

type TestProgressItem = {
	completedSteps: number;
	status: "running" | "completed" | "failed" | "cancelled";
};

function count(items: TestProgressItem[], stepsPerItem: number) {
	return countProcessedSteps(
		items,
		stepsPerItem,
		(item) => item.completedSteps,
		(item) => item.status === "failed",
	);
}

describe("countProcessedSteps", () => {
	test("counts every step of a failed item as processed", () => {
		expect(
			count(
				[
					{ completedSteps: 3, status: "completed" },
					{ completedSteps: 1, status: "failed" },
				],
				3,
			),
		).toBe(6);
	});

	test("reaches the total for completed and failed one-step items", () => {
		expect(
			count(
				[
					{ completedSteps: 1, status: "completed" },
					{ completedSteps: 0, status: "failed" },
				],
				1,
			),
		).toBe(2);
	});

	test("keeps unfinished and cancelled items at their actual progress", () => {
		expect(
			count(
				[
					{ completedSteps: 1, status: "running" },
					{ completedSteps: 0, status: "cancelled" },
				],
				3,
			),
		).toBe(1);
	});
});

describe("progressWithFinalStep", () => {
	test("reserves one percent while the final step is unfinished", () => {
		expect(progressWithFinalStep(2, 2, false)).toBe(99);
	});

	test("reaches one hundred only after the final step finishes", () => {
		expect(progressWithFinalStep(2, 2, true)).toBe(100);
	});

	test("keeps partial progress proportional within the first 99 percent", () => {
		expect(progressWithFinalStep(1, 2, false)).toBe(49.5);
		expect(progressWithFinalStep(0, 0, false)).toBe(0);
	});
});
