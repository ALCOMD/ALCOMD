import { expect, type Page } from "@playwright/test";

export type SettingsSection = "general" | "unity" | "appearance";

/** Use the real section buttons so translated labels do not change the flow. */
export async function selectSettingsSection(page: Page, section: SettingsSection) {
    const sections: readonly SettingsSection[] = ["general", "unity", "appearance"];
    const navigation = page.locator(".utility-workspace-toolbar nav.resource-navigation");
    const button = navigation.getByRole("button").nth(sections.indexOf(section));
    await expect(button).toBeVisible();
    if (await button.getAttribute("aria-pressed") !== "true") await button.click();
    await expect(button).toHaveAttribute("aria-pressed", "true");
}
