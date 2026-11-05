import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import type { RpcError } from "@alcomd/sdk";
import { Button } from "./Material";
import { RequestReminder } from "./Reminder";
import { normalizeRequestError } from "./request-errors";
import { useTranslation } from "./i18n";
import type { ItemPage } from "./PagedItems";
import { useCoreEvents, useCoreInvalidation, useViewReadingPosition } from "./core-events-ui";

interface LivePagedItemsProps<T, C> {
    initial: ItemPage<T, C>;
    readPage(cursor?: C): Promise<ItemPage<T, C>>;
    merge(current: T[], incoming: T[]): T[];
    catchUp(cursor: C, loaded: T[]): boolean;
    message: string;
    refreshLoaded?(items: T[]): Promise<T[]>;
    children(items: T[]): ReactNode;
}

export function LivePagedItems<T, C>(props: LivePagedItemsProps<T, C>) {
    const [source, setSource] = useState({ reader: props.readPage, initial: props.initial, generation: 0 });
    if (source.reader !== props.readPage) {
        setSource({ reader: props.readPage, initial: props.initial, generation: source.generation + 1 });
    }
    return <LivePageSession key={source.generation} {...props} />;
}

function LivePageSession<T, C>({ readPage, merge, catchUp, initial, message, children, refreshLoaded }: LivePagedItemsProps<T, C>) {
    const { t } = useTranslation();
    const events = useCoreEvents();
    const saved = useRef(events?.readView<ItemPage<T, C>>("activity"));
    const bookmark = useRef(events?.metadata<T | undefined>("activity", "historyBoundary", undefined));
    const firstBoundary = useRef<T[] | undefined>(saved.current?.items ?? (bookmark.current ? [bookmark.current] : initial.items));
    const restoring = useRef(saved.current === undefined && bookmark.current !== undefined);
    const [items, setItems] = useState(() => merge(saved.current?.items ?? [], initial.items));
    const itemsRef = useRef(items);
    const [cursor, setCursor] = useState(saved.current?.nextCursor ?? initial.nextCursor);
    const cursorRef = useRef(cursor);
    const [pending, setPending] = useState<"latest" | "more">();
    const [latestError, setLatestError] = useState<RpcError>();
    const [moreError, setMoreError] = useState<RpcError>();
    const [synchronized, setSynchronized] = useState(false);
    const controller = useRef<{ read(kind: "latest" | "more"): Promise<void> } | undefined>(undefined);
    const root = useRef<HTMLDivElement>(null);
    const anchor = useRef<{ row: Element; viewport: HTMLElement; top: number; scrollTop: number } | undefined>(undefined);
    useViewReadingPosition("activity", root, items, synchronized);
    useEffect(() => {
        events?.writeView("activity", { items, nextCursor: cursor }, items.length);
        if (items.length && (synchronized || bookmark.current === undefined)) events?.setMetadata("activity", "historyBoundary", items.at(-1));
    }, [events, items, cursor, synchronized]);

    useLayoutEffect(() => {
        const previous = anchor.current;
        anchor.current = undefined;
        if (previous?.row.isConnected && previous.viewport.scrollTop === previous.scrollTop) {
            previous.viewport.scrollTop += previous.row.getBoundingClientRect().top - previous.top;
        }
    }, [items]);

    useEffect(() => {
        let active = true;
        let reading = false;
        let queuedLatest = false;
        const apply = (incoming: T[]) => {
            const viewport = root.current?.closest<HTMLElement>(".utility-workspace-body");
            if (viewport && viewport.scrollTop > 0) {
                const bounds = viewport.getBoundingClientRect();
                const row = [...root.current!.querySelectorAll("tbody tr")].find((item) => {
                    const box = item.getBoundingClientRect();
                    return box.bottom > bounds.top && box.top < bounds.bottom;
                });
                if (row) anchor.current = { row, viewport, top: row.getBoundingClientRect().top, scrollTop: viewport.scrollTop };
            }
            itemsRef.current = merge(itemsRef.current, incoming);
            setItems(itemsRef.current);
        };
        const read = async (kind: "latest" | "more") => {
            if (!active || (kind === "more" && cursorRef.current === undefined)) return;
            if (reading) { if (kind === "latest") queuedLatest = true; return; }
            reading = true;
            setPending(kind);
            try {
                if (kind === "more") {
                    const page = await readPage(cursorRef.current);
                    if (!active) return;
                    apply(page.items);
                    cursorRef.current = page.nextCursor;
                    setCursor(page.nextCursor);
                    setMoreError(undefined);
                } else {
                    const previous = firstBoundary.current ?? itemsRef.current;
                    const incoming: T[] = [];
                    let next: C | undefined;
                    const seen = new Set<string>();
                    do {
                        const page = await readPage(next);
                        if (!active) return;
                        incoming.push(...page.items);
                        next = page.nextCursor;
                        if (next === undefined || !catchUp(next, previous)) break;
                        const key = JSON.stringify(next);
                        if (seen.has(key)) throw { code: "internal_error" };
                        seen.add(key);
                    } while (next !== undefined);
                    if (previous.length === 0 || restoring.current) {
                        cursorRef.current = next;
                        setCursor(next);
                    }
                    if (refreshLoaded !== undefined) incoming.push(...await refreshLoaded(itemsRef.current));
                    if (!active) return;
                    apply(incoming);
                    firstBoundary.current = undefined;
                    restoring.current = false;
                    setLatestError(undefined);
                    setSynchronized(true);
                }
            } catch (caught: unknown) {
                if (active) (kind === "more" ? setMoreError : setLatestError)(normalizeRequestError(caught));
                if (kind === "latest") throw caught;
            } finally {
                reading = false;
                if (active) setPending(undefined);
                if (active && queuedLatest) { queuedLatest = false; void read("latest").catch(() => {}); }
            }
        };
        const controls = { read };
        controller.current = controls;
        return () => {
            active = false;
            if (controller.current === controls) controller.current = undefined;
        };
    }, [readPage, merge, catchUp, refreshLoaded]);
    const retryLatest = useCoreInvalidation("activity:latest", ["activity"], async () => { await controller.current?.read("latest"); }, true);

    return <div ref={root}>
        {latestError === undefined ? null : <RequestReminder error={latestError} message={message}>
            <Button disabled={pending !== undefined} onClick={retryLatest} type="button" variant="text">{t("Retry")}</Button>
        </RequestReminder>}
        {children(items)}
        {cursor === undefined ? null : <div className="pagination-controls" aria-busy={pending === "more"}>
            {moreError === undefined ? null : <RequestReminder error={moreError}>{t("Could not load more items. Your loaded items are still available.")}</RequestReminder>}
            <Button disabled={pending !== undefined} onClick={() => void controller.current?.read("more")} type="button" variant="tonal">{t(moreError === undefined ? "Load more" : "Retry loading more")}</Button>
        </div>}
    </div>;
}
