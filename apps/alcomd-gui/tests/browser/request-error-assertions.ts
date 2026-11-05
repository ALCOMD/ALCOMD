import { expect, type Locator } from "@playwright/test";
import { requestErrorMessage } from "../../src/request-errors";

export async function expectRequestError(target: Locator, code: string) {
    await expect(target).toContainText(requestErrorMessage(code));
    await expect(target).toContainText(code);
}
