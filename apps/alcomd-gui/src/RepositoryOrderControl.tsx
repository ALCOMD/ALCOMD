import { useEffect, useLayoutEffect, useRef, useState, type PointerEvent } from "react";

import { IconButton, Menu, MenuItem } from "./Material";
import { useTranslation } from "./i18n";

interface RepositoryDrag {
    id: string;
    handle: HTMLButtonElement;
    pointer: number;
    startY: number;
    y: number;
    frame: number;
    position?: string | null;
}

export function useRepositoryDrag(move: (id: string, before: string | null) => void) {
    const drag = useRef<RepositoryDrag | undefined>(undefined);
    const suppressClick = useRef(false);
    const [drop, setDrop] = useState<string | null>();
    useEffect(() => () => cancelAnimationFrame(drag.current?.frame ?? 0), []);

    const update = (active: RepositoryDrag) => {
        const table = active.handle.closest("table");
        const rows = table?.querySelectorAll<HTMLTableRowElement>("tr.repository-order-movable") ?? [];
        let position: string | null = null;
        for (const row of rows) {
            const id = row.dataset.rowKey;
            if (id === undefined || id === active.id) continue;
            const rect = row.getBoundingClientRect();
            if (active.y < rect.top + rect.height / 2) {
                position = id;
                break;
            }
        }
        if (active.position !== position) {
            active.position = position;
            setDrop(position);
        }
    };
    const scroll = (active: RepositoryDrag) => {
        const viewport = active.handle.closest<HTMLElement>(".desktop-list-viewport");
        if (viewport === null) return;
        const { top, bottom, height } = viewport.getBoundingClientRect();
        const edge = Math.min(36, height / 3);
        const offset = active.y < top + edge ? active.y - top - edge : Math.max(0, active.y - bottom + edge);
        if (offset !== 0) {
            viewport.scrollTop += Math.max(-18, Math.min(18, offset / 3));
            update(active);
        }
        active.frame = requestAnimationFrame(() => scroll(active));
    };
    const finish = (id: string, pointer: number, commit: boolean) => {
        const active = drag.current;
        if (active?.id !== id || active.pointer !== pointer) return;
        drag.current = undefined;
        cancelAnimationFrame(active.frame);
        setDrop(undefined);
        if (active.handle.hasPointerCapture(pointer)) active.handle.releasePointerCapture(pointer);
        if (active.position !== undefined) {
            suppressClick.current = true;
            if (commit) move(id, active.position);
        }
    };

    return {
        drop,
        bindings(id: string, enabled: boolean) {
            return {
                onPointerDown(event: PointerEvent<HTMLButtonElement>) {
                    if (!enabled || !event.isPrimary || event.button !== 0 || drag.current !== undefined) return;
                    suppressClick.current = false;
                    const handle = event.currentTarget;
                    handle.setPointerCapture(event.pointerId);
                    drag.current = { id, handle, pointer: event.pointerId, startY: event.clientY, y: event.clientY, frame: 0 };
                },
                onPointerMove(event: PointerEvent<HTMLButtonElement>) {
                    const active = drag.current;
                    if (active?.id !== id || active.pointer !== event.pointerId) return;
                    if (active.position === undefined && Math.abs(event.clientY - active.startY) < 4) return;
                    active.y = event.clientY;
                    if (active.position === undefined) active.frame = requestAnimationFrame(() => scroll(active));
                    update(active);
                },
                onPointerUp(event: PointerEvent<HTMLButtonElement>) {
                    finish(id, event.pointerId, true);
                },
                onPointerCancel(event: PointerEvent<HTMLButtonElement>) {
                    finish(id, event.pointerId, false);
                },
                onLostPointerCapture(event: PointerEvent<HTMLButtonElement>) {
                    finish(id, event.pointerId, false);
                },
                cancel() {
                    const active = drag.current;
                    if (active?.id === id) finish(id, active.pointer, false);
                },
                consumeClick() {
                    const suppressed = suppressClick.current;
                    suppressClick.current = false;
                    return suppressed;
                }
            };
        }
    };
}

export function RepositoryOrderControl({ label, busy, focusAfterMove, up, down, drag }: {
    label: string;
    busy: boolean;
    focusAfterMove: boolean;
    up?: () => void;
    down?: () => void;
    drag: ReturnType<ReturnType<typeof useRepositoryDrag>["bindings"]>;
}) {
    const { t } = useTranslation();
    const anchor = useRef<HTMLElement>(null);
    const [open, setOpen] = useState(false);
    const { cancel, consumeClick, ...pointer } = drag;
    useLayoutEffect(() => {
        const handle = anchor.current;
        if (focusAfterMove && handle !== null && document.activeElement === document.body) {
            handle.scrollIntoView({ block: "nearest", inline: "nearest" });
            handle.focus({ preventScroll: true });
        }
    }, [focusAfterMove]);
    return <>
        <IconButton
            {...pointer}
            aria-disabled={busy}
            aria-expanded={open}
            aria-haspopup="menu"
            label={label}
            className="repository-order-handle"
            onClick={(event) => {
                if (event.detail > 0 && consumeClick()) return;
                if (!busy) setOpen(true);
            }}
            onKeyDown={(event) => { if (event.key === "Escape") cancel(); }}
            ref={anchor}
            title={label}
            type="button"
        ><span aria-hidden="true" /></IconButton>
        <Menu anchorRef={anchor} open={open} onClose={() => setOpen(false)}>
            <MenuItem disabled={busy || up === undefined} label={t("Move up")} onClick={() => { setOpen(false); up?.(); }} />
            <MenuItem disabled={busy || down === undefined} label={t("Move down")} onClick={() => { setOpen(false); down?.(); }} />
        </Menu>
    </>;
}
