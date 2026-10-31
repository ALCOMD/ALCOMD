import { expect, test, type Page } from "@playwright/test";
import type { Operation } from "../../src/core-models";

test.beforeEach(async ({ page }) => {
    await page.goto("/tests/browser/utility-workspace.html?operationFollow=1");
    await expect.poll(() => page.evaluate(() => window.followReads.length)).toBe(1);
});

test("operation follow rejects a late DTO after changing the owned task", async ({ page }) => {
    await page.getByRole("button", { name: "Switch task", exact: true }).click();
    await expect.poll(() => page.evaluate(() => window.followReads.length)).toBe(2);
    await resolve(page, 1, "running");
    await resolve(page, 0, "succeeded");
    await expect(page.locator(".operation-follow strong")).toHaveText("running");
    expect(await page.evaluate(() => window.followTerminals)).toEqual([]);
    await expect.poll(() => page.evaluate(() => window.followReads.length)).toBe(3);
    expect(await page.evaluate(() => window.followReads[2]!.operationId)).toBe("00000000-0000-4000-8000-000000000502");
    await resolve(page, 2, "succeeded");
    await expect.poll(() => page.evaluate(() => window.followTerminals.length)).toBe(1);
});

test("operation follow binds in-flight results to the current client and stops on route cleanup", async ({ page }) => {
    await page.getByRole("button", { name: "Switch client", exact: true }).click();
    await expect.poll(() => page.evaluate(() => window.followReads.length)).toBe(2);
    await resolve(page, 1, "running");
    await resolve(page, 0, "succeeded");
    await expect(page.locator(".operation-follow strong")).toHaveText("running");
    expect(await page.evaluate(() => window.followTerminals)).toEqual([]);
    await expect.poll(() => page.evaluate(() => window.followReads.length)).toBe(3);
    await page.getByRole("button", { name: "Leave page", exact: true }).click();
    await resolve(page, 2, "succeeded");
    await expect(page.getByRole("status")).toHaveCount(0);
    expect(await page.evaluate(() => window.followTerminals)).toEqual([]);
});

test("operation follow retries the same known ID after an initial failure or a mismatched response", async ({ page }) => {
    await page.evaluate(() => window.followReads[0]!.reject({ code: "daemon_unavailable" }));
    await expect(page.getByRole("status")).toContainText("temporarily unavailable");
    await expect.poll(() => page.evaluate(() => window.followReads.length)).toBe(2);
    await resolve(page, 1, "succeeded", "00000000-0000-4000-8000-000000000599");
    expect(await page.evaluate(() => window.followTerminals)).toEqual([]);
    await expect.poll(() => page.evaluate(() => window.followReads.length)).toBe(3);
    expect(await page.evaluate(() => window.followReads.map((read) => read.operationId))).toEqual(Array(3).fill("00000000-0000-4000-8000-000000000501"));
    await resolve(page, 2, "succeeded");
    await expect.poll(() => page.evaluate(() => window.followTerminals.length)).toBe(1);
    await expect(page.getByRole("status")).not.toContainText("temporarily unavailable");
});

async function resolve(page: Page, index: number, state: string, operationId?: string) {
    await page.evaluate(({ index, state, operationId }) => {
        const read = window.followReads[index]!;
        const value: Operation = { operationId: operationId ?? read.operationId, kind: "templates.import", state, revision: 2, createdAtMs: 1_700_000_000_000, updatedAtMs: 1_700_000_001_000 };
        read.resolve(value);
    }, { index, state, operationId });
}
