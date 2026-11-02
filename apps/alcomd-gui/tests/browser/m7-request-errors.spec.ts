import { expect, test } from "@playwright/test";
import { createTranslator } from "../../src/i18n";
import { normalizeRequestError, requestErrorMessage } from "../../src/request-errors";

for (const locale of ["en-US", "zh-CN", "ja-JP"] as const) {
    test(`readable request errors preserve context and hide technical details in ${locale}`, async ({ page }) => {
        const t = createTranslator(locale);
        await page.goto(`/browser-harness.html?request-reminder=1&locale=${locale}`);
        const alert = page.getByRole("alert");
        await expect(alert).toContainText(t(requestErrorMessage("extension_publisher_confirmation_required")));
        await expect(alert).not.toContainText("extension_publisher_confirmation_required");
        await expect(page.locator("body")).not.toContainText("do-not-display");
        await alert.getByRole("button", { name: t("Show details"), exact: true }).click();
        await expect(alert).toContainText("extension_publisher_confirmation_required");
        await expect(alert).toContainText("test-diagnostic");
        await alert.getByRole("button", { name: "Change error", exact: true }).click();
        await expect(alert).toContainText(t(requestErrorMessage("extension_signature_invalid")));
        await expect(alert).not.toContainText("extension_signature_invalid");
        await expect(alert).not.toContainText("test-diagnostic");
        await expect(alert.getByRole("button", { name: t("Show details"), exact: true })).toBeVisible();
        await page.getByRole("button", { name: "Unknown error", exact: true }).click();
        await expect(alert).toContainText(t(requestErrorMessage("unrecognized_future_error")));
        await expect(alert).not.toContainText("unrecognized_future_error");
        await alert.getByRole("button", { name: t("Show details"), exact: true }).click();
        await expect(alert).toContainText("unrecognized_future_error");
        await expect(page.locator("body")).not.toContainText("private.invalid");
    });
}

test("request error normalization retains only bounded identifiers and never trusts raw messages", () => {
    expect(normalizeRequestError({ code: "extension_signature_invalid", message: "secret", diagnosticId: "test-id" })).toMatchObject({ code: "extension_signature_invalid", diagnosticId: "test-id" });
    expect(JSON.stringify(normalizeRequestError({ code: "permission_denied", message: "secret" }))).not.toContain("secret");
    expect(normalizeRequestError(new Error("secret")).code).toBe("internal_error");
    for (const code of ["extension_publisher_confirmation_required", "extension_signature_invalid", "permission_denied", "revision_conflict", "extension_ui_session_stale", "repository_unavailable", "template_builtin_immutable", "authentication_not_supported"]) {
        expect(requestErrorMessage(code)).not.toBe(requestErrorMessage("unrecognized_future_error"));
        for (const locale of ["zh-CN", "ja-JP"] as const) expect(createTranslator(locale)(requestErrorMessage(code))).not.toBe(requestErrorMessage(code));
    }
});

test("publisher confirmation feedback explains the checkbox without approving or replaying a request", async ({ page }) => {
    await page.goto("/browser-harness.html?route=%2Fextensions&state=ready&h4-management=1");
    await page.getByRole("button", { name: "Install extension", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    await dialog.getByLabel("Extension package", { exact: true }).fill("C:\\Fixture\\sample.alcomdext");
    await dialog.getByRole("button", { name: "Create install plan", exact: true }).click();
    await expect(dialog.getByRole("alert")).toContainText(requestErrorMessage("extension_publisher_confirmation_required"));
    const approval = dialog.getByLabel("Approve this publisher for this extension", { exact: true });
    await expect(approval).not.toBeChecked();
    expect(await page.evaluate(() => window.h4Management.requests.filter((value) => value.method === "extensions.planInstall").map((value) => value.params.publisherApproval))).toEqual(["none"]);
    await approval.check();
    await expect(dialog.getByRole("alert")).toHaveCount(0);
    await dialog.getByRole("button", { name: "Create install plan", exact: true }).click();
    await expect(dialog.getByRole("heading", { name: "Review extension install", exact: true })).toBeVisible();
    expect(await page.evaluate(() => window.h4Management.requests.filter((value) => value.method === "extensions.planInstall").map((value) => value.params.publisherApproval))).toEqual(["none", "approve_for_extension"]);
    expect(await page.evaluate(() => window.h4Management.requests.filter((value) => value.method === "extensions.applyInstall"))).toHaveLength(0);
});
