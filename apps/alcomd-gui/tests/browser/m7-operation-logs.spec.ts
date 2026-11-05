import type { OperationLogEntry } from "@alcomd/sdk";
import { expect, test } from "@playwright/test";

const fixtureRecord = (sequence: number, actor: string): OperationLogEntry => ({
    sequence, actor, entryId: `11111111-1111-4111-8111-${String(sequence).padStart(12, "0")}`,
    executionId: "22222222-2222-4222-8222-222222222222", occurredAtMs: 1793937600000 + sequence,
    action: "projects.register", step: "inspect_project", outcome: "succeeded",
    target: { kind: "project", name: `Fixture project ${sequence}`, path: `C:\\Isolated\\项目 ${sequence}` }
});

test("Core change adds records with their initiator; idle time does not read again", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/projects&state=ready");
    const primary = page.getByRole("navigation", { name: "Primary", exact: true });
    const entry = primary.locator('[title="Settings"] + [title="Logs"]');
    await entry.click();
    await expect(page.getByRole("heading", { name: "Logs", exact: true })).toBeVisible();
    await expect(entry).toHaveAttribute("data-aria-current", "page");
    await expect(page.getByText("No logs yet", { exact: true })).toBeVisible();
    await page.evaluate((records) => window.fixtureLogs.replace(records), [fixtureRecord(1, "GUI"), fixtureRecord(2, "CLI"), fixtureRecord(3, "MCP")]);
    const log = page.getByRole("table", { name: "Logs", exact: true });
    await expect(log.locator("tbody tr")).toHaveCount(3);
    for (const [sequence, actor] of [[1, "GUI"], [2, "CLI"], [3, "MCP"]] as const) {
        await expect(log.locator("tr").filter({ hasText: `Fixture project ${sequence}` })).toContainText(actor);
    }
    const before = await page.evaluate(() => window.fixtureLogs.reads.length);
    await page.clock.install();
    await page.clock.fastForward(12_000);
    expect(await page.evaluate(() => window.fixtureLogs.reads.length)).toBe(before);
});

test("scrolling automatically reads older history on the same page", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/logs&state=ready");
    await expect(page.getByText("No logs yet", { exact: true })).toBeVisible();
    const records = Array.from({ length: 126 }, (_, index) => fixtureRecord(index + 1, "CLI"));
    await page.evaluate((entries) => window.fixtureLogs.replace(entries), records);
    const table = page.getByRole("table", { name: "Logs", exact: true });
    await expect(table.locator("tbody tr")).toHaveCount(100);
    const viewport = page.locator(".operation-log-list [data-overlayscrollbars-viewport]").first();
    await viewport.evaluate((element) => { element.scrollTop = element.scrollHeight; });
    await expect(table.locator("tbody tr")).toHaveCount(126);
    await expect(table.getByText("Fixture project 1", { exact: true })).toHaveCount(1);
    const cursors = await page.evaluate(() => window.fixtureLogs.reads.map((request) => request.beforeSequence));
    expect(cursors).toContain(27);
    await expect(page.getByRole("tab")).toHaveCount(0);
    await expect(page.getByRole("button", { name: "Load more", exact: true })).toHaveCount(0);
});
