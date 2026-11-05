import { requestErrorMessage } from "./request-errors";
import { useTranslation } from "./i18n";
import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";

import { UtilityActionDialog } from "./UtilityActionDialog";
import { Button } from "./Material";
import { PageHeader } from "./PageHeader";
import { LoadingState } from "./LoadingState";
import { useInitialPageRead } from "./PageRead";
import { ScrollArea } from "./ScrollArea";
import { useReconnect } from "./capabilities";
import type { RpcError } from "@alcomd/sdk";
import { normalizeRequestError } from "./request-errors";
import { useCoreEvents, useWorkspaceInvalidation } from "./core-events-ui";
import type { ChangeScope } from "./core-events";

interface UtilityWorkspaceProps<T> {
    title: string;
    navigation?: ReactNode;
    back?: ReactNode;
    load(): Promise<T>;
    children(value: T, refresh: () => void): ReactNode;
    action?: {
        label: string;
        disabled?: boolean;
        render(value: T, refresh: () => void): ReactNode;
    };
    tools?(value: T, refresh: () => void): ReactNode;
    changes?: readonly ChangeScope[];
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
    load: sourceLoad,
    children,
    action,
    tools,
    changes = []
}: UtilityWorkspaceProps<T>) {
    const { t } = useTranslation();
    const reconnect = useReconnect();
    const events = useCoreEvents();
    const scopeKey = changes.join("|");
    const load = useCallback(() => events && scopeKey ? events.consistentRead(scopeKey.split("|"), sourceLoad) : sourceLoad(), [events, scopeKey, sourceLoad]);
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
    const waiters = useRef<Array<{ resolve(): void; reject(error: unknown): void }>>([]);
    const readError = useRef<unknown>(undefined);

    useEffect(() => {
        if (initial !== undefined && generation === 0 && load === initialLoader) return;
        let active = true;
        pendingRef.current = true;
        readError.current = undefined;
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
            readError.current = caught;
            setState((current) => ({ ...current, pending: queuedRefreshRef.current, error: normalizeRequestError(caught) }));
        }).finally(() => {
            if (!active) return;
            if (queuedRefreshRef.current) {
                queuedRefreshRef.current = false;
                setGeneration((current) => current + 1);
            } else {
                pendingRef.current = false;
                waiters.current.splice(0).forEach((waiter) => readError.current === undefined ? waiter.resolve() : waiter.reject(readError.current));
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
    const refreshAndWait = useCallback(() => new Promise<void>((resolve, reject) => {
        waiters.current.push({ resolve, reject });
        refresh();
    }), [refresh]);
    const retryRead = useWorkspaceInvalidation(changes, refreshAndWait);
    useEffect(() => () => { waiters.current.splice(0).forEach((waiter) => waiter.resolve()); }, []);

    // A new route's loader must never display the previous route's data.
    const value = state.loader === load ? state.value : undefined;
    const error = state.loader === load ? state.error : undefined;
    const pending = state.loader !== load || state.pending;
    useEffect(() => { if (value !== undefined) events?.noteDisplayedData(); }, [events, value]);
    const retry = () => {
        if (error?.code === "daemon_unavailable") reconnect?.();
        if (events) retryRead(); else refresh();
    };

    const failure = error === undefined ? null : <div role="alert"><p>{t(requestErrorMessage((error).code))}</p><code>{(error).code}</code>{error.diagnosticId === undefined ? null : <p><code>{error.diagnosticId}</code></p>}
        <p>{t("This page could not be loaded.")}</p>
        <Button disabled={pending} onClick={retry} type="button" variant="text">{t("Retry")}</Button>
    </div>;

    const toolbar = value === undefined ? null : tools?.(value, refresh);
    return (
        <section className="utility-workspace page-surface--utility" aria-busy={value === undefined && pending} aria-labelledby="route-title">
            <PageHeader className="utility-workspace-toolbar" leading={back} title={title}>
                {navigation}
                {action === undefined && toolbar == null ? null : <div className="utility-workspace-actions">
                    {toolbar}
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
                </div>}
            </PageHeader>
            <ScrollArea className="utility-workspace-body">
                {value === undefined ? failure : null}
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
