import { useCallback, useState, type ReactNode } from "react";
import type { RpcError } from "@alcomd/sdk";
import { closeIcon, infoIcon } from "@alcomd/ui/icons";
import { Button, Icon, IconButton } from "./Material";
import { ScrollArea } from "./ScrollArea";
import { useTranslation } from "./i18n";
import { normalizeRequestError, requestErrorMessage } from "./request-errors";
import "./Reminder.css";

/** Contextual feedback rendered at its caller. It never publishes or records logs. */
export function Reminder({ actions, children, details, message, onDismiss, title, tone = "info" }: {
    actions?: ReactNode;
    children?: ReactNode;
    details?: ReactNode;
    message?: string;
    onDismiss?(): void;
    title?: string;
    tone?: "success" | "info" | "warning" | "error";
}) {
    const { t } = useTranslation();
    const [expanded, setExpanded] = useState(false);
    return <section className={`reminder reminder--${tone}`} role={tone === "error" || tone === "warning" ? "alert" : "status"}>
        <Icon asset={infoIcon} />
        <div className="reminder-body">
            {title === undefined ? null : <strong className="reminder-title">{title}</strong>}
            <div>{message === undefined ? children : <><p>{t(message)}</p>{children}</>}</div>
            {details === undefined ? null : <>
                <Button aria-expanded={expanded} onClick={() => setExpanded((value) => !value)} type="button" variant="text">{t(expanded ? "Hide details" : "Show details")}</Button>
                {expanded ? <ScrollArea className="reminder-details">{details}</ScrollArea> : null}
            </>}
            {actions === undefined ? null : <div className="reminder-actions">{actions}</div>}
        </div>
        {onDismiss === undefined ? null : <IconButton label={t("Close")} onClick={onDismiss} type="button"><Icon asset={closeIcon} /></IconButton>}
    </section>;
}

export function RequestReminder({ children, error, message, onDismiss, title }: {
    children?: ReactNode; error: RpcError; message?: string; onDismiss?(): void; title?: string | null;
}) {
    const { t } = useTranslation();
    const safe = normalizeRequestError(error);
    const confirmation = safe.code === "extension_publisher_confirmation_required";
    return <Reminder key={`${safe.code}:${safe.diagnosticId ?? ""}`} message={message ?? requestErrorMessage(safe.code)} onDismiss={onDismiss}
        tone={confirmation ? "warning" : "error"} title={title === null ? undefined : title ?? t(confirmation ? "Publisher confirmation required" : "Request failed")}
        details={<DiagnosticDetails code={safe.code} diagnosticId={safe.diagnosticId} />}>
        {typeof safe.data === "object" && safe.data !== null && "activityRecordingUnavailable" in safe.data && safe.data.activityRecordingUnavailable === true ? <p>{t("The activity record for this operation could not be saved.")}</p> : null}
        {children === undefined ? null : <div className="reminder-context">{children}</div>}
    </Reminder>;
}

export function DiagnosticDetails({ code, diagnosticId }: { code: string; diagnosticId?: string }) {
    const { t } = useTranslation();
    return <><strong>{t("Error code")}</strong><code>{code}</code>
        {diagnosticId === undefined ? null : <><strong>{t("Diagnostic ID")}</strong><code>{diagnosticId}</code></>}
    </>;
}

/** Page-owned error state for actions without a persistent form or dialog. */
export function useInlineError() {
    const [failure, setFailure] = useState<{ error: RpcError; message?: string }>();
    const clear = useCallback(() => setFailure(undefined), []);
    const show = useCallback((error: RpcError, message?: string) => setFailure({ error, message }), []);
    return { clear, show, notice: failure === undefined ? null : <RequestReminder error={failure.error} message={failure.message} onDismiss={clear} /> };
}
