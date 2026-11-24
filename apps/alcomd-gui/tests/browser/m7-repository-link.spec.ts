import { expect, test } from "@playwright/test";
import { selectSettingsSection } from "./settings-sections";

test("repository links prefill the shared review, queue and require explicit registration", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/projects&resource-lifecycle=1&repository-link-fixture=1");
    const dialog = page.locator("md-dialog[open]");
    await expect(page.getByRole("dialog", { name: "Add repository", exact: true })).toBeVisible();
    await expect(dialog.getByRole("textbox", { name: "Repository URL", exact: true })).toHaveValue("https://repository.test/index.json");
    await expect(dialog.getByRole("textbox", { name: "Header name", exact: true })).toHaveValue("Authorization");
    await expect(page.locator("md-dialog[open] input[type=password]")).toHaveValue("Bearer fixture-link-token");
    const requests = () => page.evaluate(() => window.resourceRequests.filter(({ method }) => method === "repositories.inspect" || method === "repositories.register"));
    expect(await requests()).toEqual([]);

    await page.evaluate(() => {
        window.dispatchEvent(new CustomEvent("fixture-desktop-link", { detail: { kind: "vccRepository", link: "vcc://vpm/addRepo?url=https://ignored.test&url=https://duplicate.test" } }));
        window.dispatchEvent(new CustomEvent("fixture-desktop-link", { detail: { kind: "vccRepository", link: "vcc://vpm/addRepo?url=http://second.test/index.json&future=ignored&headers%5B%5D=X-API-Key%3Afixture-http-token" } }));
    });
    await dialog.getByRole("button", { name: "Review repository", exact: true }).click();
    await expect(dialog.getByText("Uses authentication", { exact: true })).toBeVisible();
    await expect(dialog).not.toContainText("fixture-link-token");
    expect((await requests()).map(({ method }) => method)).toEqual(["repositories.inspect"]);
    await dialog.getByRole("button", { name: "Add repository", exact: true }).click();

    await expect(dialog.getByRole("textbox", { name: "Repository URL", exact: true })).toHaveValue("http://second.test/index.json");
    await expect(dialog.getByRole("textbox", { name: "Header name", exact: true })).toHaveValue("X-API-Key");
    await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(dialog).toBeHidden();
    expect((await requests()).map(({ method }) => method)).toEqual(["repositories.inspect", "repositories.register"]);
    expect(JSON.stringify(await requests())).not.toContain("fixture-link-token");
});

test("duplicate request headers in a repository link must be corrected before review", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/projects&resource-lifecycle=1&repository-link-fixture=1");
    const dialog = page.locator("md-dialog[open]");
    await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(dialog).toBeHidden();
    await page.evaluate(() => {
        const link = new URL("vcc://vpm/addRepo?url=https://repository.test/index.json");
        link.searchParams.append("headers[]", "Authorization:Bearer first-fixture");
        link.searchParams.append("headers[]", "Authorization:Bearer second-fixture");
        window.dispatchEvent(new CustomEvent("fixture-desktop-link", { detail: { kind: "vccRepository", link: link.href } }));
    });
    const headers = dialog.getByRole("textbox", { name: "Header name", exact: true });
    const review = dialog.getByRole("button", { name: "Review repository", exact: true });
    const duplicate = dialog.getByRole("alert").filter({ hasText: "Request header names must be unique" });
    await expect(headers).toHaveCount(2);
    await expect(duplicate).toBeVisible();
    await headers.nth(1).fill("authorization");
    await expect(review).toBeDisabled();
    await expect(duplicate).toBeVisible();
    await headers.nth(1).fill("X-Token");
    await expect(duplicate).toHaveCount(0);
    await expect(review).toBeEnabled();
    expect(await page.evaluate(() => window.resourceRequests.some(({ method }) => method === "repositories.inspect"))).toBe(false);
});

test("VCC handling defaults on and saves a narrow preference without using the OS", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/settings&vcc-settings-fixture=1");
    const card = page.getByRole("region", { name: "Link protocols", exact: true });
    await expect(card).toBeHidden();
    await selectSettingsSection(page, "associations");
    const enabled = card.getByRole("checkbox", { name: "Handle VCC links", exact: true });
    const register = card.getByRole("button", { name: "Register ALCOMD for VCC links", exact: true });
    await expect(enabled).toBeChecked();
    await expect(register).toBeEnabled();
    await expect(card.getByText("ALCOMD is not registered", { exact: true })).toBeVisible();
    await expect(card.locator(".settings-row").filter({ has: page.getByText("vcc://", { exact: true }) })).toContainText("Another app is the default");
    const files = page.getByRole("region", { name: "File associations", exact: true });
    await expect(files.locator("dt")).toContainText([".alcomdrepositories", ".alcomdtemplate", ".alcomdextension", ".alcomrepositories", ".alcomtemplate"]);
    await expect(files.locator("dd")).toHaveText(["No default app", "ALCOMD is the default", "Default app could not be verified", "Another app is the default", "No default app"]);
    await card.getByRole("button", { name: "Open default apps settings", exact: true }).click();
    await expect.poll(() => page.evaluate(() => window.resourceRequests.filter(({ method }) => method === "gui_open_default_apps_settings").length)).toBe(1);
    await enabled.uncheck();
    await expect(register).toHaveCount(0);
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(1);
    await expect(enabled).toBeEnabled();
    await enabled.check();
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(2);
    await expect(register).toBeEnabled();
    expect(await page.evaluate(() => window.settingsRequests.map(({ update }) => update))).toEqual([
        { integrations: { handleVccLinks: false } },
        { integrations: { handleVccLinks: true } },
    ]);
});

test("platforms with read-only associations still show the local status", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/settings&vcc-settings-fixture=read-only");
    await selectSettingsSection(page, "associations");
    const protocols = page.getByRole("region", { name: "Link protocols", exact: true });
    const alcom = protocols.locator(".settings-row").filter({ has: page.getByText("alcom://", { exact: true }) });
    await expect(alcom).toContainText("ALCOM (vrc-get)");
    await expect(alcom).toContainText("Another app is the default");
    await expect(protocols.getByRole("checkbox", { name: "Handle VCC links" })).toHaveCount(0);
    await expect(protocols.getByRole("button", { name: "Open default apps settings" })).toHaveCount(0);
    const files = page.getByRole("region", { name: "File associations", exact: true });
    await expect(files).toContainText("ALCOM repository list");
    await expect(files).toContainText("ALCOM project template");
});
