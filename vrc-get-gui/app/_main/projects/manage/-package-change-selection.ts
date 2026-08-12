export type PackageChangeMutationResult = "cancelled" | "settled";

export function shouldClearBulkUpdateSelection(
	result: PackageChangeMutationResult | undefined,
): boolean {
	return result !== "cancelled";
}
