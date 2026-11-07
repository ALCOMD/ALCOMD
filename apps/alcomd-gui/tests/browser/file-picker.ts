import { expect, type Locator, type Page } from "@playwright/test";

export async function openFilePicker(page: Page, owner: Locator) {
    await owner.getByRole("button", { name: "Select", exact: true }).click();
    const picker = page.locator("md-dialog[open]").filter({ has: page.getByRole("heading", { name: "Select file", exact: true }) });
    await expect(picker).toBeVisible();
    const actions = picker.locator('[slot="actions"]').getByRole("button");
    await expect(actions).toHaveCount(2);
    await expect(actions.nth(0)).toHaveAccessibleName("Cancel");
    await expect(actions.nth(1)).toHaveAccessibleName("Confirm");
    return picker;
}

export function browseForFile(picker: Locator) {
    return picker.getByRole("button", { name: "Drop a file here or click to select", exact: true });
}

export async function confirmFile(picker: Locator) {
    await picker.getByRole("button", { name: "Confirm", exact: true }).click();
    await expect(picker).toHaveCount(0);
}
