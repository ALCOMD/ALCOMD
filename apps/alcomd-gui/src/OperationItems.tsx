import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import type { Operation, OperationsListResult } from "./core-models";
import type { GuiRpcClient } from "./rpc";
import { Button } from "./Material";
import { RequestReminder } from "./Reminder";
import { normalizeRequestError } from "./request-errors";
import { useTranslation } from "./i18n";
import { useCoreEvents, useCoreInvalidation, useViewReadingPosition } from "./core-events-ui";

type Cursor = NonNullable<OperationsListResult["nextCursor"]>;
interface Saved { items: Operation[]; cursor?: Cursor }
const terminal = (item: Operation) => ["succeeded", "failed", "cancelled"].includes(item.state);
const compare = (left: Cursor, right: Cursor) => left.createdAtMs - right.createdAtMs || left.operationId.localeCompare(right.operationId);
function merge(current: Operation[], incoming: Operation[]) {
    const items = new Map(current.map((item) => [item.operationId, item]));
    for (const item of incoming) {
        const previous = items.get(item.operationId);
        if (!previous || item.revision >= previous.revision) items.set(item.operationId, item);
    }
    return [...items.values()].sort((left, right) => compare(right, left));
}

export function OperationItems(props: { client: GuiRpcClient; initial: OperationsListResult; children(items: Operation[]): ReactNode }) {
    const [source, setSource] = useState({ client: props.client, generation: 0 });
    if (source.client !== props.client) setSource({ client: props.client, generation: source.generation + 1 });
    return <OperationSession key={source.generation} {...props} />;
}

function OperationSession({ client, initial, children }: { client: GuiRpcClient; initial: OperationsListResult; children(items: Operation[]): ReactNode }) {
    const { t } = useTranslation();
    const events = useCoreEvents();
    const saved = useRef(events?.readView<Saved>("operations"));
    const [items, setItems] = useState(() => merge(saved.current?.items ?? [], initial.operations));
    const itemsRef = useRef(items);
    const [cursor, setCursor] = useState(saved.current?.cursor ?? initial.nextCursor);
    const cursorRef = useRef(cursor);
    const [pending, setPending] = useState(false);
    const [synchronized, setSynchronized] = useState(false);
    const [error, setError] = useState<ReturnType<typeof normalizeRequestError>>();
    const [moreError, setMoreError] = useState<ReturnType<typeof normalizeRequestError>>();
    const active = useRef(true);
    const reading = useRef(false);
    const queued = useRef(false);
    const seeding = useRef(false);
    const root = useRef<HTMLDivElement>(null);
    const observers = useRef(new Map<string, () => void>());
    useViewReadingPosition("operations", root, items, synchronized);
    const apply = useCallback((incoming: Operation[]) => {
        if (!active.current) return;
        const next = merge(itemsRef.current, incoming);
        itemsRef.current = next;
        setItems(next);
        seeding.current = true;
        for (const item of incoming) events?.seedOperation(item);
        seeding.current = false;
    }, [events]);

    const latest = useCallback(async () => {
        if (reading.current) { queued.current = true; return; }
        reading.current = true;
        setPending(true);
        const overflowVersion = events?.overflowVersion() ?? 0;
        try {
            do {
                queued.current = false;
                const previous = itemsRef.current;
                const boundary = saved.current?.items.length ? saved.current.items.reduce((newest, item) => compare(item, newest) > 0 ? item : newest)
                    : events?.metadata<Cursor | undefined>("operations", "historyBoundary", undefined);
                const head = await client.operationsList();
                if (!active.current) return;
                const incoming = [...head.operations];
                let history = head.nextCursor;
                const seen = new Set<string>();
                while (history && boundary && compare(history, boundary) > 0) {
                    const key = JSON.stringify(history);
                    if (seen.has(key)) throw { code: "internal_error" };
                    seen.add(key);
                    const page = await client.operationsList(history);
                    if (!active.current) return;
                    incoming.push(...page.operations);
                    history = page.nextCursor;
                }
                const historyLast = incoming.at(-1);
                const alive = new Map<string, Operation>();
                let activeCursor: Cursor | undefined;
                const activePages = new Set<string>();
                do {
                    const page = await client.operationsList(activeCursor, true);
                    if (!active.current) return;
                    for (const item of page.operations) alive.set(item.operationId, item);
                    activeCursor = page.nextCursor;
                    if (activeCursor) {
                        const key = JSON.stringify(activeCursor);
                        if (activePages.has(key)) throw { code: "internal_error" };
                        activePages.add(key);
                    }
                } while (activeCursor);
                incoming.push(...alive.values());
                // A task may have finished while this view was away and be older than the recent page.
                for (const item of previous) if (!terminal(item) && !alive.has(item.operationId)) {
                    incoming.push(await client.operationGet(item.operationId));
                    if (!active.current) return;
                }
                if (!saved.current) {
                    cursorRef.current = history; setCursor(history);
                    if (historyLast) events?.setMetadata("operations", "historyBoundary", { createdAtMs: historyLast.createdAtMs, operationId: historyLast.operationId });
                }
                apply(incoming);
                saved.current = { items: itemsRef.current, cursor: cursorRef.current };
                setError(undefined);
            } while (queued.current && active.current);
            events?.completeOverflowSync(overflowVersion);
            if (active.current) setSynchronized(true);
        } catch (caught: unknown) {
            if (active.current) setError(normalizeRequestError(caught));
            throw caught;
        } finally { reading.current = false; if (active.current) setPending(false); }
    }, [client, events, apply]);
    const retryLatest = useCoreInvalidation("operations:list", ["operations", "operations-resync"], latest, true);
    useEffect(() => {
        active.current = true;
        if (!events) void latest().catch(() => {});
        const release = events?.subscribeOperations((value) => {
            if (seeding.current) return;
            if (itemsRef.current.some((item) => item.operationId === value.operationId)) apply([value]);
        });
        return () => {
            active.current = false; release?.();
            observers.current.forEach((stop) => stop()); observers.current.clear();
        };
    }, [events, latest, apply]);
    useEffect(() => {
        if (!events) return;
        const watched = new Set(items.filter((item) => !terminal(item)).map((item) => item.operationId));
        for (const [id, stop] of observers.current) if (!watched.has(id)) { stop(); observers.current.delete(id); }
        seeding.current = true;
        for (const item of items) if (watched.has(item.operationId) && !observers.current.has(item.operationId)) {
            observers.current.set(item.operationId, events.observeOperation(item.operationId, () => {}, item));
        }
        seeding.current = false;
        events.writeView("operations", { items, cursor: cursorRef.current }, items.length);
    }, [events, items, cursor]);

    const more = async () => {
        if (reading.current || !cursorRef.current) return;
        reading.current = true; setPending(true);
        try {
            const page = await client.operationsList(cursorRef.current);
            if (!active.current) return;
            cursorRef.current = page.nextCursor; setCursor(page.nextCursor);
            apply(page.operations); setMoreError(undefined);
            const oldest = page.operations.at(-1);
            if (oldest) events?.setMetadata("operations", "historyBoundary", { createdAtMs: oldest.createdAtMs, operationId: oldest.operationId });
            saved.current = { items: itemsRef.current, cursor: cursorRef.current };
        } catch (caught: unknown) { if (active.current) setMoreError(normalizeRequestError(caught)); }
        finally {
            reading.current = false; if (active.current) setPending(false);
            if (queued.current && active.current) void latest().catch(() => {});
        }
    };
    return <div ref={root}>
        {error === undefined ? null : <RequestReminder inline error={error} message="Tasks could not be updated. The last loaded entries are still shown.">
            <Button disabled={pending} onClick={retryLatest} type="button" variant="text">{t("Retry")}</Button>
        </RequestReminder>}
        {children(items)}
        {cursor === undefined ? null : <div className="pagination-controls" aria-busy={pending}>
            {moreError === undefined ? null : <RequestReminder error={moreError}>{t("Could not load more items. Your loaded items are still available.")}</RequestReminder>}
            <Button disabled={pending} onClick={() => void more()} type="button" variant="tonal">{t(moreError === undefined ? "Load more" : "Retry loading more")}</Button>
        </div>}
    </div>;
}
