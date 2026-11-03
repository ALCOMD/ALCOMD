import { useCallback, type ReactNode } from "react";
import type { ActivityCursor, ActivityItem, ActivityListResult } from "./core-models";
import type { GuiRpcClient } from "./rpc";
import { LivePagedItems } from "./LivePagedItems";

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

function catchUp(cursor: ActivityCursor, loaded: ActivityItem[]) {
    const latest = loaded.reduce((time, item) => Math.max(time, item.occurredAtMs), 0);
    return cursor.occurredAtMs >= latest;
}

export function ActivityLogItems({ client, initial, children }: {
    client: Pick<GuiRpcClient, "activityList">;
    initial: ActivityListResult;
    children(items: ActivityItem[]): ReactNode;
}) {
    const readPage = useCallback((cursor?: ActivityCursor) => client.activityList(cursor), [client]);
    return <LivePagedItems initial={initial} readPage={readPage} merge={mergeActivity} catchUp={catchUp}
        message="New activity could not be loaded. The last loaded entries are still shown.">{children}</LivePagedItems>;
}
