import type { Locator } from "@playwright/test";

export function observeActionMenuLifecycle(trigger: Locator) {
    return trigger.page().locator("body").evaluateHandle((element) => {
        const once = (type: string) => new Promise<void>((resolve) => {
            const listener = (event: Event) => {
                if (!(event.target instanceof Element) || !event.target.matches('md-menu[data-menu-kind="actions"]')) return;
                element.removeEventListener(type, listener, true);
                resolve();
            };
            element.addEventListener(type, listener, true);
        });
        return { opened: once("opened"), closed: once("closed") };
    });
}

/** Exercise a normal action after Material's opening and closing animations. */
export async function selectActionMenuItem(trigger: Locator, item: Locator): Promise<void> {
    const lifecycle = await observeActionMenuLifecycle(trigger);
    try {
        await trigger.click();
        await lifecycle.evaluate((events) => events.opened);
        await item.click();
        await lifecycle.evaluate((events) => events.closed);
    } finally {
        await lifecycle.dispose();
    }
}
