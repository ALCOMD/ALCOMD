export type PackageApplyProgressDisplayStatus =
	| "applying"
	| "finalizing"
	| "completed"
	| "failed";

export function packageApplyProgressPercent(
	status: PackageApplyProgressDisplayStatus,
	completedSteps: number,
	totalSteps: number,
): number {
	if (status === "completed") return 100;
	if (totalSteps <= 0) return 0;

	return Math.min(
		99,
		Math.max(0, Math.round((completedSteps / totalSteps) * 99)),
	);
}
