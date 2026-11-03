import { useTranslation } from "./i18n";
import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";

import { UtilityActionDialog } from "./UtilityActionDialog";
import { Button, RefreshButton } from "./Material";
import { PageHeader } from "./PageHeader";
import { LoadingState } from "./LoadingState";
import { useInitialPageRead } from "./PageRead";
import { ScrollArea } from "./ScrollArea";
import { useReconnect } from "./capabilities";
import type { RpcError } from "@alcomd/sdk";
import { GlobalReminder, RequestReminder } from "./Reminder";
import { normalizeRequestError } from "./request-errors";

interface UtilityWorkspaceProps<T> {
    title: string;
    navigation?: ReactNode;
    back?: ReactNode;
    refreshDisabled?: boolean;
    showRefresh?: boolean;
    load(): Promise<T>;
    children(value: T, refresh: () => void): ReactNode;
    action?: {
        label: string;
        disabled?: boolean;
        render(value: T, refresh: () => void): ReactNode;
    };
    tools?(value: T, refresh: () => void): ReactNode;
}

interface WorkspaceState<T> {
    loader: () => Promise<T>;
    value?: T;
    pending: boolean;
    error?: RpcError;
}

export function UtilityWorkspace<T>({
    title,
    navigation,
    back,
    refreshDisabled = false,
    showRefresh = true,
    load,
    children,
    action,
    tools
}: UtilityWorkspaceProps<T>) {
    const { t } = useTranslation();
    const reconnect = useReconnect();
    const initial = useInitialPageRead(load);
    const initialLoader = useRef(load).current;
    const [state, setState] = useState<WorkspaceState<T>>(() => ({
        loader: load, pending: initial === undefined,
        ...(initial?.kind === "ready" ? { value: initial.value } : initial?.kind === "error" ? { error: normalizeRequestError(initial.error) } : {})
    }));
    const [generation, setGeneration] = useState(0);
    const [actionOpen, setActionOpen] = useState(false);
    const pendingRef = useRef(initial === undefined);
    const queuedRefreshRef = useRef(false);

    useEffect(() => {
        if (initial !== undefined && generation === 0 && load === initialLoader) return;
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
            setState((current) => ({ ...current, pending: queuedRefreshRef.current, error: normalizeRequestError(caught) }));
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
    }, [load, generation, initialLoader, initial]);

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
    const retry = () => {
        if (error?.code === "daemon_unavailable") reconnect?.();
        refresh();
    };

    const failure = error === undefined ? null : <RequestReminder inline={value === undefined} error={error}>
        <p>{value === undefined ? t("This page could not be loaded.") : t("Refresh failed. The last loaded result is still shown.")}</p>
        <Button disabled={pending} onClick={retry} type="button" variant="text">{t("Retry")}</Button>
    </RequestReminder>;

    return (
        <section className="utility-workspace page-surface--utility" aria-busy={value === undefined && pending} aria-labelledby="route-title">
            <PageHeader className="utility-workspace-toolbar" leading={back} title={title}
                refresh={showRefresh ? <RefreshButton disabled={refreshDisabled} onClick={retry} refreshing={pending} /> : undefined}>
                {navigation}
                <div className="utility-workspace-actions">
                    {value === undefined ? null : tools?.(value, refresh)}
                    {action === undefined ? null : (
                        <Button
                            aria-haspopup="dialog"
                            disabled={value === undefined || action.disabled}
                            onClick={() => setActionOpen(true)}
                            type="button"
                        >
                            {action.label}
                        </Button>
                    )}
                </div>
            </PageHeader>
            <ScrollArea className="utility-workspace-body">
                {value === undefined ? failure : <GlobalReminder>{failure}</GlobalReminder>}
                {value === undefined ? (
                    pending ? <LoadingState title={t("Loading…")} /> : null
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
            </ScrollArea>
        </section>
    );
}
