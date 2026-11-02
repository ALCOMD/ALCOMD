import { expect, type Locator } from "@playwright/test";
import { requestErrorMessage } from "../../src/request-errors";

export async function expectRequestError(target: Locator, code: string) {
    await expect(target).toContainText(requestErrorMessage(code));
    await target.getByRole("button", { name: "Show details", exact: true }).click();
    await expect(target).toContainText(code);
}
