import { describe, expect, test } from "vitest";
import {
	disableMaterialTheme,
	isMaterialThemeExtensionEnabled,
	setMaterialThemeExtensionRuntimeEnabled,
	USER_THEME_STYLE_ID,
} from "@/lib/material-theme-extension";

describe("material theme extension state", () => {
	test("disabling removes the generated theme and restores the system base theme", () => {
		const style = document.createElement("style");
		style.id = USER_THEME_STYLE_ID;
		document.head.appendChild(style);
		document.documentElement.className = "dark";
		document.documentElement.style.setProperty("--theme-hue", "120");

		setMaterialThemeExtensionRuntimeEnabled(false);

		expect(isMaterialThemeExtensionEnabled()).toBe(false);
		expect(document.getElementById(USER_THEME_STYLE_ID)).toBeNull();
		expect(document.documentElement.className).toBe("system");
		expect(document.documentElement.style.getPropertyValue("--theme-hue")).toBe(
			"",
		);
	});

	test("enabling changes runtime presentation", () => {
		setMaterialThemeExtensionRuntimeEnabled(false);

		setMaterialThemeExtensionRuntimeEnabled(true);

		expect(isMaterialThemeExtensionEnabled()).toBe(true);
	});

	test("explicit disable is idempotent", () => {
		disableMaterialTheme();
		disableMaterialTheme();

		expect(document.documentElement.className).toBe("system");
		expect(document.getElementById(USER_THEME_STYLE_ID)).toBeNull();
	});
});
