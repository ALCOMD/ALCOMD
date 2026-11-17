import type { Page } from "@playwright/test";

export async function openLocalExtensionInstall(page: Page, t: (value: string) => string = (value) => value) {
    await page.getByRole("button", { name: t("More extension installation options"), exact: true }).click();
    await page.getByRole("menuitem", { name: t("Local file"), exact: true }).click();
}
