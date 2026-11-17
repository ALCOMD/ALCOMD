import { openLocalExtensionInstall } from "./extension-install-entry";
import { expect, test } from "@playwright/test";
import { createTranslator } from "../../src/i18n";
import { normalizeRequestError, requestErrorMessage } from "../../src/request-errors";

for (const locale of ["en-US", "zh-CN", "ja-JP"] as const) {
    test(`readable execution errors preserve context and hide private messages in ${locale}`, async ({ page }) => {
        const t = createTranslator(locale);
        await page.goto(`/browser-harness.html?route=/extensions&h4-management=1&locale=${locale}`);
        await openLocalExtensionInstall(page, t);
        const dialog = page.locator("md-dialog[open]");
        await dialog.getByLabel(t("Extension package"), { exact: true }).fill("C:\\Fixture\\sample.alcomdextension");
        await page.evaluate(() => window.h4Management.failNext("extensions.planInstall", "extension_signature_invalid"));
        await dialog.getByRole("button", { name: t("Install extension"), exact: true }).click();
        const alert = dialog.getByRole("alert");
        await expect(alert).toContainText(t(requestErrorMessage("extension_signature_invalid")));
        await expect(alert).toContainText("extension_signature_invalid");
        await expect(page.locator("body")).not.toContainText("Fixture request refused.");
        await expect(dialog.getByRole("checkbox")).toHaveCount(0);
        expect(await page.evaluate(() => window.h4Management.requests.filter(value => value.method === "extensions.planInstall").length)).toBe(1);
    });
}

test("request error normalization retains only bounded identifiers and never trusts raw messages", () => {
    expect(normalizeRequestError({ code: "extension_signature_invalid", message: "secret", diagnosticId: "test-id" })).toMatchObject({ code: "extension_signature_invalid", diagnosticId: "test-id" });
    expect(JSON.stringify(normalizeRequestError({ code: "permission_denied", message: "secret" }))).not.toContain("secret");
    expect(normalizeRequestError(new Error("secret")).code).toBe("internal_error");
    for (const code of ["extension_signature_invalid", "extension_signature_invalid", "permission_denied", "revision_conflict", "extension_ui_session_stale", "repository_unavailable", "template_builtin_immutable", "authentication_not_supported"]) {
        expect(requestErrorMessage(code)).not.toBe(requestErrorMessage("unrecognized_future_error"));
        for (const locale of ["zh-CN", "ja-JP"] as const) expect(createTranslator(locale)(requestErrorMessage(code))).not.toBe(requestErrorMessage(code));
    }
});

test("install has no publisher approval and a failed package check can be retried without automatic apply", async ({ page }) => {
    await page.goto("/browser-harness.html?route=%2Fextensions&state=ready&h4-management=1");
    await openLocalExtensionInstall(page);
    const dialog = page.locator("md-dialog[open]");
    await dialog.getByLabel("Extension package", { exact: true }).fill("C:\\Fixture\\sample.alcomdextension");
    await expect(dialog.getByRole("checkbox")).toHaveCount(0);
    await page.evaluate(() => window.h4Management.failNext("extensions.planInstall", "extension_signature_invalid"));
    await dialog.getByRole("button", { name: "Install extension", exact: true }).click();
    await expect(dialog.getByRole("alert")).toContainText(requestErrorMessage("extension_signature_invalid"));
    expect(await page.evaluate(() => window.h4Management.requests.filter((value) => value.method === "extensions.applyInstall"))).toHaveLength(0);
    await dialog.getByRole("button", { name: "Install extension", exact: true }).click();
    await expect(dialog.getByRole("heading", { name: "Required permissions", exact: true })).toBeVisible();
    await expect(dialog.getByRole("alert")).toHaveCount(0);
    expect(await page.evaluate(() => window.h4Management.requests.filter((value) => value.method === "extensions.planInstall").map((value) => value.params))).toEqual([
        { packagePath: "C:\\Fixture\\sample.alcomdextension", expectedRevision: 0 },
        { packagePath: "C:\\Fixture\\sample.alcomdextension", expectedRevision: 0 },
    ]);
    await expect(dialog.getByText("Trust", { exact: true })).toHaveCount(0);
    await expect(dialog.getByRole("region", { name: "Requested permissions", exact: true })).toBeVisible();
    expect(await page.evaluate(() => window.h4Management.requests.filter((value) => value.method === "extensions.applyInstall"))).toHaveLength(0);
});
