export const THEME_SIDEBAR_EXTENSION_ID = "theme";
export const UNITY_DISCORD_STATUS_EXTENSION_ID = "unity-discord-status";
export const EXTENSION_STATE_CHANGED_EVENT = "extension-state-changed";
export const SIDEBAR_EXTENSIONS_QUERY_KEY = [
	"environmentGetSidebarExtensions",
] as const;
export const EXTENSION_MANAGEMENT_QUERY_KEY = [
	"environmentGetExtensionManagement",
] as const;

export const DEFAULT_SIDEBAR_EXTENSION_ORDER = [
	"projects",
	"packages",
	"mcp",
	THEME_SIDEBAR_EXTENSION_ID,
	"settings",
	"log",
	UNITY_DISCORD_STATUS_EXTENSION_ID,
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

export function restoreDefaultSidebarExtensions<
	T extends { id: string; installed: boolean; visible: boolean },
>(extensions: readonly T[]): T[] {
	return sortSidebarExtensionsByDefaultOrder(extensions).map((extension) =>
		extension.installed && !extension.visible
			? { ...extension, visible: true }
			: extension,
	);
}
