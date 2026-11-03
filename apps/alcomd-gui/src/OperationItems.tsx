import { useCallback, useMemo, type ReactNode } from "react";
import type { Operation, OperationsListResult } from "./core-models";
import type { GuiRpcClient } from "./rpc";
import { LivePagedItems } from "./LivePagedItems";

type Cursor = NonNullable<OperationsListResult["nextCursor"]>;

function merge(current: Operation[], incoming: Operation[]) {
    const items = new Map(current.map((item) => [item.operationId, item]));
    for (const item of incoming) {
        const previous = items.get(item.operationId);
        if (previous === undefined || item.revision >= previous.revision) items.set(item.operationId, item);
    }
    return [...items.values()].sort((left, right) => right.createdAtMs - left.createdAtMs || right.operationId.localeCompare(left.operationId));
}

function catchUp(cursor: Cursor, loaded: Operation[]) {
    const compare = (left: Cursor, right: Cursor) => left.createdAtMs - right.createdAtMs || left.operationId.localeCompare(right.operationId);
    const newest = loaded.reduce((latest, item) => compare(item, latest) > 0 ? item : latest);
    // Older unfinished tasks still change without moving in the creation-time order.
    const boundary = loaded.reduce((oldest, item) => ["succeeded", "failed", "cancelled"].includes(item.state)
        || compare(item, oldest) >= 0 ? oldest : item, newest);
    return compare(cursor, boundary) > 0;
}

export function OperationItems({ client, initial, children }: {
    client: Pick<GuiRpcClient, "operationsList">;
    initial: OperationsListResult;
    children(items: Operation[]): ReactNode;
}) {
    const first = useMemo(() => ({ items: initial.operations, nextCursor: initial.nextCursor }), [initial]);
    const readPage = useCallback(async (cursor?: Cursor) => {
        const page = await client.operationsList(cursor);
        return { items: page.operations, nextCursor: page.nextCursor };
    }, [client]);
    return <LivePagedItems initial={first} readPage={readPage} merge={merge} catchUp={catchUp}
        message="Tasks could not be updated. The last loaded entries are still shown.">{children}</LivePagedItems>;
}
