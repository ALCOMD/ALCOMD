import { useEffect, useLayoutEffect, useRef, type ReactNode } from "react";

/** Keep one live route tree: motion never delays navigation or replays page effects. */
export function PageTransition({ children, routeKey }: { children: ReactNode; routeKey: string }) {
    const ref = useRef<HTMLDivElement>(null);
    const previousRoute = useRef(routeKey);
    const animation = useRef<Animation | undefined>(undefined);

    useEffect(() => {
        const root = document.documentElement;
        const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
        const onPointer = () => { root.dataset.inputModality = "pointer"; };
        const onKeyboard = () => {
            root.dataset.inputModality = "keyboard";
            animation.current?.cancel();
        };
        const onMotionChange = () => {
            if (reducedMotion.matches || root.dataset.motion === "reduced") animation.current?.cancel();
        };
        const observer = new MutationObserver(onMotionChange);
        observer.observe(root, { attributes: true, attributeFilter: ["data-motion"] });
        document.addEventListener("pointerdown", onPointer, true);
        document.addEventListener("keydown", onKeyboard, true);
        reducedMotion.addEventListener("change", onMotionChange);
        return () => {
            document.removeEventListener("pointerdown", onPointer, true);
            document.removeEventListener("keydown", onKeyboard, true);
            reducedMotion.removeEventListener("change", onMotionChange);
            observer.disconnect();
            animation.current?.cancel();
            delete root.dataset.inputModality;
        };
    }, []);

    useLayoutEffect(() => {
        if (previousRoute.current === routeKey) return;
        previousRoute.current = routeKey;
        const root = document.documentElement;
        if (root.dataset.inputModality !== "pointer" || root.dataset.motion === "reduced"
            || window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
        // Fade only the content; the canvas stays opaque, and fixed menus keep their coordinates.
        const next = ref.current?.animate([{ opacity: 0.65 }, { opacity: 1 }], {
            duration: 180,
            easing: "cubic-bezier(0.2, 0, 0, 1)"
        });
        animation.current = next;
        return () => next?.cancel();
    }, [routeKey]);

    return <div className="route-content" ref={ref}>{children}</div>;
}
