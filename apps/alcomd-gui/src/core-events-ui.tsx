import { createContext, useCallback, useContext, useEffect, useId, useRef, useState, type ReactNode, type RefObject } from "react";
import type { Operation } from "./core-models";
import { CoreEvents, type ChangeScope } from "./core-events";
import { useReadingPosition } from "./view-reading-position";

const Context = createContext<CoreEvents | undefined>(undefined);
export const useCoreEvents = () => useContext(Context);

export function CoreEventsProvider({ events, enabled, children }: { events: CoreEvents; enabled: boolean; children: ReactNode }) {
    useEffect(() => {
        if (!enabled) return;
        let active = true;
        // StrictMode's first setup/cleanup must not leave a second native task.
        queueMicrotask(() => { if (active) events.start(); });
        return () => { active = false; void events.stop(); };
    }, [events, enabled]);
    return <Context.Provider value={events}>{children}</Context.Provider>;
}

export function useCoreInvalidation(key: string, scopes: readonly string[], read: () => Promise<void>, initial = false) {
    const events = useCoreEvents();
    const readRef = useRef(read);
    readRef.current = read;
    const scopeKey = scopes.join("|");
    useEffect(() => events?.register(key, scopeKey.split("|").filter(Boolean), () => readRef.current(), initial), [events, key, scopeKey, initial]);
    return useCallback(() => { if (!events?.retryRead(key)) void readRef.current().catch(() => {}); }, [events, key]);
}

export function useWorkspaceInvalidation(scopes: readonly ChangeScope[], read: () => Promise<void>) {
    const id = useId();
    return useCoreInvalidation("workspace:" + id, scopes, read);
}

export function useViewState<T>(view: "projects" | "operations" | "activity", key: string, initial: T) {
    const events = useCoreEvents();
    const [value, setValue] = useState(() => events?.metadata(view, key, initial) ?? initial);
    const write = (next: T | ((previous: T) => T)) => setValue((previous) => {
        const value = typeof next === "function" ? (next as (previous: T) => T)(previous) : next;
        events?.setMetadata(view, key, value);
        return value;
    });
    return [value, write] as const;
}

export function useViewReadingPosition(view: "projects" | "operations" | "activity", root: RefObject<HTMLElement | null>, revision: unknown, complete = true) {
    const events = useCoreEvents();
    useReadingPosition(events, view, root, revision, complete);
}

export function useObservedOperation(id: string, initial?: Operation) {
    const events = useCoreEvents();
    const [, render] = useState(0);
    useEffect(() => events?.observeOperation(id, () => render((value) => value + 1), initial), [events, id, initial]);
    return events?.operationSnapshot(id);
}
