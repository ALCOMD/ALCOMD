export const DEFAULT_SIDEBAR_EXTENSION_ORDER = [
	"projects",
	"packages",
	"mcp",
	"theme",
	"settings",
	"log",
] as const;

const DEFAULT_SIDEBAR_EXTENSION_INDEX = new Map<string, number>(
	DEFAULT_SIDEBAR_EXTENSION_ORDER.map((id, index) => [id, index]),
);

export function sortSidebarExtensionsByDefaultOrder<T extends { id: string }>(
	extensions: readonly T[],
): T[] {
	return extensions
		.map((extension, currentIndex) => ({
			extension,
			currentIndex,
			defaultIndex: DEFAULT_SIDEBAR_EXTENSION_INDEX.get(extension.id),
		}))
		.sort((left, right) => {
			if (left.defaultIndex == null) {
				return right.defaultIndex == null
					? left.currentIndex - right.currentIndex
					: 1;
			}
			if (right.defaultIndex == null) return -1;
			return left.defaultIndex - right.defaultIndex;
		})
		.map(({ extension }) => extension);
}
