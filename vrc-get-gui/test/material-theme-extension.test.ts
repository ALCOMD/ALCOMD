import { describe, expect, test } from "vitest";
import {
	disableMaterialTheme,
	isMaterialThemeExtensionEnabled,
	MATERIAL_THEME_EXTENSION_ENABLED_KEY,
	storeMaterialThemeExtensionEnabled,
	USER_THEME_STYLE_ID,
} from "@/lib/material-theme-extension";

describe("material theme extension state", () => {
	test("disabling removes the generated theme and restores the system base theme", () => {
		const style = document.createElement("style");
		style.id = USER_THEME_STYLE_ID;
		document.head.appendChild(style);
		document.documentElement.className = "dark";
		document.documentElement.style.setProperty("--theme-hue", "120");

		storeMaterialThemeExtensionEnabled(false);

		expect(isMaterialThemeExtensionEnabled()).toBe(false);
		expect(localStorage.getItem(MATERIAL_THEME_EXTENSION_ENABLED_KEY)).toBe(
			"false",
		);
		expect(document.getElementById(USER_THEME_STYLE_ID)).toBeNull();
		expect(document.documentElement.className).toBe("system");
		expect(document.documentElement.style.getPropertyValue("--theme-hue")).toBe(
			"",
		);
	});

	test("enabling persists the module state without discarding saved theme data", () => {
		localStorage.setItem("user_theme_source", "#ff3366");
		storeMaterialThemeExtensionEnabled(false);

		storeMaterialThemeExtensionEnabled(true);

		expect(isMaterialThemeExtensionEnabled()).toBe(true);
		expect(localStorage.getItem(MATERIAL_THEME_EXTENSION_ENABLED_KEY)).toBe(
			"true",
		);
		expect(localStorage.getItem("user_theme_source")).toBe("#ff3366");
	});

	test("explicit disable is idempotent", () => {
		disableMaterialTheme();
		disableMaterialTheme();

		expect(document.documentElement.className).toBe("system");
		expect(document.getElementById(USER_THEME_STYLE_ID)).toBeNull();
	});
});
