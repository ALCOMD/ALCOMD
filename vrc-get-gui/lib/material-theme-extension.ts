export const USER_THEME_STYLE_ID = "user-theme-style";
let materialThemeExtensionEnabled = false;

export function isMaterialThemeExtensionEnabled() {
	return materialThemeExtensionEnabled;
}

export function disableMaterialTheme() {
	if (typeof document === "undefined") return;

	document.getElementById(USER_THEME_STYLE_ID)?.remove();
	document.documentElement.classList.remove("light", "dark", "system");
	document.documentElement.classList.add("system");
	document.documentElement.style.removeProperty("--theme-hue");
}

export function setMaterialThemeExtensionRuntimeEnabled(enabled: boolean) {
	materialThemeExtensionEnabled = enabled;
	if (!enabled) {
		disableMaterialTheme();
	}
}
