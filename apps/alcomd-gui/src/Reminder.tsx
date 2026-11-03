import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode, type Ref } from "react";
import { createPortal } from "react-dom";
import type { RpcError } from "@alcomd/sdk";
import { closeIcon, infoIcon } from "@alcomd/ui/icons";
import { Button, Icon, IconButton } from "./Material";
import { ScrollArea } from "./ScrollArea";
import { useTranslation } from "./i18n";
import { normalizeRequestError, requestErrorMessage } from "./request-errors";
import "./Reminder.css";

interface ReminderActions {
    success(message: string): void;
    error(error: RpcError, message?: string): void;
    clear(): void;
}

const ReminderContext = createContext<ReminderActions | undefined>(undefined);
const ReminderOutletContext = createContext<HTMLDivElement | null | undefined>(undefined);

/** One outlet; success keeps its visit scope while actionable feedback follows its source. */
export function ReminderProvider({ children, feedback, visit }: { children: ReactNode; feedback?: ReactNode; visit: number }) {
    const [notice, setNotice] = useState<{ id: number; message?: string; error?: RpcError; visit: number }>();
    const [outlet, setOutlet] = useState<HTMLDivElement | null>(null);
    const [paused, setPaused] = useState(false);
    const sequence = useRef(0);
    const activeVisit = useRef(visit);
    activeVisit.current = visit;
    const success = useCallback((message: string) => {
        if (activeVisit.current !== visit) return;
        setPaused(false);
        setNotice({ id: ++sequence.current, message, visit });
    }, [visit]);
    const clear = useCallback(() => { if (activeVisit.current === visit) setNotice(undefined); }, [visit]);
    const error = useCallback((failure: RpcError, message?: string) => {
        if (activeVisit.current !== visit) return;
        setNotice({ id: ++sequence.current, error: failure, message, visit });
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
    return <ReminderContext.Provider value={actions}><ReminderOutletContext.Provider value={outlet}>{children}{createPortal(
        <div className="reminder-floating" ref={setOutlet} onMouseEnter={() => setPaused(true)} onMouseLeave={() => setPaused(false)} onFocusCapture={() => setPaused(true)} onBlurCapture={(event) => {
            if (!event.currentTarget.contains(event.relatedTarget)) setPaused(false);
        }}>
            {feedback}
            {visibleNotice === undefined ? null : visibleNotice.error === undefined
                ? <Reminder key={visibleNotice.id} onDismiss={clear} tone="success">{visibleNotice.message}</Reminder>
                : <RequestReminder key={visibleNotice.id} error={visibleNotice.error} message={visibleNotice.message} onDismiss={clear} />}
        </div>, document.body
    )}</ReminderOutletContext.Provider></ReminderContext.Provider>;
}

/** Keep actionable page feedback mounted in the shared outlet, owned by its page. */
export function GlobalReminder({ children, ref }: { children: ReactNode; ref?: Ref<HTMLDivElement> }) {
    const outlet = useContext(ReminderOutletContext);
    if (outlet === undefined) throw new Error("Missing ReminderProvider");
    return outlet === null || children === null || children === undefined ? null : createPortal(ref === undefined ? children : <div ref={ref}>{children}</div>, outlet);
}

export function useReminder(): ReminderActions {
    const context = useContext(ReminderContext);
    if (context === undefined) throw new Error("Missing ReminderProvider");
    return context;
}

/** Persistent contextual feedback. Only ReminderProvider's short success message expires. */
export function Reminder({ actions, children, details, onDismiss, title, tone = "info" }: {
    actions?: ReactNode;
    children: ReactNode;
    details?: ReactNode;
    onDismiss?(): void;
    title?: string;
    tone?: "success" | "info" | "warning" | "error";
}) {
    const { t } = useTranslation();
    const [expanded, setExpanded] = useState(false);
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
        {onDismiss === undefined ? null : <IconButton label={t("Dismiss notification")} onClick={onDismiss} type="button"><Icon asset={closeIcon} /></IconButton>}
    </section>;
}

export function RequestReminder({ children, error, message, onDismiss, title }: { children?: ReactNode; error: RpcError; message?: string; onDismiss?(): void; title?: string | null }) {
    const { t } = useTranslation();
    const safe = normalizeRequestError(error);
    const confirmation = safe.code === "extension_publisher_confirmation_required";
    return <Reminder key={`${safe.code}:${safe.diagnosticId ?? ""}`} onDismiss={onDismiss} tone={confirmation ? "warning" : "error"} title={title === null ? undefined : title ?? t(confirmation ? "Publisher confirmation required" : "Request failed")} details={<>
        <strong>{t("Error code")}</strong><code>{safe.code}</code>
        {safe.diagnosticId === undefined ? null : <><strong>{t("Diagnostic ID")}</strong><code>{safe.diagnosticId}</code></>}
    </>}>
        <p>{t(message ?? requestErrorMessage(safe.code))}</p>
        {children === undefined ? null : <div className="reminder-context">{children}</div>}
    </Reminder>;
}
