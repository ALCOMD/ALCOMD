import { createContext, useContext, useEffect, useId, useRef, type FocusEvent } from "react";
import { PreloadCache, PRELOAD_HOVER_MS } from "./preload-cache";

export const PreloadContext = createContext<PreloadCache | undefined>(undefined);
export const RoutePreloadContext = createContext<((path: string) => { key: string; load(signal: AbortSignal): Promise<unknown>; valid(): boolean } | undefined) | undefined>(undefined);

/** Pointer, focus and opened interaction are independent owners of one preparation. */
export function usePreloadIntent<T>({ key, load, discard = () => {}, enabled = true, weight, valid }: { key: string; load(signal: AbortSignal): Promise<T>; discard?(): void; enabled?: boolean; weight?(): number; valid?(): boolean }) {
    const shared = useContext(PreloadContext);
    const local = useRef<PreloadCache | undefined>(undefined);
    if (!shared && !local.current) local.current = new PreloadCache();
    const cache = shared ?? local.current!;
    const owner = useId();
    const options = useRef({ load, discard, enabled, weight });
    options.current = { load, discard, enabled, weight };
    const signals = useRef({ pointer: false, focus: false, opened: false });
    const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
    const read = () => cache.retain(key, owner, (signal) => options.current.load(signal), () => options.current.discard(), weight === undefined ? undefined : () => options.current.weight?.() ?? 0, valid);
    const sync = (immediate: boolean) => {
        window.clearTimeout(timer.current);
        const active = Object.values(signals.current).some(Boolean);
        if (!options.current.enabled || !active) { cache.release(key, owner); return; }
        if (immediate || signals.current.focus || signals.current.opened) void read().catch(() => {});
        else timer.current = window.setTimeout(() => { if (Object.values(signals.current).some(Boolean) && options.current.enabled) void read().catch(() => {}); }, PRELOAD_HOVER_MS);
    };
    useEffect(() => () => {
        window.clearTimeout(timer.current);
        cache.forget(key, owner);
        signals.current = { pointer: false, focus: false, opened: false };
    }, [cache, key]);
    useEffect(() => { if (!enabled) { window.clearTimeout(timer.current); cache.drop(key); } }, [cache, enabled, key]);
    useEffect(() => { local.current?.activate(); return () => local.current?.dispose(); }, []);
    return {
        onPointerEnter: () => { signals.current.pointer = true; sync(false); },
        onPointerLeave: () => { signals.current.pointer = false; sync(false); },
        onFocusCapture: () => { signals.current.focus = true; sync(true); },
        onBlurCapture: (event: FocusEvent<HTMLElement>) => {
            if (!event.currentTarget.contains(event.relatedTarget as Node | null)) { signals.current.focus = false; sync(false); }
        },
        pin: (opened: boolean) => { signals.current.opened = opened; sync(opened); },
        prepare: () => cache.foreground(key, owner, (signal) => options.current.load(signal), () => options.current.discard(), weight === undefined ? undefined : () => options.current.weight?.() ?? 0),
    };
}

export function useRoutePreload(path: string, enabled = true) {
    const fallback = useId();
    const route = useContext(RoutePreloadContext)?.(path);
    return usePreloadIntent({ key: route?.key ?? `disabled:${fallback}`, enabled: enabled && route !== undefined, load: (signal) => route?.load(signal) ?? Promise.resolve(undefined), valid: route?.valid });
}
