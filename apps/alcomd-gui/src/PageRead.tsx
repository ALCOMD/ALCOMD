import { createContext, use, useContext } from "react";

export type PageReadResult<T> = { kind: "ready"; value: T } | { kind: "error"; error: unknown };

let nextVisit = 0;
type PageReadKey = "primary" | "settings.unity" | "settings.repositories";

/** At most three initial read slots per visit, shared across render retries, never across visits. */
export function createPageRead() {
    return { visit: ++nextVisit, results: new Map<PageReadKey, Promise<PageReadResult<unknown>>>() };
}

export const PageReadContext = createContext<ReturnType<typeof createPageRead> | undefined>(undefined);
export const LocalPageReadContext = createContext(false);

/** Only read-only page loaders belong here. Mutations and extension session actions remain in events/effects. */
export function usePageReadPromise<T>(key: PageReadKey, load: () => Promise<T>): Promise<PageReadResult<T>>;
export function usePageReadPromise<T>(key: PageReadKey, load: () => Promise<T>, enabled: boolean): Promise<PageReadResult<T>> | undefined;
export function usePageReadPromise<T>(key: PageReadKey, load: () => Promise<T>, enabled = true): Promise<PageReadResult<T>> | undefined {
    const scope = useContext(PageReadContext);
    if (!enabled) return undefined;
    if (scope === undefined) throw new Error("Page reads require a route visit scope");
    if (!scope.results.has(key)) {
        scope.results.set(key, Promise.resolve().then(load).then(
            (value): PageReadResult<T> => ({ kind: "ready", value }),
            (error: unknown): PageReadResult<T> => ({ kind: "error", error })
        ));
    }
    return scope.results.get(key)! as Promise<PageReadResult<T>>;
}

export function useInitialPageRead<T>(load: () => Promise<T>): PageReadResult<T> | undefined {
    const local = useContext(LocalPageReadContext);
    const scope = useContext(PageReadContext);
    // Reusable workspaces load locally unless they are inside a primary navigation scope.
    const promise = usePageReadPromise("primary", load, !local && scope !== undefined);
    return promise === undefined ? undefined : use(promise);
}
