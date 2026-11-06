import { invoke, isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCallback, useEffect, useRef, useState } from "react";
import { RegisterRepositoryPanel } from "./CoreActions";
import { UtilityActionDialog } from "./UtilityActionDialog";
import { useTranslation } from "./i18n";
import { parseRepositoryLink, type RepositoryLinkDraft } from "./repository-link";
import type { GuiRpcClient } from "./rpc";

export interface RepositoryLinkInbox {
    take(): Promise<string | null>;
    listen(available: () => void): Promise<() => void>;
}

export const nativeRepositoryLinks: RepositoryLinkInbox | undefined = isTauri() ? {
    take: () => invoke("gui_take_repository_link"),
    listen: (available) => getCurrentWindow().listen("alcomd-repository-link-available", available),
} : undefined;

export function RepositoryLinkDialog({ client, inbox }: { client: GuiRpcClient; inbox: RepositoryLinkInbox }) {
    const { t } = useTranslation();
    const [draft, setDraft] = useState<RepositoryLinkDraft>();
    const state = useRef({ open: false, reading: false, pending: false, generation: 0 });
    const receive = useCallback(async () => {
        const current = state.current;
        current.pending = true;
        if (current.open || current.reading) return;
        current.reading = true;
        try {
            while (current.pending) {
                current.pending = false;
                const input = await inbox.take();
                if (input === null) continue;
                const draft = parseRepositoryLink(input);
                if (draft) {
                    current.open = true;
                    current.generation += 1;
                    setDraft(draft);
                    return;
                }
                current.pending = true;
            }
        } catch {
            // Native teardown or a WebView reload can interrupt an inbox read.
        } finally {
            current.reading = false;
        }
    }, [inbox]);

    useEffect(() => {
        let active = true;
        let unlisten: (() => void) | undefined;
        void inbox.listen(() => { if (active) void receive(); }).then((stop) => {
            if (!active) { stop(); return; }
            unlisten = stop;
            void receive();
        }, () => { if (active) void receive(); });
        return () => { active = false; unlisten?.(); };
    }, [inbox, receive]);

    if (!draft) return null;
    return <UtilityActionDialog key={state.current.generation} title={t("Add repository")} onClose={() => {
        state.current.open = false;
        setDraft(undefined);
        void receive();
    }}>
        <RegisterRepositoryPanel client={client} initial={draft} />
    </UtilityActionDialog>;
}
