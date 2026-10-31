import { createScrollArea } from "@alcomd/ui";
import { useCallback, type HTMLAttributes } from "react";

/** React 19's ref cleanup destroys observers and generated controls before unmount. */
export function useScrollArea<T extends HTMLElement>() {
    return useCallback((element: T | null) => {
        if (element === null) return;
        element.setAttribute("data-overlayscrollbars-initialize", "");
        let instance: ReturnType<typeof createScrollArea> | undefined;
        // Allow a rendering opportunity before synchronous scrollbar measurements.
        let frame = requestAnimationFrame(() => {
            frame = requestAnimationFrame(() => { instance = createScrollArea(element); });
        });
        return () => {
            cancelAnimationFrame(frame);
            instance?.destroy();
            element.removeAttribute("data-overlayscrollbars-initialize");
        };
    }, []);
}

export function ScrollArea(props: HTMLAttributes<HTMLDivElement>) {
    return <div {...props} ref={useScrollArea<HTMLDivElement>()} />;
}
