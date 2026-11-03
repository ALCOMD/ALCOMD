import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import type { RpcError } from "@alcomd/sdk";
import type { ActivityCursor, ActivityItem, ActivityListResult } from "./core-models";
import type { GuiRpcClient } from "./rpc";
import { Button } from "./Material";
import { RequestReminder } from "./Reminder";
import { normalizeRequestError } from "./request-errors";
import { useTranslation } from "./i18n";

interface Props {
    client: Pick<GuiRpcClient, "activityList">;
    initial: ActivityListResult;
    children(items: ActivityItem[]): ReactNode;
}

export function activityItemKey(item: ActivityItem): string {
    return item.type === "event" ? `event:${item.eventSequence}` : `operation:${item.operationId}`;
}

function mergeActivity(current: ActivityItem[], incoming: ActivityItem[]): ActivityItem[] {
    const items = new Map(current.map((item) => [activityItemKey(item), item]));
    for (const item of incoming) {
        const previous = items.get(activityItemKey(item));
        if (previous === undefined || item.occurredAtMs >= previous.occurredAtMs) items.set(activityItemKey(item), item);
    }
    return [...items.values()].sort((left, right) => {
        const time = right.occurredAtMs - left.occurredAtMs;
        if (time !== 0) return time;
        if (left.type !== right.type) return left.type === "event" ? -1 : 1;
        if (left.type === "event") return (right.eventSequence ?? 0) - (left.eventSequence ?? 0);
        return (right.operationId ?? "").localeCompare(left.operationId ?? "");
    });
}

export function ActivityLogItems(props: Props) {
    const [source, setSource] = useState({ client: props.client, initial: props.initial, generation: 0 });
    if (source.client !== props.client || source.initial !== props.initial) {
        setSource({ client: props.client, initial: props.initial, generation: source.generation + 1 });
    }
    return <ActivityLogSession key={source.generation} {...props} />;
}

function ActivityLogSession({ client, initial, children }: Props) {
    const { t } = useTranslation();
    const [items, setItems] = useState(initial.items);
    const itemsRef = useRef(items);
    const [cursor, setCursor] = useState(initial.nextCursor);
    const cursorRef = useRef(cursor);
    const [pending, setPending] = useState<"latest" | "more">();
    const [latestError, setLatestError] = useState<RpcError>();
    const [moreError, setMoreError] = useState<RpcError>();
    const controller = useRef<{ read(kind: "latest" | "more"): Promise<void> } | undefined>(undefined);
    const root = useRef<HTMLDivElement>(null);
    const anchor = useRef<{ row: Element; viewport: HTMLElement; top: number; scrollTop: number } | undefined>(undefined);

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
        const apply = (incoming: ActivityItem[]) => {
            const viewport = root.current?.closest<HTMLElement>(".utility-workspace-body");
            if (viewport && viewport.scrollTop > 0) {
                const bounds = viewport.getBoundingClientRect();
                const row = [...root.current!.querySelectorAll("tbody tr")].find((item) => {
                    const box = item.getBoundingClientRect();
                    return box.bottom > bounds.top && box.top < bounds.bottom;
                });
                if (row) anchor.current = { row, viewport, top: row.getBoundingClientRect().top, scrollTop: viewport.scrollTop };
            }
            itemsRef.current = mergeActivity(itemsRef.current, incoming);
            setItems(itemsRef.current);
        };
        const read = async (kind: "latest" | "more") => {
            if (!active || reading || (kind === "more" && cursorRef.current === undefined)) return;
            reading = true;
            setPending(kind);
            try {
                if (kind === "more") {
                    const page = await client.activityList(cursorRef.current);
                    if (!active) return;
                    apply(page.items);
                    cursorRef.current = page.nextCursor;
                    setCursor(page.nextCursor);
                    setMoreError(undefined);
                } else {
                    const boundary = itemsRef.current.length === 0 ? undefined : itemsRef.current.reduce((latest, item) => Math.max(latest, item.occurredAtMs), 0);
                    const incoming: ActivityItem[] = [];
                    let next: ActivityCursor | undefined;
                    const seen = new Set<string>();
                    do {
                        const page = await client.activityList(next);
                        if (!active) return;
                        incoming.push(...page.items);
                        next = page.nextCursor;
                        if (boundary === undefined || next === undefined || next.occurredAtMs < boundary) break;
                        const key = JSON.stringify(next);
                        if (seen.has(key)) throw { code: "internal_error" };
                        seen.add(key);
                    } while (next !== undefined);
                    if (boundary === undefined) {
                        cursorRef.current = next;
                        setCursor(next);
                    }
                    apply(incoming);
                    setLatestError(undefined);
                }
            } catch (caught: unknown) {
                if (active) (kind === "more" ? setMoreError : setLatestError)(normalizeRequestError(caught));
            } finally {
                reading = false;
                if (active) setPending(undefined);
            }
        };
        const controls = { read };
        controller.current = controls;
        const update = () => { if (document.visibilityState === "visible") void read("latest"); };
        const timer = window.setInterval(update, 3_000);
        document.addEventListener("visibilitychange", update);
        return () => {
            active = false;
            window.clearInterval(timer);
            document.removeEventListener("visibilitychange", update);
            if (controller.current === controls) controller.current = undefined;
        };
    }, [client]);

    return <div ref={root}>
        {latestError === undefined ? null : <RequestReminder inline error={latestError} message="New activity could not be loaded. The last loaded entries are still shown.">
            <Button disabled={pending !== undefined} onClick={() => void controller.current?.read("latest")} type="button" variant="text">{t("Retry")}</Button>
        </RequestReminder>}
        {children(items)}
        {cursor === undefined ? null : <div className="pagination-controls" aria-busy={pending === "more"}>
            {moreError === undefined ? null : <RequestReminder error={moreError}>{t("Could not load more items. Your loaded items are still available.")}</RequestReminder>}
            <Button disabled={pending !== undefined} onClick={() => void controller.current?.read("more")} type="button" variant="tonal">{t(moreError === undefined ? "Load more" : "Retry loading more")}</Button>
        </div>}
    </div>;
}
