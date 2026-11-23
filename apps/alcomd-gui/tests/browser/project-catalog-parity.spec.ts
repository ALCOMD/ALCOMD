import { expect, test } from "@playwright/test";
import { selectActionMenuItem } from "./material-menu-actions";

test("registering relative to the default directory sends the original source and key", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/projects");
    await selectActionMenuItem(page.getByRole("button", { name: "More project creation options" }), page.getByRole("menuitem", { name: "Add existing project" }));
    const form = page.locator("md-dialog[open]").filter({ hasText: "Register this project?" });
    await form.getByRole("checkbox", { name: "Relative to default project directory" }).check();
    await form.getByRole("textbox", { name: "Project location" }).fill("Group/Example");
    await form.getByRole("button", { name: "Confirm" }).click();
    await expect.poll(() => page.evaluate(() => window.resourceRequests.find(request => request.method === "projects.register")?.params)).toEqual({
        source: { kind: "default", relativePath: "Group/Example" }, idempotencyKey: expect.any(String),
    });
});

test("a mixed package plan keeps intent order and leaves the default version, source and policy absent", async ({ page }) => {
    const project = "00000000-0000-4000-8000-000000000101";
    await page.goto(`/browser-harness.html?route=${encodeURIComponent("/projects/" + project)}&state=package-multiple`);
    await selectActionMenuItem(page.getByRole("button", { name: "More package actions" }), page.getByRole("menuitem", { name: "Custom package plan" }));
    const form = page.locator("md-dialog[open]").filter({ hasText: "Custom package plan" });
    await form.getByRole("combobox", { name: "Plan kind" }).click();
    await page.getByRole("option", { name: "Mixed package changes" }).click();
    await form.getByRole("textbox", { name: "Package ID" }).fill("com.example.new");
    await form.getByRole("button", { name: "Add package" }).click();
    await form.getByRole("combobox", { name: "Package action" }).click();
    await page.getByRole("option", { name: "Remove", exact: true }).click();
    await form.getByRole("textbox", { name: "Package ID" }).fill("com.example.avatar");
    await form.getByRole("button", { name: "Review package plan" }).click();
    await expect.poll(() => page.evaluate(() => window.packageRequests)).toEqual([{ method: "bulk", params: {
        projectId: project, expectedRevision: 2, intents: [{ kind: "install", packageId: "com.example.new" }, { kind: "remove", packageId: "com.example.avatar" }]
    } }]);
});

test("custom package install sends a range and an explicit stable-only override to Core", async ({ page }) => {
    const project = "00000000-0000-4000-8000-000000000101";
    await page.goto(`/browser-harness.html?route=${encodeURIComponent("/projects/" + project)}&state=package-multiple`);
    await selectActionMenuItem(page.getByRole("button", { name: "More package actions" }), page.getByRole("menuitem", { name: "Custom package plan" }));
    const form = page.locator("md-dialog[open]").filter({ hasText: "Custom package plan" });
    await form.getByRole("textbox", { name: "Package ID" }).fill("com.example.avatar");
    await form.getByRole("textbox", { name: "Version range" }).fill("^1.2.0");
    await form.getByRole("combobox", { name: "Prerelease policy" }).click();
    await page.getByRole("option", { name: "Stable versions only" }).click();
    await form.getByRole("button", { name: "Review package plan" }).click();
    await expect.poll(() => page.evaluate(() => window.packageRequests)).toEqual([{ method: "install", params: {
        projectId: project, expectedRevision: 2, packageId: "com.example.avatar", versionRange: "^1.2.0", includePrerelease: false
    } }]);
});

test("project inspection, refresh and frozen plan lookup send the selected inputs", async ({ page }) => {
    await page.goto("/browser-harness.html?route=%2Fprojects&state=ready");
    const more = page.getByRole("button", { name: "More project creation options" });
    await selectActionMenuItem(more, page.getByRole("menuitem", { name: "Inspect project" }));
    const inspection = page.locator("md-dialog[open]").filter({ hasText: "Inspect project" });
    await inspection.getByRole("textbox", { name: "Project location" }).fill("C:\\Projects\\Sample\\Assets");
    await inspection.getByRole("checkbox", { name: "Search parent folders" }).check();
    await inspection.getByRole("button", { name: "Inspect project" }).click();
    await expect(inspection.getByRole("status")).toContainText("2022.");
    expect(await page.evaluate(() => window.resourceRequests.find((r) => r.method === "projects.inspect")?.params)).toEqual({
        path: "C:\\Projects\\Sample\\Assets", discoveryMode: "search-parents"
    });
    await inspection.getByRole("button", { name: "Close" }).click();
    await selectActionMenuItem(page.getByRole("button", { name: /^More actions for/ }).first(), page.getByRole("menuitem", { name: "Refresh project" }));
    await expect.poll(() => page.evaluate(() => window.resourceRequests.find((r) => r.method === "projects.refresh")?.params)).toEqual({
        projectId: "00000000-0000-4000-8000-000000000101", expectedRevision: 2, idempotencyKey: expect.any(String)
    });
    await selectActionMenuItem(more, page.getByRole("menuitem", { name: "Look up plan" }));
    const lookup = page.locator("md-dialog[open]").filter({ hasText: "Look up plan" });
    const id = "00000000-0000-4000-8000-000000000106";
    await lookup.getByRole("textbox", { name: "Plan ID" }).fill(id);
    await lookup.getByRole("button", { name: "Look up plan" }).click();
    await expect(lookup.getByRole("status")).toContainText("com.example.avatar");
    expect(await page.evaluate(() => window.resourceRequests.find((r) => r.method === "plans.get")?.params)).toEqual({ kind: "package", planId: id });
});
