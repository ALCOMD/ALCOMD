import { useEffect, useRef, useState, type ReactNode } from "react";

import { Button } from "./Material";

export interface ItemPage<T, C> {
    items: T[];
    nextCursor?: C;
}

export interface PagedItemsProps<T, C> {
    initialItems: T[];
    initialCursor?: C;
    loadMore(cursor: C): Promise<ItemPage<T, C>>;
    children(items: T[]): ReactNode;
}

export function PagedItems<T, C>(props: PagedItemsProps<T, C>) {
    const [source, setSource] = useState({ items: props.initialItems, cursor: props.initialCursor, generation: 0 });
    if (source.items !== props.initialItems || source.cursor !== props.initialCursor) {
        setSource({ items: props.initialItems, cursor: props.initialCursor, generation: source.generation + 1 });
    }
    // A refreshed first page owns a new session, including its request lock and cursor.
    return <PageSession key={source.generation} {...props} />;
}

function PageSession<T, C>({ initialItems, initialCursor, loadMore, children }: PagedItemsProps<T, C>) {
    const [items, setItems] = useState(initialItems);
    const [cursor, setCursor] = useState(initialCursor);
    const [loading, setLoading] = useState(false);
    const [failed, setFailed] = useState(false);
    const request = useRef<object | undefined>(undefined);

    useEffect(() => () => { request.current = undefined; }, []);

    async function next() {
        if (cursor === undefined || request.current !== undefined) {
            return;
        }
        const token = {};
        request.current = token;
        setLoading(true);
        setFailed(false);
        try {
            const page = await loadMore(cursor);
            if (request.current !== token) {
                return;
            }
            setItems((current) => [...current, ...page.items]);
            setCursor(page.nextCursor);
        } catch {
            if (request.current === token) {
                setFailed(true);
            }
        } finally {
            if (request.current === token) {
                request.current = undefined;
                setLoading(false);
            }
        }
    }

    return (
        <>
            {children(items)}
            {cursor === undefined ? null : (
                <div className="pagination-controls">
                    {failed ? <p className="inline-error" role="alert">Could not load more items. Your loaded items are still available.</p> : null}
                    <Button widthLabels={["Loading…", "Retry loading more", "Load more"]} disabled={loading} onClick={() => { void next(); }} type="button" variant="tonal">
                        {loading ? "Loading…" : failed ? "Retry loading more" : "Load more"}
                    </Button>
                    {loading ? <span role="status">Loading more items…</span> : null}
                </div>
            )}
        </>
    );
}
