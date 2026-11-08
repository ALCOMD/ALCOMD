import { expect, test } from "@playwright/test";
import type { CandidateSource, PackageCandidateSummary } from "@alcomd/sdk";
import { orderPackageWorkspaceRows } from "../../src/package-workspace-order";

function row(packageId: string, sources: string[], installed = false) {
    return {
        packageId,
        ...(installed ? { installedVersion: "1.0.0" } : {}),
        evidence: { providers: sources.map((id) => {
            const source: CandidateSource = id === "user" ? { kind: "user_package", user_package_id: id } : { kind: "repository", repository_id: id };
            return { source, sourceRevision: 1, sourceKind: id === "user" ? "user-package" as const : "remote" as const, displayOrder: ({ official: 0, curated: 1, user: 2, third: 3, fourth: 4 } as Record<string, number>)[id] ?? 5 };
        }) }
    };
}



test("Installed groups follow official, curated, user and configured repositories, retaining source enumeration", () => {
    // IDs and repository IDs deliberately disagree with the desired display order.
    const available = [row("a.fourth", ["fourth"]), row("b.third", ["third"]), row("c.user", ["user"]),
        row("d.curated", ["curated"]), row("z.official", ["official"]), row("e.official", ["official"])];
    const installed = available.map((entry) => ({ ...entry, packageId: "installed." + entry.packageId, installedVersion: "1.0.0" }));
    const input = [...available, ...installed];
    const ordered = orderPackageWorkspaceRows(input);
    const sourceOrder = ["z.official", "e.official", "d.curated", "c.user", "b.third", "a.fourth"];
    expect(ordered.map((entry) => entry.packageId)).toEqual([...sourceOrder.map((id) => "installed." + id), ...sourceOrder]);
    expect(input).toEqual([...available, ...installed]);
    expect(ordered[0]).toBe(installed[4]);
});

test("Shared packages keep their earliest visible provider regardless of selected candidate source", () => {
    const input = [row("third", ["third"]), row("user", ["user"]),
        { ...row("shared", ["fourth", "user", "official"]), selectedSource: "fourth" }, row("curated", ["curated"])];
    expect(orderPackageWorkspaceRows(input).map((entry) => entry.packageId)).toEqual(["shared", "curated", "user", "third"]);
    const hiddenOfficial = input.map((entry) => entry.packageId === "shared" ? row("shared", ["fourth", "user"]) : entry);
    expect(orderPackageWorkspaceRows(hiddenOfficial).map((entry) => entry.packageId)).toEqual(["curated", "user", "shared", "third"]);
});

test("Unlisted providers and missing-source packages retain their order after known sources within installed groups", () => {
    const input = [row("missing.available", []), row("missing.installed", [], true), row("unlisted", ["unlisted"]),
        row("known", ["fourth"]), row("missing.second", []), row("missing.second.installed", [], true)];
    expect(orderPackageWorkspaceRows(input).map((entry) => entry.packageId)).toEqual([
        "missing.installed", "missing.second.installed", "known", "unlisted", "missing.available", "missing.second"
    ]);
    const withoutEvidence: Array<{ packageId: string; installedVersion?: string; evidence?: Pick<PackageCandidateSummary, "providers"> }> = [
        { packageId: "requested" }, { packageId: "locked", installedVersion: "1.0.0" }
    ];
    expect(orderPackageWorkspaceRows(withoutEvidence).map((entry) => entry.packageId)).toEqual(["locked", "requested"]);
    expect(orderPackageWorkspaceRows([])).toEqual([]);
});

test("Package search and status filters preserve installed-first source ordering across candidate pages", async ({ page }) => {
    const projectId = "00000000-0000-4000-8000-000000000101";
    await page.goto(`/browser-harness.html?route=${encodeURIComponent(`/projects/${projectId}`)}&state=package-multiple&paged=1&reverse-candidate-order=1`);
    const table = page.getByRole("table", { name: "Packages", exact: true });
    const rows = table.locator("tbody tr");
    const expected = ["Avatar tools", "Remote tools", "Local tools"];
    await expect(rows).toHaveCount(3);
    await expect(rows).toContainText(expected);
    await page.getByRole("searchbox", { name: "Search packages" }).fill("tools");
    await expect(rows).toHaveCount(3);
    await expect(rows).toContainText(expected);
    await expect.poll(() => page.evaluate(() => window.candidateRequests.some((request) => request.view.kind === "summary" && request.view.filter?.search === "tools" && request.cursor !== undefined))).toBe(true);
    await page.getByRole("button", { name: "Filter packages" }).click();
    await page.getByRole("combobox", { name: "Status", exact: true }).click();
    await page.getByRole("option", { name: "Available", exact: true }).click();
    await page.keyboard.press("Escape");
    await expect(rows).toHaveCount(2);
    await expect(rows).toContainText(["Remote tools", "Local tools"]);
    expect(await page.evaluate(() => window.repositoryPackageRequests)).toEqual([]);
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);
    expect(await page.evaluate(() => window.settingsWrites)).toEqual([]);
});
