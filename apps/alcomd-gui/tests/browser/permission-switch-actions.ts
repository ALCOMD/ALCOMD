import { expect, type Page } from "@playwright/test";

/** Wait for the write and Core readback, not the switch's optimistic native input. */
export async function confirmPermissionChange(page: Page, method: "extensions.setGrant" | "extensions.revokeGrant", action: () => Promise<void>) {
    const switches = page.locator(".extension-permissions").getByRole("switch");
    const count = await switches.count();
    expect(count).toBeGreaterThan(0);
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
        await expect(switches).toHaveCount(count);
        for (const control of await switches.all()) await expect(control).toBeDisabled();
    } finally {
        await page.evaluate((method) => {
            window.h4Management.hold(method, false);
            window.h4Management.release(method);
            window.h4Management.hold("extensions.permissions.list", false);
            while (window.h4Management.release("extensions.permissions.list")) { /* Release only held authority reads. */ }
        }, method);
    }
}
