import { expect, type Locator } from "@playwright/test";
import { requestErrorMessage } from "../../src/request-errors";

export async function expectRequestError(target: Locator, code: string) {
    await expect(target).toContainText(requestErrorMessage(code));
    const expand = target.getByRole("button", { name: "Show details", exact: true });
    if (await expand.isVisible()) await expand.click();
    await expect(target).toContainText(code);
}
