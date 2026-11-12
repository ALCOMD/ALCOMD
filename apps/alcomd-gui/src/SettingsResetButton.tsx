import { requestErrorMessage } from "./request-errors";
import { createContext, useContext, useEffect, useRef, useState, type ReactNode } from "react";
import type { RpcError } from "@alcomd/sdk";
import { DialogCloseButton, Button, Dialog } from "./Material";
import { useTranslation } from "./i18n";
import { normalizeRequestError } from "./request-errors";

const ResetContext = createContext<{ current: boolean } | undefined>(undefined);

export function SettingsResetBoundary({ children }: { children: ReactNode }) {
    const pending = useRef(false);
    return <ResetContext.Provider value={pending}>{children}</ResetContext.Provider>;
}

export function useSettingsResetPending() { return useContext(ResetContext); }

export function isSettingsResetTrigger(target: EventTarget | null): boolean {
    while (target instanceof Node) {
        if (target instanceof Element && target.closest("[data-settings-reset-trigger]") !== null) return true;
        const root = target.getRootNode();
        target = root instanceof ShadowRoot ? root.host : null;
    }
    return false;
}

export function SettingsResetButton({ title, disabled, onRestore }: {
    title: string; disabled?: boolean; onRestore(): Promise<unknown>;
}) {
    const { t } = useTranslation();
    const scope = useSettingsResetPending();
    const origin = useRef<HTMLElement>(null);
    const returnFocus = useRef({ get current() {
        const button = origin.current as (HTMLElement & { disabled: boolean }) | null;
        return button && !button.disabled ? button : button?.closest(".settings-section")?.querySelector<HTMLElement>("h2") ?? null;
    } });
    const action = useRef(onRestore);
    const pending = useRef(false);
    const [shown, setShown] = useState(false);
    const [open, setOpen] = useState(false);
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState<RpcError>();
    useEffect(() => () => { if (scope) scope.current = false; }, [scope]);
    const restore = async () => {
        if (pending.current || disabled) return;
        pending.current = true; setBusy(true); setError(undefined);
        try { await action.current(); setOpen(false); }
        catch (caught: unknown) { setError(normalizeRequestError(caught)); }
        finally { pending.current = false; setBusy(false); }
    };
    return <>
        <Button ref={origin} data-settings-reset-trigger="true" disabled={disabled} type="button" variant="text"
            onPointerDown={(event) => { event.preventDefault(); }}
            onClick={() => {
                if (disabled || scope?.current) return;
                if (scope) scope.current = true;
                action.current = onRestore;
                setError(undefined); setShown(true); setOpen(true);
            }}>{t("Restore defaults")}</Button>
        {shown ? <Dialog open={open} returnFocus={returnFocus.current} title={t("Restore defaults")} dismissible={!busy}
            onClose={() => { setOpen(false); setError(undefined); if (scope) scope.current = false; }}>
            <p>{t("Restore the default settings for {name}?", { name: title })}</p>
            <p>{t("Only this card's settings will be reset.")}</p>
            {error === undefined ? null : <div role="alert" data-notification-error={error.code}><strong>{t(error.code === "internal_error" || error.code === "daemon_unavailable" ? "Restore result is unknown" : "Defaults could not be restored.")}</strong><p>{t((error.code === "internal_error" || error.code === "daemon_unavailable" ? "Retry checks the current settings before writing again." : undefined) ?? requestErrorMessage((error).code))}</p><code>{(error).code}</code>{error.diagnosticId === undefined ? null : <p><code>{error.diagnosticId}</code></p>}</div>}
            <div className="dialog-actions">
                <DialogCloseButton disabled={busy} variant="text">{t("Cancel")}</DialogCloseButton>
                <Button disabled={busy || disabled} onClick={() => void restore()} type="button">{t("Restore defaults")}</Button>
            </div>
        </Dialog> : null}
    </>;
}
