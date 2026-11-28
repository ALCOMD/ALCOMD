import { invoke, isTauri } from "@tauri-apps/api/core";
import { useCallback, useEffect, useId, useRef, useState } from "react";

import { useTranslation } from "./i18n";
import type { Translate } from "./i18n-types";
import { Button, Dialog, DialogCloseButton } from "./Material";

type Action = "status" | "check" | "download" | "cancel" | "install" | "resume";
type UpdateStatus =
    | { state: "unsupported" | "idle" | "checking" | "current" | "downloading" | "failed" | "installFailed" | "recovery" | "restarting" }
    | { state: "available"; version: string; bytes: number }
    | { state: "ready"; version: string };

function productUpdate(action: Action): Promise<UpdateStatus> {
    return isTauri()
        ? invoke<UpdateStatus>("gui_product_update", { action })
        : Promise.resolve({ state: "unsupported" });
}

export function ProductUpdateSection() {
    const { locale, t } = useTranslation();
    const titleId = useId();
    const [status, setStatus] = useState<UpdateStatus>({ state: "unsupported" });
    const [pending, setPending] = useState(false);
    const [confirm, setConfirm] = useState<"install" | "resume">();
    const sequence = useRef(0);
    const acting = useRef(false);
    const update = useCallback(async (action: Action) => {
        if (acting.current) return;
        const id = ++sequence.current;
        if (action !== "status") {
            acting.current = true;
            setPending(true);
        }
        try {
            const result = await productUpdate(action);
            if (id === sequence.current) setStatus(result);
        } catch {
            if (id === sequence.current) setStatus({ state: "failed" });
        } finally {
            if (action !== "status") {
                acting.current = false;
                if (id === sequence.current) setPending(false);
            }
        }
    }, []);

    useEffect(() => {
        void update("status");
        return () => { sequence.current++; };
    }, [update]);

    const transfer = status.state === "checking" || status.state === "downloading";
    useEffect(() => {
        if (!transfer) return;
        let active = true;
        let timer: number;
        const refresh = async () => {
            await update("status");
            if (active) timer = window.setTimeout(() => void refresh(), 500);
        };
        timer = window.setTimeout(() => void refresh(), 500);
        return () => {
            active = false;
            window.clearTimeout(timer);
        };
    }, [transfer, update]);

    if (status.state === "unsupported") return null;
    const restart = confirm === "resume" ? t("Resume and restart") : t("Install and restart");

    return <section className="notice-card" aria-labelledby={titleId}>
        <h2 id={titleId}>{t("Product updates")}</h2>
        <p role="status">{updateMessage(status, t, locale)}</p>
        {transfer && <Button type="button" disabled={pending} onClick={() => void update("cancel")}>{t("Cancel")}</Button>}
        {(status.state === "idle" || status.state === "current" || status.state === "failed" || status.state === "installFailed") &&
            <Button type="button" disabled={pending} onClick={() => void update("check")}>{t("Check for updates")}</Button>}
        {status.state === "available" &&
            <Button type="button" disabled={pending} onClick={() => void update("download")}>{t("Download update")}</Button>}
        {status.state === "ready" &&
            <Button type="button" disabled={pending} onClick={() => setConfirm("install")}>{t("Install and restart")}</Button>}
        {status.state === "recovery" &&
            <Button type="button" disabled={pending} onClick={() => setConfirm("resume")}>{t("Resume and restart")}</Button>}
        {confirm && <Dialog open title={restart} onClose={() => setConfirm(undefined)} actions={<>
            <DialogCloseButton variant="text">{t("Cancel")}</DialogCloseButton>
            <DialogCloseButton afterClose={() => {
                setConfirm(undefined);
                void update(confirm);
            }}>{restart}</DialogCloseButton>
        </>}>
            <p>{t("ALCOMD will close to complete the update. Close any other ALCOMD clients before continuing.")}</p>
        </Dialog>}
    </section>;
}

function updateMessage(status: UpdateStatus, t: Translate, locale: string): string {
    switch (status.state) {
        case "checking": return t("Checking for updates…");
        case "current": return t("You are using the latest version.");
        case "available": return t("Version {version} is available ({size} MiB).", {
            version: status.version,
            size: new Intl.NumberFormat(locale, { maximumFractionDigits: 1 }).format(status.bytes / 1048576),
        });
        case "downloading": return t("Downloading the signed update…");
        case "ready": return t("Version {version} is ready to install.", { version: status.version });
        case "failed": return t("The update could not be prepared. Check again to retry.");
        case "installFailed": return t("The update could not finish. Close other ALCOMD clients and check again to retry.");
        case "recovery": return t("An interrupted update is ready to resume.");
        case "restarting": return t("Restarting to complete the update…");
        default: return "";
    }
}
