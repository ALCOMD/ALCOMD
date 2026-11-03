import { createContext, useCallback, useContext, useEffect, useId, useLayoutEffect, useMemo, useRef, useState, useSyncExternalStore, type ReactNode, type Ref } from "react";
import { createPortal } from "react-dom";
import type { RpcError } from "@alcomd/sdk";
import { closeIcon, infoIcon } from "@alcomd/ui/icons";
import { Button, DialogOpenContext, Icon, IconButton } from "./Material";
import { ScrollArea } from "./ScrollArea";
import { useTranslation } from "./i18n";
import { normalizeRequestError, requestErrorMessage } from "./request-errors";
import { currentDialogSurface, subscribeDialogSurface } from "./dialog-surface";
import "./Reminder.css";

interface ReminderActions {
    success(message: string): void;
    error(error: RpcError, message?: string, origin?: HTMLElement): void;
    clear(): void;
}

const ReminderContext = createContext<ReminderActions | undefined>(undefined);
const ReminderChannelContext = createContext<{
    publish(source: string, content: ReactNode): void;
    remove(source: string): void;
} | undefined>(undefined);
const InReminderOutlet = createContext(false);
const ReminderSourceContext = createContext("shell");
type ReminderIdentity = object | string;
const ReminderDismissContext = createContext<{
    has(source: string, identity: ReminderIdentity): boolean;
    dismiss(source: string, identity: ReminderIdentity): void;
} | undefined>(undefined);

/** One outlet; success keeps its visit scope while actionable feedback follows its source. */
export function ReminderProvider({ children, feedback, visit }: { children: ReactNode; feedback?: ReactNode; visit: number }) {
    const dialogSurface = useSyncExternalStore(subscribeDialogSurface, currentDialogSurface);
    const [notice, setNotice] = useState<{ id: number; message?: string; error?: RpcError; visit: number; origin?: HTMLElement }>();
    const [feedbackBySource, setFeedbackBySource] = useState<ReadonlyMap<string, ReactNode>>(() => new Map());
    const dismissedObjects = useRef(new WeakSet<object>());
    const [dismissedVersion, setDismissedVersion] = useState(0);
    const [dismissedMessages, setDismissedMessages] = useState<ReadonlyMap<string, ReadonlySet<string>>>(() => new Map());
    const [paused, setPaused] = useState(false);
    const sequence = useRef(0);
    const activeVisit = useRef(visit);
    activeVisit.current = visit;
    const publish = useCallback((source: string, content: ReactNode) => {
        setFeedbackBySource((current) => new Map(current).set(source, content));
    }, []);
    const remove = useCallback((source: string) => {
        setFeedbackBySource((current) => {
            if (!current.has(source)) return current;
            const next = new Map(current);
            next.delete(source);
            return next;
        });
        setDismissedMessages((current) => {
            if (!current.has(source)) return current;
            const next = new Map(current);
            next.delete(source);
            return next;
        });
    }, []);
    const channel = useMemo(() => ({ publish, remove }), [publish, remove]);
    const dismissals = useMemo(() => ({
        has: (source: string, identity: ReminderIdentity) => typeof identity === "string"
            ? dismissedMessages.get(source)?.has(identity) === true : dismissedObjects.current.has(identity),
        dismiss: (source: string, identity: ReminderIdentity) => {
            if (typeof identity === "string") {
                setDismissedMessages((current) => new Map(current).set(source, new Set([...(current.get(source) ?? []), identity])));
            } else if (!dismissedObjects.current.has(identity)) {
                dismissedObjects.current.add(identity);
                setDismissedVersion((version) => version + 1);
            }
        }
    }), [dismissedMessages, dismissedVersion]);
    const success = useCallback((message: string) => {
        if (activeVisit.current !== visit) return;
        setPaused(false);
        setNotice({ id: ++sequence.current, message, visit });
    }, [visit]);
    const clear = useCallback(() => { if (activeVisit.current === visit) setNotice(undefined); }, [visit]);
    const error = useCallback((failure: RpcError, message?: string, origin?: HTMLElement) => {
        if (activeVisit.current !== visit) return;
        setNotice({ id: ++sequence.current, error: failure, message, visit,
            origin: origin ?? (document.activeElement instanceof HTMLElement ? document.activeElement : undefined) });
    }, [visit]);
    const actions = useMemo(() => ({ success, error, clear }), [success, error, clear]);
    useEffect(() => { setNotice(undefined); setPaused(false); }, [visit]);
    useEffect(() => {
        if (notice === undefined || notice.error !== undefined || paused) return;
        const timer = window.setTimeout(() => {
            setNotice((current) => current?.id === notice.id ? undefined : current);
        }, 5000);
        return () => window.clearTimeout(timer);
    }, [notice, paused]);
    const visibleNotice = notice?.visit === visit ? notice : undefined;
    const dismissError = () => {
        if (activeVisit.current !== visit) return;
        clear();
        const target = notice?.origin;
        if (target?.isConnected && !target.hasAttribute("disabled")) target.focus();
    };
    return <ReminderContext.Provider value={actions}><ReminderChannelContext.Provider value={channel}>{children}{createPortal(
        <div className="reminder-floating" onMouseEnter={() => setPaused(true)} onMouseLeave={() => setPaused(false)} onFocusCapture={() => setPaused(true)} onBlurCapture={(event) => {
            if (!event.currentTarget.contains(event.relatedTarget)) setPaused(false);
        }}>
            <InReminderOutlet.Provider value={true}><ReminderDismissContext.Provider value={dismissals}>{feedback}
            {Array.from(feedbackBySource, ([source, content]) => <ReminderSourceContext.Provider key={source} value={source}><div className="reminder-source">{content}</div></ReminderSourceContext.Provider>)}
            {visibleNotice === undefined ? null : visibleNotice.error === undefined
                ? <Reminder key={visibleNotice.id} onDismiss={clear} tone="success">{visibleNotice.message}</Reminder>
                : <RequestReminder key={visibleNotice.id} error={visibleNotice.error} message={visibleNotice.message} onDismiss={dismissError} />}</ReminderDismissContext.Provider></InReminderOutlet.Provider>
        </div>, dialogSurface ?? document.body
    )}</ReminderChannelContext.Provider></ReminderContext.Provider>;
}

/** Publish page-owned feedback; its source clears it when resolved, hidden or unmounted. */
export function GlobalReminder({ children, ref }: { children: ReactNode; ref?: Ref<HTMLDivElement> }) {
    const channel = useContext(ReminderChannelContext);
    const dialogOpen = useContext(DialogOpenContext);
    const source = useId();
    if (channel === undefined) throw new Error("Missing ReminderProvider");
    useLayoutEffect(() => {
        if (dialogOpen === false || children === null || children === undefined) { channel.remove(source); return; }
        channel.publish(source, ref === undefined ? children : <div ref={ref}>{children}</div>);
    }, [channel, children, dialogOpen, ref, source]);
    useLayoutEffect(() => () => channel.remove(source), [channel, source]);
    return null;
}

export function useReminder(): ReminderActions {
    const context = useContext(ReminderContext);
    if (context === undefined) throw new Error("Missing ReminderProvider");
    return context;
}

/** Persistent contextual feedback. Only ReminderProvider's short success message expires. */
export function Reminder({ actions, children, details, identity, onDismiss, title, tone = "info" }: {
    actions?: ReactNode;
    children: ReactNode;
    details?: ReactNode;
    identity?: object;
    onDismiss?(): void;
    title?: string;
    tone?: "success" | "info" | "warning" | "error";
}) {
    const { t } = useTranslation();
    const [expanded, setExpanded] = useState(false);
    const floating = useContext(InReminderOutlet);
    const source = useContext(ReminderSourceContext);
    const dismissals = useContext(ReminderDismissContext);
    const notification = identity ?? JSON.stringify([tone, title, typeof children === "string" ? children : undefined]);
    if (floating && onDismiss === undefined && dismissals?.has(source, notification)) return null;
    const dismiss = onDismiss ?? (floating ? () => dismissals?.dismiss(source, notification) : undefined);
    return <section className={`reminder reminder--${tone}`} role={tone === "error" || tone === "warning" ? "alert" : "status"} aria-live={tone === "error" || tone === "warning" ? "assertive" : "polite"}>
        <Icon asset={infoIcon} />
        <div className="reminder-body">
            {title === undefined ? null : <strong className="reminder-title">{title}</strong>}
            <div>{children}</div>
            {details === undefined ? null : <>
                <Button aria-expanded={expanded} onClick={() => setExpanded((value) => !value)} type="button" variant="text">{expanded ? t("Hide details") : t("Show details")}</Button>
                {expanded ? <ScrollArea className="reminder-details">{details}</ScrollArea> : null}
            </>}
            {actions === undefined ? null : <div className="reminder-actions">{actions}</div>}
        </div>
        {dismiss === undefined ? null : <IconButton label={t("Dismiss notification")} onClick={dismiss} type="button"><Icon asset={closeIcon} /></IconButton>}
    </section>;
}

export function RequestReminder({ children, error, inline = false, message, onDismiss, title }: { children?: ReactNode; error: RpcError; inline?: boolean; message?: string; onDismiss?(): void; title?: string | null }) {
    const { t } = useTranslation();
    const inOutlet = useContext(InReminderOutlet);
    const safe = normalizeRequestError(error);
    const confirmation = safe.code === "extension_publisher_confirmation_required";
    const notice = <Reminder key={`${safe.code}:${safe.diagnosticId ?? ""}`} identity={error} onDismiss={onDismiss} tone={confirmation ? "warning" : "error"} title={title === null ? undefined : title ?? t(confirmation ? "Publisher confirmation required" : "Request failed")} details={<DiagnosticDetails code={safe.code} diagnosticId={safe.diagnosticId} />}>
        <p>{t(message ?? requestErrorMessage(safe.code))}</p>
        {children === undefined ? null : <div className="reminder-context">{children}</div>}
    </Reminder>;
    return inline || inOutlet ? notice : <GlobalReminder>{notice}</GlobalReminder>;
}

export function DiagnosticDetails({ code, diagnosticId }: { code: string; diagnosticId?: string }) {
    const { t } = useTranslation();
    return <><strong>{t("Error code")}</strong><code>{code}</code>
        {diagnosticId === undefined ? null : <><strong>{t("Diagnostic ID")}</strong><code>{diagnosticId}</code></>}
    </>;
}
