import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import type { RpcError } from "@alcomd/sdk";
import { closeIcon, infoIcon } from "@alcomd/ui/icons";
import { Button, Icon, IconButton } from "./Material";
import { ScrollArea } from "./ScrollArea";
import { useTranslation } from "./i18n";
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

export function RequestReminder({ children, error, title }: { children?: ReactNode; error: RpcError; title?: string }) {
    const { t } = useTranslation();
    return <Reminder tone="error" title={title ?? t("Request failed")} details={error.diagnosticId === undefined ? undefined : <><strong>{t("Diagnostic ID")}</strong><code>{error.diagnosticId}</code></>}>
        <p>{children ?? t("The request could not be completed. Review the details before trying again.")}</p>
        <code className="reminder-code">{error.code}</code>
    </Reminder>;
}
