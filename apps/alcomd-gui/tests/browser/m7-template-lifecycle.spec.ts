import { expect, test, type Page } from "@playwright/test";
import { expectRequestError } from "./request-error-assertions";

const avatarId = "com.vrclearn.alcomd.templates.vrchat.avatars";

test("the ordinary form can reproduce both locked declarations and reuse their catalog entries", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/templates");
    for (const [name, kind] of [["Avatar starter", "avatars"], ["World starter", "worlds"]]) {
        const id = "com.vrclearn.alcomd.templates.vrchat." + kind;
        const packageId = "com.vrchat." + kind;
        const dialog = await create(page, id, name!);
        await dialog.getByLabel("Unity version series", { exact: true }).fill("2022.3");
        await dialog.getByLabel("Unity version presets", { exact: true }).fill("2022.3.22f1, 2022.3.6f1, 2019.4.31f1");
        const dependencies = dialog.getByRole("group", { name: "VPM dependencies", exact: true });
        await dependencies.getByRole("button", { name: "Add dependency", exact: true }).click();
        await dependencies.getByLabel("Package ID", { exact: true }).fill(packageId);
        await dependencies.getByLabel("Version range", { exact: true }).fill("*");
        const constraints = dialog.getByRole("group", { name: "Package constraints by Unity version", exact: true });
        for (const [index, constraint] of ["com.vrchat.base", packageId].entries()) {
            await constraints.getByRole("button", { name: "Add constraint", exact: true }).click();
            await constraints.getByLabel("Unity version", { exact: true }).nth(index).fill("2019.4.31f1");
            await constraints.getByLabel("Package ID", { exact: true }).nth(index).fill(constraint);
            await constraints.getByLabel("Version range", { exact: true }).nth(index).fill("<=3.4.2");
        }
        await dialog.getByRole("button", { name: "Save template", exact: true }).click();
        await expect(dialog).toHaveCount(0);
        await expect(page.getByRole("row").filter({ hasText: id })).toHaveCount(1);
    }
    expect(await page.evaluate(() => window.templateRequests.filter(({ method }) => method === "projectTemplates.save"))).toHaveLength(2);
});

test("IDs distinguish equal names and user templates can be edited or removed", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/templates");
    const id = "com.example.avatar-tools";
    const dialog = await create(page, id, "Avatar starter");
    await dialog.getByRole("button", { name: "Save template", exact: true }).click();
    const userRow = page.getByRole("row").filter({ hasText: id });
    await expect(userRow).toContainText("Avatar starter");
    await expect(page.getByRole("row").filter({ hasText: avatarId })).toContainText("Avatar starter");
    await userRow.getByRole("button", { name: "More actions for Avatar starter", exact: true }).click();
    await page.getByRole("menuitem", { name: "Edit template", exact: true }).click();
    await expect(dialog.getByLabel("Template ID", { exact: true })).toHaveJSProperty("readOnly", true);
    await dialog.getByLabel("Name", { exact: true }).fill("My avatar tools");
    await dialog.getByRole("button", { name: "Save template", exact: true }).click();
    await expect(userRow).toContainText("My avatar tools");
    await userRow.getByRole("button", { name: "More actions for My avatar tools", exact: true }).click();
    await page.getByRole("menuitem", { name: "Remove template", exact: true }).click();
    await expect(dialog).toContainText(id);
    await dialog.getByRole("button", { name: "Remove template", exact: true }).click();
    await expect(userRow).toHaveCount(0);
    await expect(page.getByRole("row").filter({ hasText: avatarId })).toBeVisible();
});

test("saving different content under an existing ID reports a conflict", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/templates");
    const dialog = await create(page, avatarId, "Unrelated template");
    await dialog.getByRole("button", { name: "Save template", exact: true }).click();
    await expectRequestError(dialog.getByRole("alert"), "template_conflict");
    await expect(dialog.getByLabel("Name", { exact: true })).toHaveValue("Unrelated template");
    await expect(page.getByRole("row").filter({ hasText: avatarId })).toContainText("Avatar starter");
});

async function create(page: Page, id: string, name: string) {
    await page.getByRole("button", { name: "Create template", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    await dialog.getByLabel("Template ID", { exact: true }).fill(id);
    await dialog.getByLabel("Name", { exact: true }).fill(name);
    return dialog;
}
