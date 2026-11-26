import { invoke, isTauri } from "@tauri-apps/api/core";
import { useEffect, useId, useRef, useState } from "react";

import { useTranslation } from "./i18n";
import { Button } from "./Material";

type Action = "status" | "enable" | "remove";
type Status = "unsupported" | "disabled" | "enabled" | "needsRepair" | "conflict";

const statusMessages = {
    disabled: "Enable the alcomd-cli command for this account.",
    enabled: "The alcomd-cli command is available in ~/.local/bin.",
    needsRepair: "The command needs repair. If the AppImage moved, reopen it from its current location.",
    conflict: "An existing file conflicts with CLI integration. ALCOMD preserved it.",
} as const;

function portableCli(action: Action): Promise<Status> {
    return invoke<Status>("gui_portable_cli", { action });
}

export function PortableCliSection() {
    const { t } = useTranslation();
    const titleId = useId();
    const [status, setStatus] = useState<Status>("unsupported");
    const [pending, setPending] = useState(false);
    const [failed, setFailed] = useState(false);
    const acting = useRef(false);

    useEffect(() => {
        if (!isTauri()) return;
        let mounted = true;
        void portableCli("status").then((result) => {
            if (mounted) setStatus(result);
        }).catch(() => undefined);
        return () => { mounted = false; };
    }, []);

    async function act(action: Exclude<Action, "status">) {
        if (acting.current) return;
        acting.current = true;
        setPending(true);
        setFailed(false);
        try {
            const result = await portableCli(action);
            setStatus(result);
            setFailed(result === "needsRepair");
        } catch {
            setFailed(true);
        } finally {
            acting.current = false;
            setPending(false);
        }
    }

    if (status === "unsupported") return null;
    const primaryAction = status === "enabled" ? "remove" : "enable";
    const primaryLabel = status === "enabled"
        ? "Remove CLI command"
        : status === "needsRepair" ? "Repair CLI command" : "Enable CLI command";

    return <section className="notice-card" aria-labelledby={titleId}>
        <h2 id={titleId}>{t("Command-line integration")}</h2>
        <p role="status">{t(pending ? "Updating CLI integration…" : statusMessages[status])}</p>
        {failed && <p role="alert">{t("CLI integration could not be changed. Reopen the AppImage and retry.")}</p>}
        {status !== "conflict" && <Button type="button" disabled={pending} onClick={() => void act(primaryAction)}>{t(primaryLabel)}</Button>}
        {status === "needsRepair" && <Button type="button" disabled={pending} onClick={() => void act("remove")}>{t("Remove CLI command")}</Button>}
    </section>;
}
