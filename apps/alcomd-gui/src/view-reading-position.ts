import { useLayoutEffect, useRef, type RefObject } from "react";
import type { CoreEvents } from "./core-events";

type View = "projects" | "operations" | "activity";
interface ReadingPosition { row?: string; index: number; offset: number; scrollTop: number }

/** Restore a stable row only after its history has returned, rather than clamping on the first page. */
export function useReadingPosition(events: CoreEvents | undefined, view: View, root: RefObject<HTMLElement | null>, revision: unknown, complete: boolean) {
    const saved = useRef(events?.metadata<ReadingPosition | undefined>(view, "readingPosition", undefined));
    const restored = useRef(saved.current === undefined);
    useLayoutEffect(() => {
        const element = root.current;
        if (!events || !element) return;
        const viewport = element.closest<HTMLElement>(".utility-workspace-body") ?? element.querySelector<HTMLElement>(".desktop-list-viewport") ?? element;
        const rows = () => [...element.querySelectorAll<HTMLElement>("tbody tr[data-row-key]")];
        const record = () => {
            if (!restored.current) return;
            const bounds = viewport.getBoundingClientRect();
            const all = rows();
            const index = all.findIndex((row) => row.getBoundingClientRect().bottom > bounds.top);
            const row = all[Math.max(0, index)];
            const position: ReadingPosition = { row: row?.dataset.rowKey, index: Math.max(0, index),
                offset: row ? row.getBoundingClientRect().top - bounds.top : 0, scrollTop: viewport.scrollTop };
            saved.current = position;
            events.setMetadata(view, "readingPosition", position);
        };
        const restore = () => {
            const position = saved.current;
            if (!position || position.scrollTop === 0) { restored.current = true; record(); return; }
            const bounds = viewport.getBoundingClientRect();
            if (bounds.height <= 0) return;
            const all = rows();
            let row = all.find((row) => row.dataset.rowKey === position.row);
            if (!row && !complete) return;
            row ??= all[Math.min(position.index, Math.max(0, all.length - 1))];
            if (row) viewport.scrollTop += row.getBoundingClientRect().top - bounds.top - position.offset;
            else viewport.scrollTop = position.scrollTop;
            restored.current = true;
            record();
        };
        restore();
        const chooseCurrentPosition = () => { if (!restored.current) { restored.current = true; record(); } };
        viewport.addEventListener("scroll", record);
        viewport.addEventListener("wheel", chooseCurrentPosition, { passive: true });
        viewport.addEventListener("pointerdown", chooseCurrentPosition, { passive: true });
        viewport.addEventListener("keydown", chooseCurrentPosition);
        const observer = new ResizeObserver(() => { if (!restored.current) restore(); });
        observer.observe(viewport);
        return () => {
            record(); observer.disconnect();
            viewport.removeEventListener("scroll", record);
            viewport.removeEventListener("wheel", chooseCurrentPosition);
            viewport.removeEventListener("pointerdown", chooseCurrentPosition);
            viewport.removeEventListener("keydown", chooseCurrentPosition);
        };
    }, [events, view, root, revision, complete]);
}
