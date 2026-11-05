import { expect, test } from "@playwright/test";

const projectId = "00000000-0000-4000-8000-000000000101";

for (const route of ["/projects", "/projects/" + projectId + "/packages"]) {
    test("desktop window events change the Unity action in " + route, async ({ page }) => {
        await page.goto("/browser-harness.html?route=" + encodeURIComponent(route) + "&state=unity-automatic&unityWindows=1");
        const controls = route === "/projects" ? page.locator(".project-row-actions").first() : page.getByRole("navigation", { name: "Project actions" });
        const open = controls.getByRole("button", { name: "Open Unity", exact: true });
        await expect(open).toBeEnabled();
        await expect.poll(() => page.evaluate(() => window.unityWindowEvidence?.reads)).toBe(1);
        const width = (await open.boundingBox())?.width;
        await page.clock.install();
        await page.clock.fastForward(20_000);
        expect(await page.evaluate(() => ({ reads: window.unityWindowEvidence?.reads, starts: window.unityWindowEvidence?.starts }))).toEqual({ reads: 1, starts: 1 });

        await page.evaluate((id) => window.unityWindowEvidence!.set(id, "opening"), projectId);
        const opening = controls.getByRole("button", { name: "Opening…", exact: true });
        await expect(opening).toBeDisabled();
        expect((await opening.boundingBox())?.width).toBe(width);

        await page.evaluate((id) => window.unityWindowEvidence!.set(id, "ready"), projectId);
        const front = controls.getByRole("button", { name: "Bring to front", exact: true });
        await expect(front).toBeEnabled();
        expect((await front.boundingBox())?.width).toBe(width);
        await page.evaluate(() => {
            const windows = window.unityWindowEvidence!;
            const focus = windows.focus.bind(windows);
            windows.focus = async (id, revision) => {
                const completion = new Promise<void>((resolve) => {
                    window.addEventListener("test-unity-focus-complete", () => resolve(), { once: true });
                });
                await focus(id, revision);
                await completion;
            };
        });
        await front.click();
        await expect.poll(() => page.evaluate(() => window.unityWindowEvidence?.focused)).toEqual([{ projectId, revision: 2 }]);
        await expect(front).toBeEnabled();
        await front.click();
        expect(await page.evaluate(() => window.unityWindowEvidence?.focused)).toEqual([{ projectId, revision: 2 }]);
        await page.evaluate(() => { window.dispatchEvent(new Event("test-unity-focus-complete")); });
        await front.click();
        await expect.poll(() => page.evaluate(() => window.unityWindowEvidence?.focused.length)).toBe(2);
        await page.evaluate(() => { window.dispatchEvent(new Event("test-unity-focus-complete")); });
        expect(await page.evaluate(() => window.unityRequests.filter((request) => request.method === "unity.launch"))).toEqual([]);
        await page.evaluate((id) => window.unityWindowEvidence!.set(id, "closed"), projectId);
        await expect(open).toBeEnabled();
    });
}

test("a denied foreground action reports the Windows refusal without launching", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/projects&state=unity-automatic&unityWindows=1&unityWindowsReject=1");
    const controls = page.locator(".project-row-actions").first();
    await expect.poll(() => page.evaluate(() => window.unityWindowEvidence?.reads)).toBe(1);
    await page.evaluate((id) => window.unityWindowEvidence!.set(id, "ready"), projectId);
    await controls.getByRole("button", { name: "Bring to front", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    await expect(page.getByRole("dialog", { name: "Could not bring Unity to the front" })).toBeVisible();
    await expect(dialog.getByText("Windows did not allow the Unity window to move to the front.", { exact: true })).toBeVisible();
    expect(await page.evaluate(() => window.unityRequests.filter((request) => request.method === "unity.launch"))).toEqual([]);
    await dialog.getByRole("button", { name: "Close", exact: true }).click();
    await expect(dialog).not.toBeVisible();
});
