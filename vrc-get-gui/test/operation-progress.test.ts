import { describe, expect, test } from "vitest";
import { countProcessedSteps } from "@/lib/operation-progress";

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
