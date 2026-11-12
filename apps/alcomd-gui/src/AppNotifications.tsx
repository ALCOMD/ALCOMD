import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import { closeIcon } from "@alcomd/ui/icons";
import { Button, Icon, IconButton } from "./Material";
import { activeNotificationDialog, AppNotifications, notificationAlreadyPresented } from "./app-notifications";
import { useTranslation } from "./i18n";
import { requestErrorMessage } from "./request-errors";
import "./AppNotifications.css";

export function AppNotificationOutlet({ notifications, navigate }: { notifications: AppNotifications; navigate(path: string): void }) {
    const { t } = useTranslation();
    const queue = useSyncExternalStore(notifications.subscribe, notifications.getSnapshot);
    const [blocked, setBlocked] = useState(false);
    const [paused, setPaused] = useState(false);
    const [hidden, setHidden] = useState(document.visibilityState !== "visible");
    const [announcement, setAnnouncement] = useState({ text: "", error: false });
    const root = useRef<HTMLElement>(null);
    const previousFocus = useRef<HTMLElement | undefined>(undefined);
    const announced = useRef<number | undefined>(undefined);
    const current = queue[0];
    const visible = current !== undefined && !blocked;
    useEffect(() => { notifications.start(); return () => notifications.stop(); }, [notifications]);
    useEffect(() => {
        const update = () => {
            for (const item of notifications.getSnapshot()) if (notificationAlreadyPresented(item)) notifications.dismiss(item.id);
            setBlocked(activeNotificationDialog() !== undefined);
        };
        const observer = new MutationObserver(update);
        observer.observe(document.body, { subtree: true, childList: true, attributes: true,
            attributeFilter: ["open", "data-dialog-closing", "data-notification-error", "data-notification-result", "data-notification-task"] });
        const unsubscribe = notifications.subscribe(update);
        update();
        return () => { observer.disconnect(); unsubscribe(); };
    }, [notifications]);
    useEffect(() => {
        const focus = (event: FocusEvent) => {
            if (event.target instanceof HTMLElement && !root.current?.contains(event.target)) previousFocus.current = event.target;
        };
        const visibility = () => setHidden(document.visibilityState !== "visible");
        if (document.activeElement instanceof HTMLElement) previousFocus.current = document.activeElement;
        document.addEventListener("focusin", focus);
        document.addEventListener("visibilitychange", visibility);
        return () => { document.removeEventListener("focusin", focus); document.removeEventListener("visibilitychange", visibility); };
    }, []);
    const title = current === undefined ? "" : current.outcome === "error"
        ? t("Could not complete: {action}", { action: t(current.action) })
        : current.outcome === "cancelled" ? t("Cancelled: {action}", { action: t(current.action) })
        : t("Completed: {action}", { action: t(current.action) });
    const detail = current?.summary ? t("{succeeded} succeeded · {failed} failed or not completed", current.summary)
        : current?.error ? t(requestErrorMessage(current.error.code)) : "";
    const uncertain = current?.error && ["daemon_unavailable", "internal_error"].includes(current.error.code) && current.operationId === undefined;
    useEffect(() => {
        const element = root.current;
        if (!element) return;
        if (typeof element.showPopover === "function") {
            if (visible && !element.matches(":popover-open")) element.showPopover();
            else if (!visible && element.matches(":popover-open")) element.hidePopover();
        } else {
            element.dataset.fallback = "true";
            element.hidden = !visible;
        }
    }, [visible]);
    useEffect(() => {
        if (!visible) { setAnnouncement((value) => value.text ? { text: "", error: false } : value); return; }
        if (hidden || !current || announced.current === current.id) return;
        announced.current = current.id;
        const text = `${title}${detail ? " " + detail : ""}${uncertain ? " " + t("The result could not be confirmed. Check the current state before retrying.") : ""}${current.operationId ? " " + t("Actions available") : ""}`;
        const announcer = document as Document & { ariaNotify?(text: string, options: { priority: "high" | "normal" }): void };
        if (typeof announcer.ariaNotify === "function") announcer.ariaNotify(text, { priority: current.outcome === "error" ? "high" : "normal" });
        else setAnnouncement({ text, error: current.outcome === "error" });
    }, [current, detail, hidden, t, title, uncertain, visible]);
    useEffect(() => {
        if (!visible || !current || current.outcome === "error" || paused || hidden) return;
        const timer = window.setTimeout(() => {
            if (!root.current?.matches(":focus-within")) notifications.dismiss(current.id);
        }, 4000);
        return () => window.clearTimeout(timer);
    }, [current, hidden, notifications, paused, visible]);
    const dismiss = (restoreFocus: boolean) => {
        if (!current) return;
        const focused = root.current?.matches(":focus-within");
        notifications.dismiss(current.id);
        if (restoreFocus && focused && !activeNotificationDialog() && previousFocus.current?.isConnected) previousFocus.current.focus();
    };
    return <>
        <div className="app-notifications-announcer" aria-live="polite" aria-atomic="true">{announcement.error ? "" : announcement.text}</div>
        <div className="app-notifications-announcer" aria-live="assertive" aria-atomic="true">{announcement.error ? announcement.text : ""}</div>
        <section ref={root} popover="manual" role="region" aria-label={t("Notifications")} className="app-notifications"
            data-outcome={current?.outcome} onPointerEnter={() => setPaused(true)} onPointerLeave={() => setPaused(false)}
            onFocusCapture={() => setPaused(true)} onBlurCapture={() => queueMicrotask(() => setPaused(root.current?.matches(":focus-within") ?? false))}>
            {current === undefined ? null : <>
                <div className="app-notifications-content"><strong>{title}</strong>{detail ? <p>{detail}</p> : null}
                    {uncertain ? <p>{t("The result could not be confirmed. Check the current state before retrying.")}</p> : null}
                </div>
                <div className="app-notifications-actions">
                    {current.operationId === undefined ? null : <Button type="button" variant="text" onClick={() => {
                        const path = `/operations/${current.operationId}`;
                        dismiss(false); navigate(path);
                    }}>{t("View operation")}</Button>}
                    <IconButton label={t("Dismiss notification")} title={t("Dismiss notification")} type="button" onClick={() => dismiss(true)}><Icon asset={closeIcon} /></IconButton>
                </div>
            </>}
        </section>
    </>;
}
