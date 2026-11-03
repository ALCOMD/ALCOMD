import { expect, test } from "@playwright/test";

test.use({ viewport: { width: 1280, height: 900 } });

for (const mode of ["light", "dark"]) {
    test(`all floating notification types share one surface and always close in ${mode}`, async ({ page }) => {
        await page.goto(`/browser-harness.html?reminder-presentation=1&appearance=${mode}`);
        await expect(page.locator("html")).toHaveAttribute("data-appearance", mode);
        const outlet = page.locator(".reminder-floating");
        const notices = outlet.locator(".reminder");
        await expect(notices).toHaveCount(6);
        const styles = await notices.evaluateAll((elements) => elements.map((element) => {
            const style = getComputedStyle(element);
            return { color: style.color, background: style.backgroundColor, radius: style.borderRadius, padding: style.padding, gap: style.gap, width: element.getBoundingClientRect().width };
        }));
        expect(new Set(styles.map((style) => JSON.stringify(style))).size).toBe(1);
        expect(styles[0]!.background).not.toBe("rgba(0, 0, 0, 0)");
        for (let count = 6; count > 0; count -= 1) {
            await expect(notices.first().getByRole("button", { name: "Dismiss notification", exact: true })).toBeVisible();
            await notices.first().getByRole("button", { name: "Dismiss notification", exact: true }).click();
            await expect(notices).toHaveCount(count - 1);
            if (count === 2) expect((await outlet.boundingBox())!.height).toBe((await notices.first().boundingBox())!.height);
        }
        await expect(outlet).toBeHidden();
        await expect(page.getByLabel("Failure state", { exact: true })).toHaveText("permission_denied");
        await expect(page.locator(".inline-evidence")).toContainText("Required review information");
        await expect(page.locator(".inline-evidence").getByRole("button", { name: "Dismiss notification", exact: true })).toHaveCount(0);
    });
}

test("closing callback-free notifications survives rerenders and modal moves while a new failure can notify again", async ({ page }) => {
    await page.goto("/browser-harness.html?reminder-presentation=1");
    const outlet = page.locator(".reminder-floating");
    await expect(outlet.locator(".reminder")).toHaveCount(6);
    for (let count = 6; count > 0; count -= 1) await outlet.getByRole("button", { name: "Dismiss notification", exact: true }).first().click();
    await expect(outlet).toBeHidden();
    await page.getByRole("button", { name: "Render again", exact: true }).click();
    await expect(page.getByLabel("Render count", { exact: true })).toHaveText("1");
    await expect(outlet).toBeHidden();
    await page.getByRole("button", { name: "Open modal", exact: true }).click();
    const modal = page.locator("md-dialog[open]");
    await expect(modal.locator(".reminder-floating")).toHaveCount(1);
    await expect(outlet).toBeHidden();
    await modal.getByRole("button", { name: "Fail inside modal", exact: true }).click();
    await expect(outlet.getByRole("alert")).toHaveCount(1);
    await expect(outlet).toContainText("Failed request");
    await outlet.getByRole("button", { name: "Dismiss notification", exact: true }).click();
    await expect(outlet).toBeHidden();
    await modal.getByRole("button", { name: "Close modal", exact: true }).click();
    await expect(page.locator("body > .reminder-floating")).toHaveCount(1);
    await expect(outlet).toBeHidden();
    for (const kind of ["success", "failure"]) {
        for (let attempt = 0; attempt < 2; attempt += 1) {
            await page.getByRole("button", { name: `Publish ${kind}`, exact: true }).click();
            await expect(outlet.locator(".reminder")).toHaveCount(1);
            await outlet.getByRole("button", { name: "Dismiss notification", exact: true }).click();
            await expect(outlet).toBeHidden();
        }
    }
});
