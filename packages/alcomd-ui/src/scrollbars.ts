import { OverlayScrollbars, type OverflowBehavior } from "overlayscrollbars";
import libraryCss from "overlayscrollbars/overlayscrollbars.css?raw";
import themeCss from "./scrollbars.css?raw";

/** Keep the existing viewport and its children under their original owner's control. */
export function createScrollArea(viewport: HTMLElement, overflowX: OverflowBehavior = "scroll") {
    viewport.classList.add("alcomd-scroll-area");
    return OverlayScrollbars({
        target: viewport,
        elements: { viewport, padding: false, content: false }
    }, {
        overflow: { x: overflowX, y: "scroll" },
        scrollbars: { theme: "alcomd-scrollbars", autoHide: "leave", autoHideDelay: 600, visibility: "auto", clickScroll: "instant" }
    });
}

interface ScrollHost extends HTMLElement {
    open?: boolean;
    addController(controller: { hostConnected(): void; hostUpdated(): void; hostDisconnected(): void }): void;
}

type MaterialConstructor = CustomElementConstructor & {
    addInitializer(initializer: (host: ScrollHost) => void): void;
};

/** The selectors belong to the pinned Material adapter, never to individual pages. */
export function installMaterialScrollbars(surfaces: readonly { name: string; viewport: string }[]): void {
    for (const surface of surfaces) {
        const component = customElements.get(surface.name) as MaterialConstructor;
        component.addInitializer((host) => {
            let style: HTMLStyleElement | undefined;
            let viewport: HTMLElement | null = null;
            let instance: OverlayScrollbars | undefined;
            const connect = () => {
                const root = host.shadowRoot;
                if (root === null || !host.isConnected) return;
                // Closed row menus can number in the hundreds. They need no observers.
                if (!host.open) {
                    instance?.destroy();
                    instance = undefined;
                    viewport = null;
                    return;
                }
                if (style === undefined) {
                    style = host.ownerDocument.createElement("style");
                    style.dataset.alcomdScrollbars = "";
                    style.textContent = libraryCss + themeCss;
                }
                if (style.parentNode !== root) root.append(style);
                const next = root.querySelector<HTMLElement>(surface.viewport);
                if (next !== viewport) {
                    instance?.destroy();
                    viewport = next;
                    instance = next === null ? undefined : createScrollArea(next, "hidden");
                }
            };
            host.addController({
                hostConnected: connect,
                hostUpdated: connect,
                hostDisconnected() {
                    instance?.destroy();
                    instance = undefined;
                    viewport = null;
                }
            });
        });
    }
}
