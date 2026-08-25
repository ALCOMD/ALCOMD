import { expect, test } from "@playwright/test";

test("official GUI home is reachable by keyboard in a real browser", async ({ page }) => {
    await page.goto("/");

    await expect(page.getByRole("heading", { level: 1, name: "ALCOMD3" })).toBeVisible();
    await page.keyboard.press("Tab");
    await expect(page.getByRole("button", { name: /ALCOMD3/i })).toBeFocused();
});
