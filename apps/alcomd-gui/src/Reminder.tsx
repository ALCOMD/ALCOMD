import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
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
    clear(): void;
}

const ReminderContext = createContext<ReminderActions | undefined>(undefined);

/** One short success message per page visit; actionable results remain in their own context. */
export function ReminderProvider({ children }: { children: ReactNode }) {
    const [notice, setNotice] = useState<{ id: number; message: string }>();
    const [paused, setPaused] = useState(false);
    const sequence = useRef(0);
    const success = useCallback((message: string) => {
        setPaused(false);
        setNotice({ id: ++sequence.current, message });
    }, []);
    const clear = useCallback(() => setNotice(undefined), []);
    const actions = useMemo(() => ({ success, clear }), [success, clear]);
    useEffect(() => {
        if (notice === undefined || paused) return;
        const timer = window.setTimeout(() => {
            setNotice((current) => current?.id === notice.id ? undefined : current);
        }, 5000);
        return () => window.clearTimeout(timer);
    }, [notice, paused]);
    return <ReminderContext.Provider value={actions}>{children}{notice === undefined ? null : createPortal(
        <div className="reminder-floating" onMouseEnter={() => setPaused(true)} onMouseLeave={() => setPaused(false)} onFocusCapture={() => setPaused(true)} onBlurCapture={(event) => {
            if (!event.currentTarget.contains(event.relatedTarget)) setPaused(false);
        }}>
            <Reminder key={notice.id} onDismiss={clear} tone="success">{notice.message}</Reminder>
        </div>, document.body
    )}</ReminderContext.Provider>;
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

export function RequestReminder({ children, error, message, title }: { children?: ReactNode; error: RpcError; message?: string; title?: string | null }) {
    const { t } = useTranslation();
    const safe = normalizeRequestError(error);
    const confirmation = safe.code === "extension_publisher_confirmation_required";
    return <Reminder key={`${safe.code}:${safe.diagnosticId ?? ""}`} tone={confirmation ? "warning" : "error"} title={title === null ? undefined : title ?? t(confirmation ? "Publisher confirmation required" : "Request failed")} details={<>
        <strong>{t("Error code")}</strong><code>{safe.code}</code>
        {safe.diagnosticId === undefined ? null : <><strong>{t("Diagnostic ID")}</strong><code>{safe.diagnosticId}</code></>}
    </>}>
        <p>{t(message ?? requestErrorMessage(safe.code))}</p>
        {children === undefined ? null : <div className="reminder-context">{children}</div>}
    </Reminder>;
}
