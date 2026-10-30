export type MotionPreference = "system" | "full" | "reduced";

const systemMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
const listeners = new Set<() => void>();
let preference: MotionPreference = "system";

export function reducedMotion(): boolean {
    return preference === "reduced" || (preference === "system" && systemMotion.matches);
}

export function subscribeMotion(listener: () => void): () => void {
    listeners.add(listener);
    return () => { listeners.delete(listener); };
}

function notifyMotion(): void {
    document.documentElement.dataset.motion = preference;
    document.documentElement.dataset.reducedMotion = String(reducedMotion());
    for (const listener of listeners) listener();
}

export function applyMotion(value: MotionPreference): void {
    preference = value;
    notifyMotion();
}

systemMotion.addEventListener("change", notifyMotion);
notifyMotion();

interface MaterialHost extends HTMLElement {
    addController(controller: { hostConnected(): void; hostUpdated(): void; hostDisconnected(): void }): void;
}

type MaterialConstructor = CustomElementConstructor & {
    addInitializer(initializer: (host: MaterialHost) => void): void;
};

/** CSS reductions must also reach Material's shadow roots, including nested controls. */
export function installMaterialMotion(names: readonly string[]): void {
    for (const name of names) {
        const component = customElements.get(name) as MaterialConstructor;
        component.addInitializer((host) => {
            let style: HTMLStyleElement | undefined;
            let unsubscribe: (() => void) | undefined;
            const update = () => {
                if (host.shadowRoot === null) return;
                if (style === undefined) {
                    style = host.ownerDocument.createElement("style");
                    style.dataset.alcomdMotion = "";
                    style.textContent = `*, *::before, *::after {
                        scroll-behavior: auto !important;
                        transition-duration: 0.001ms !important;
                        animation-duration: 0.001ms !important;
                        animation-iteration-count: 1 !important;
                    }`;
                    // Ripple expansion uses Web Animations rather than CSS durations.
                    // Keep its pressed state layer, without a moving circle.
                    if (name === "md-ripple") style.textContent += `.surface::after {
                        inset: 0 !important; width: 100% !important; height: 100% !important;
                        transform: none !important;
                    }`;
                }
                if (style.parentNode !== host.shadowRoot) host.shadowRoot.append(style);
                style.media = reducedMotion() ? "all" : "not all";
            };
            host.addController({
                hostConnected() { unsubscribe = subscribeMotion(update); update(); },
                hostUpdated: update,
                hostDisconnected() { unsubscribe?.(); unsubscribe = undefined; }
            });
        });
    }
}
