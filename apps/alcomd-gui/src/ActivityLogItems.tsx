import { useCallback, type ReactNode } from "react";
import type { ActivityCursor, ActivityItem, ActivityListResult } from "./core-models";
import type { GuiRpcClient } from "./rpc";
import { LivePagedItems } from "./LivePagedItems";
export function activityItemKey(item: ActivityItem): string { return item.activityId; }
function mergeActivity(current: ActivityItem[], incoming: ActivityItem[]): ActivityItem[] {
    const items = new Map(current.map((item) => [item.activityId, item]));
    for (const item of incoming) {
        const previous = items.get(item.activityId);
        if (previous === undefined || item.revision >= previous.revision) items.set(item.activityId, item);
    }
    return [...items.values()].sort((left, right) => right.startedAtMs - left.startedAtMs || right.activityId.localeCompare(left.activityId));
}
function catchUp(cursor: ActivityCursor, loaded: ActivityItem[]) {
    const newest = [...loaded].sort((a, b) => b.startedAtMs - a.startedAtMs || b.activityId.localeCompare(a.activityId))[0];
    return newest !== undefined && (cursor.startedAtMs > newest.startedAtMs || cursor.startedAtMs === newest.startedAtMs && cursor.activityId >= newest.activityId);
}
export function ActivityLogItems({ client, initial, children }: {
    client: Pick<GuiRpcClient, "activityList" | "activityGet">;
    initial: ActivityListResult;
    children(items: ActivityItem[]): ReactNode;
}) {
    const readPage = useCallback((cursor?: ActivityCursor) => client.activityList(cursor), [client]);
    const refreshLoaded = useCallback(async (items: ActivityItem[]) => {
        const changed: ActivityItem[] = [];
        // Started-time ordering is immutable: older active rows need explicit reads.
        // Sequential reads keep this synchronization within one concurrency slot.
        for (const item of items) if (!["succeeded", "failed", "cancelled"].includes(item.status)) {
            changed.push((await client.activityGet(item.activityId)).activity);
        }
        return changed;
    }, [client]);
    return <LivePagedItems initial={initial} readPage={readPage} merge={mergeActivity} catchUp={catchUp} refreshLoaded={refreshLoaded}
        message="New activity could not be loaded. The last loaded entries are still shown.">{children}</LivePagedItems>;
}
