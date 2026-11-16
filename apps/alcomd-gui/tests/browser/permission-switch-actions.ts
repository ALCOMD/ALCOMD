import { expect, type Page } from "@playwright/test";

/** Wait for the write and Core readback, not the switch's optimistic native input. */
export async function confirmPermissionChange(page: Page, method: "extensions.setGrant" | "extensions.revokeGrant", action: () => Promise<void>) {
    await page.evaluate((method) => window.h4Management.hold(method, true), method);
    try {
        await action();
        await expect.poll(() => page.evaluate((method) => window.h4Management.pending.filter((request) => request.method === method).length, method)).toBe(1);
        await page.evaluate((method) => {
            window.h4Management.hold("extensions.permissions.list", true);
            window.h4Management.hold(method, false);
            window.h4Management.release(method);
        }, method);
        await expect.poll(() => page.evaluate(() => window.h4Management.pending.filter((request) => request.method === "extensions.permissions.list").length)).toBeGreaterThan(0);
        await expect(page.locator(".extension-permissions").getByRole("switch")).toHaveCount(0);
    } finally {
        await page.evaluate((method) => {
            window.h4Management.hold(method, false);
            window.h4Management.release(method);
            window.h4Management.hold("extensions.permissions.list", false);
            while (window.h4Management.release("extensions.permissions.list")) { /* Release only held authority reads. */ }
        }, method);
    }
}
