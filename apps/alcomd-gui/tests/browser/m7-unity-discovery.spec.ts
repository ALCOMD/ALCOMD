import { selectSettingsSection } from "./settings-sections";
import { expect, test, type Page } from "@playwright/test";
import { expectRequestError } from "./request-error-assertions";

const section = (page: Page) => page.getByRole("region", {name:"Unity installations",exact:true});
async function open(page: Page, query = "h3-unity-discovery=1") {
    await page.setViewportSize({width:1280,height:800});
    await page.goto(`/browser-harness.html?route=/settings&${query}`);
    await selectSettingsSection(page,"unity");
    await expect(section(page).getByRole("button",{name:"Discover and refresh",exact:true})).toBeVisible();
    await expect(section(page).getByRole("button",{name:"Manually select",exact:true})).toBeVisible();
    await expect(page.getByRole("button",{name:"Manage installations",exact:true})).toHaveCount(0);
    return section(page);
}
const discoveryRequests = (page:Page) => page.evaluate(()=>window.unityRequests.filter((request)=>request.method==="unityInstallationsRefresh").length);
const removals = (page:Page) => page.evaluate(()=>window.h3Requests.filter((request)=>request.method==="unity.installations.remove"));
const registrations = (page:Page) => page.evaluate(()=>window.h3Requests.filter((request)=>request.method==="unity.installations.register"));

test("Unity discovery directly populates the list without a management dialog or success popup",async ({page})=>{
    const host=await open(page);
    await expect(host.getByText("No Unity installations registered",{exact:true})).toBeVisible();
    await host.getByRole("button",{name:"Discover and refresh",exact:true}).click();
    await expect(host.getByRole("table").locator("tbody tr")).toHaveCount(1);
    await expect(page.locator("md-dialog[open]")).toHaveCount(0);
    await expect(page.locator(".reminder-floating")).toHaveCount(0);
    expect(await discoveryRequests(page)).toBe(1);
});

test("Unity discovery errors stay inline and an explicit retry clears them",async ({page})=>{
    const host=await open(page,"h3-unity-discovery=1&h3-unity-discovery-error=1");
    await host.getByRole("button",{name:"Discover and refresh",exact:true}).click();
    const feedback=host.getByRole("alert");
    await expect(feedback).toContainText("Unity Editor version could not be verified.");
    await expectRequestError(feedback,"unity_version_unverified");
    await page.clock.install();
    await page.clock.fastForward(6000);
    await expect(feedback).toBeVisible();
    expect(await discoveryRequests(page)).toBe(1);
    await host.getByRole("button",{name:"Discover and refresh",exact:true}).click();
    await expect(feedback).toHaveCount(0);
    await expect(host.getByRole("table").locator("tbody tr")).toHaveCount(1);
    expect(await discoveryRequests(page)).toBe(2);
    expect(await page.evaluate(()=>window.settingsRequests)).toEqual([]);
});

test("pending discovery shows progress and repeated clicks admit one request",async ({page})=>{
    const host=await open(page,"h3-unity-discovery=1&unity-discovery-hold=1");
    await host.getByRole("button",{name:"Discover and refresh",exact:true}).evaluate((element)=>{(element as HTMLElement).click();(element as HTMLElement).click();});
    await expect(host.getByText("Searching for Unity installations…",{exact:true})).toBeVisible();
    await expect(host.getByRole("button",{name:"Manually select",exact:true})).toBeDisabled();
    expect(await discoveryRequests(page)).toBe(1);
    await page.evaluate(()=>window.dispatchEvent(new Event("test-unity-discovery-ready")));
    await expect(host.getByRole("table").locator("tbody tr")).toHaveCount(1);
});

test("manual selection registers the original chosen path and reloads the version list",async ({page})=>{
    const path="\\\\?\\C:\\Editors\\Unity.exe";
    const host=await open(page,"h3-unity-discovery=1&unity-picker-path="+encodeURIComponent(path));
    await host.getByRole("button",{name:"Manually select",exact:true}).click();
    await expect(host.getByRole("table").locator("tbody tr")).toHaveCount(1);
    expect(await registrations(page)).toEqual([{method:"unity.installations.register",params:{executablePath:path}}]);
    await expect(host.getByTitle("C:\\Editors\\Unity.exe",{exact:true})).toBeVisible();
    expect(await discoveryRequests(page)).toBe(0);
    await expect(page.locator("md-dialog[open]")).toHaveCount(0);
});

test("cancelling manual selection preserves the list and makes no registration",async ({page})=>{
    const host=await open(page,"state=ready&unity-picker=cancel&windows-paths=1");
    const before=await host.getByRole("table").innerText();
    await host.getByRole("button",{name:"Manually select",exact:true}).click();
    await expect(host.getByRole("button",{name:"Manually select",exact:true})).toBeEnabled();
    expect(await registrations(page)).toEqual([]);
    expect(await discoveryRequests(page)).toBe(0);
    await expect(host.getByRole("table")).toHaveText(before,{useInnerText:true});
    expect(await page.evaluate(()=>window.nativePickerRequests)).toEqual([{kind:"unityEditor",currentPath:"\\\\?\\C:\\Program Files\\Unity\\Hub\\Editor\\2022.3.22f1\\Editor\\Unity.exe"}]);
});

test("leaving while a manual picker is pending discards the late selection",async ({page})=>{
    const host=await open(page,"unity-picker=hold");
    const choose=host.getByRole("button",{name:"Manually select",exact:true});
    await choose.evaluate((element)=>{(element as HTMLElement).click();(element as HTMLElement).click();});
    await expect(host.getByText("Selecting Unity Editor…",{exact:true})).toBeVisible();
    expect(await page.evaluate(()=>window.nativePickerRequests.length)).toBe(1);
    await page.locator("#primary-navigation md-list-item").filter({has:page.locator(".navigation-item-label",{hasText:/^Projects$/})}).click();
    await expect(page.getByRole("heading",{name:"Projects",exact:true})).toBeVisible();
    await page.evaluate(()=>window.dispatchEvent(new Event("test-unity-picker-ready")));
    expect(await registrations(page)).toEqual([]);
});

test("row removal confirms the exact Editor and only unregisters that installation",async ({page})=>{
    const host=await open(page,"state=unity-multiple&windows-paths=1");
    const rows=host.getByRole("table").locator("tbody tr");
    await expect(rows).toHaveCount(2);
    await rows.nth(1).getByRole("button",{name:"Remove",exact:true}).click();
    let dialog=page.locator("md-dialog[open]");
    await expect(dialog).toContainText("Only the ALCOMD registry entry is removed. The editor remains installed.");
    await expect(dialog).toContainText("\\\\builds\\Editors\\Unity.exe");
    expect(await removals(page)).toEqual([]);
    await dialog.getByRole("button",{name:"Cancel",exact:true}).click();
    await expect(rows).toHaveCount(2);
    await rows.nth(1).getByRole("button",{name:"Remove",exact:true}).click();
    dialog=page.locator("md-dialog[open]");
    await dialog.getByRole("button",{name:"Remove installation",exact:true}).click();
    await expect(dialog).toHaveCount(0);
    await expect(rows).toHaveCount(1);
    expect(await removals(page)).toEqual([{method:"unity.installations.remove",params:{installationId:"00000000-0000-4000-8000-000000000111",expectedRevision:2}}]);
    await expect(rows.first()).toContainText("C:\\Program Files\\Unity");
    await expect(host.getByRole("heading",{name:"Unity installations",exact:true})).toBeFocused();
    expect(await page.evaluate(()=>window.settingsRequests)).toEqual([]);
});

test("a refused row removal preserves the record and retries only after confirmation",async ({page})=>{
    const host=await open(page,"state=ready&unity-remove=error");
    await host.getByRole("button",{name:"Remove",exact:true}).click();
    const dialog=page.locator("md-dialog[open]");
    await dialog.getByRole("button",{name:"Remove installation",exact:true}).click();
    await expectRequestError(dialog.getByRole("alert"),"revision_conflict");
    await expect(host.getByRole("table").locator("tbody tr")).toHaveCount(1);
    expect((await removals(page)).length).toBe(1);
    await dialog.getByRole("button",{name:"Cancel",exact:true}).click();
    await host.getByRole("button",{name:"Remove",exact:true}).click();
    await dialog.getByRole("button",{name:"Remove installation",exact:true}).click();
    await expect(host.getByText("No Unity installations registered",{exact:true})).toBeVisible();
    expect((await removals(page)).length).toBe(2);
});

test("Unity installation controls fit a narrow settings panel",async ({page},testInfo)=>{
    const host=await open(page,"state=ready&windows-paths=1");
    await page.screenshot({path:testInfo.outputPath("unity-controls-wide.png"),fullPage:true});
    await page.setViewportSize({width:420,height:850});
    await expect(host.getByRole("button",{name:"Discover and refresh",exact:true})).toBeInViewport();
    await expect(host.getByRole("button",{name:"Manually select",exact:true})).toBeInViewport();
    expect(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth)).toBe(true);
    await page.screenshot({path:testInfo.outputPath("unity-controls-narrow.png"),fullPage:true});
});
