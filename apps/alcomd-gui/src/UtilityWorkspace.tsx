import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";

import { UtilityActionDialog } from "./UtilityActionDialog";
import { Button } from "./Material";

interface UtilityWorkspaceProps<T> {
    title: string;
    navigation?: ReactNode;
    back?: ReactNode;
    refreshDisabled?: boolean;
    load(): Promise<T>;
    children(value: T, refresh: () => void): ReactNode;
    action?: {
        label: string;
        render(value: T, refresh: () => void): ReactNode;
    };
    tools?(value: T, refresh: () => void): ReactNode;
}

interface WorkspaceState<T> {
    loader: () => Promise<T>;
    value?: T;
    pending: boolean;
    error?: string;
}

export function UtilityWorkspace<T>({
    title,
    navigation,
    back,
    refreshDisabled = false,
    load,
    children,
    action,
    tools
}: UtilityWorkspaceProps<T>) {
    const [state, setState] = useState<WorkspaceState<T>>({ loader: load, pending: true });
    const [generation, setGeneration] = useState(0);
    const [actionOpen, setActionOpen] = useState(false);
    const pendingRef = useRef(true);
    const queuedRefreshRef = useRef(false);

    useEffect(() => {
        let active = true;
        pendingRef.current = true;
        queuedRefreshRef.current = false;
        setState((current) => ({
            loader: load,
            value: current.loader === load ? current.value : undefined,
            pending: true
        }));

        void Promise.resolve().then(load).then((value) => {
            if (!active) return;
            setState({ loader: load, value, pending: queuedRefreshRef.current });
        }).catch((caught: unknown) => {
            if (!active) return;
            const code = typeof caught === "object" && caught !== null
                && "code" in caught && typeof caught.code === "string"
                ? caught.code
                : "internal_error";
            setState((current) => ({ ...current, pending: queuedRefreshRef.current, error: code }));
        }).finally(() => {
            if (!active) return;
            if (queuedRefreshRef.current) {
                queuedRefreshRef.current = false;
                setGeneration((current) => current + 1);
            } else {
                pendingRef.current = false;
            }
        });

        return () => { active = false; };
    }, [load, generation]);

    const refresh = useCallback(() => {
        if (pendingRef.current) {
            // A mutation can finish while the previous snapshot is still being read.
            // Coalesce its invalidations into one follow-up read instead of losing them.
            queuedRefreshRef.current = true;
            return;
        }
        pendingRef.current = true;
        setState((current) => ({ ...current, pending: true, error: undefined }));
        setGeneration((current) => current + 1);
    }, []);

    // A new route's loader must never display the previous route's data.
    const value = state.loader === load ? state.value : undefined;
    const error = state.loader === load ? state.error : undefined;
    const pending = state.loader !== load || state.pending;

    return (
        <section className="utility-workspace page-surface--utility" aria-labelledby="route-title">
            <header className="utility-workspace-toolbar">
                {back}
                <h1 className={navigation === undefined ? undefined : "sr-only"} id="route-title" tabIndex={-1}>{title}</h1>
                {navigation}
                <div className="utility-workspace-actions">
                    {value === undefined ? null : tools?.(value, refresh)}
                    <Button widthLabels={["Refreshing…", "Refresh"]} disabled={pending || refreshDisabled} onClick={refresh} type="button" variant="tonal">
                        {pending ? "Refreshing…" : "Refresh"}
                    </Button>
                    {action === undefined ? null : (
                        <Button
                            aria-haspopup="dialog"
                            disabled={value === undefined}
                            onClick={() => setActionOpen(true)}
                            type="button"
                        >
                            {action.label}
                        </Button>
                    )}
                </div>
            </header>
            <div className="utility-workspace-body">
                {error === undefined ? null : (
                    <div className="inline-error" role="alert">
                        <p>{value === undefined ? "This page could not be loaded." : "Refresh failed. The last loaded result is still shown."} <code>{error}</code></p>
                        <Button disabled={pending} onClick={refresh} type="button" variant="text">Retry</Button>
                    </div>
                )}
                {value === undefined ? (
                    pending ? <p role="status">Loading…</p> : null
                ) : (
                    <>
                        {action === undefined || !actionOpen ? null : (
                            <UtilityActionDialog title={action.label} onClose={() => setActionOpen(false)}>
                                {action.render(value, refresh)}
                            </UtilityActionDialog>
                        )}
                        {children(value, refresh)}
                    </>
                )}
            </div>
        </section>
    );
}
