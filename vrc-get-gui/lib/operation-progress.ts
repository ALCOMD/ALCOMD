export function countProcessedSteps<T>(
	items: readonly T[],
	stepsPerItem: number,
	getCompletedSteps: (item: T) => number,
	isFailed: (item: T) => boolean,
): number {
	return items.reduce(
		(total, item) =>
			total + (isFailed(item) ? stepsPerItem : getCompletedSteps(item)),
		0,
	);
}
