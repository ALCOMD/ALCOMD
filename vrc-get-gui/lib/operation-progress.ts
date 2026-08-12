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

export function progressWithFinalStep(
	processedSteps: number,
	totalSteps: number,
	finalStepFinished: boolean,
): number {
	if (finalStepFinished) return 100;
	if (totalSteps === 0) return 0;
	return (processedSteps / totalSteps) * 99;
}
