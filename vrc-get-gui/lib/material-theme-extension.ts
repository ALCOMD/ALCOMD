export const USER_THEME_STYLE_ID = "user-theme-style";
export const MATERIAL_THEME_EXTENSION_ENABLED_KEY =
	"material_theme_extension_enabled";

export function isMaterialThemeExtensionEnabled() {
	if (typeof window === "undefined") return true;
	return localStorage.getItem(MATERIAL_THEME_EXTENSION_ENABLED_KEY) !== "false";
}

export function disableMaterialTheme() {
	if (typeof document === "undefined") return;

	document.getElementById(USER_THEME_STYLE_ID)?.remove();
	document.documentElement.classList.remove("light", "dark", "system");
	document.documentElement.classList.add("system");
	document.documentElement.style.removeProperty("--theme-hue");
}

export function storeMaterialThemeExtensionEnabled(enabled: boolean) {
	if (typeof window === "undefined") return;

	localStorage.setItem(
		MATERIAL_THEME_EXTENSION_ENABLED_KEY,
		enabled.toString(),
	);
	if (!enabled) {
		disableMaterialTheme();
	}
}
