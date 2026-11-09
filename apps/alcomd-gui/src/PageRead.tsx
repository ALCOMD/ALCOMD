import { createContext, use, useContext } from "react";

export type PageReadResult<T> = { kind: "ready"; value: T } | { kind: "error"; error: unknown };

let nextVisit = 0;
export type PageReadKey = "primary" | "settings.unity" | "settings.repositories" | "about.status" | "repository.packages" | "backups.list";

/** At most three initial read slots per visit, shared across render retries, never across visits. */
export function createPageRead() {
    return { visit: ++nextVisit, deferUntil: performance.now() + 100,
        results: new Map<PageReadKey, Promise<PageReadResult<unknown>>>(),
        reactReads: new Set<PageReadKey>(),
        settled: new Map<PageReadKey, PageReadResult<unknown>>() };
}

export const PageReadContext = createContext<ReturnType<typeof createPageRead> | undefined>(undefined);
export const LocalPageReadContext = createContext(false);

/** Observe the same initial request without suspending the page title/toolbar. */
export function useInitialPageRequest<T>(load: () => Promise<T>) {
    const local = useContext(LocalPageReadContext);
    const scope = useContext(PageReadContext);
    const promise = usePageReadPromise("primary", load, !local && scope !== undefined);
    const settled = !local ? scope?.settled.get("primary") as PageReadResult<T> | undefined : undefined;
    return { promise, settled };
}

/** Start a visit's read before its deferred component mounts; never share across visits. */
export function primePageRead<T>(scope: ReturnType<typeof createPageRead>, key: PageReadKey, load: () => Promise<T>): Promise<PageReadResult<T>> {
    if (!scope.results.has(key)) {
        scope.results.set(key, Promise.resolve().then(load).then(
            (value): PageReadResult<T> => ({ kind: "ready", value }),
            (error: unknown): PageReadResult<T> => ({ kind: "error", error })
        ).then((result) => { scope.settled.set(key, result); return result; }));
    }
    return scope.results.get(key)! as Promise<PageReadResult<T>>;
}

/** Only read-only page loaders belong here. Mutations and extension session actions remain in events/effects. */
export function usePageReadPromise<T>(key: PageReadKey, load: () => Promise<T>): Promise<PageReadResult<T>>;
export function usePageReadPromise<T>(key: PageReadKey, load: () => Promise<T>, enabled: boolean): Promise<PageReadResult<T>> | undefined;
export function usePageReadPromise<T>(key: PageReadKey, load: () => Promise<T>, enabled = true): Promise<PageReadResult<T>> | undefined {
    const scope = useContext(PageReadContext);
    if (!enabled) return undefined;
    if (scope === undefined) throw new Error("Page reads require a route visit scope");
    return primePageRead(scope, key, load);
}

export function useInitialPageRead<T>(load: () => Promise<T>): PageReadResult<T> | undefined {
    const local = useContext(LocalPageReadContext);
    const scope = useContext(PageReadContext);
    // Reusable workspaces load locally unless they are inside a primary navigation scope.
    const promise = usePageReadPromise("primary", load, !local && scope !== undefined);
    // An already settled prefetch must not suspend once just so React can observe its thenable.
    const settled = !local ? scope?.settled.get("primary") as PageReadResult<T> | undefined : undefined;
    if (promise === undefined) return undefined;
    // Once a mount has suspended here, retries must consume the same thenable.
    // Bypassing use() on retry can leave later hooks in React's update mode even
    // though they have not mounted yet. Prefetches settled before the first
    // render still take the synchronous path without an extra suspension.
    if (settled !== undefined && !scope?.reactReads.has("primary")) return settled;
    scope?.reactReads.add("primary");
    return use(promise);
}
