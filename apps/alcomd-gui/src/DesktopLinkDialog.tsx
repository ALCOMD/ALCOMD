import { invoke, isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import { ExtensionInstallPanel, RegisterRepositoryPanel } from "./CoreActions";
import { UtilityActionDialog } from "./UtilityActionDialog";
import { ImportRepositoryList } from "./RepositoryListDialog";
import { ImportDesktopTemplate } from "./ProjectTemplatesWorkspace";
import { currentDialogSurface, subscribeDialogSurface } from "./dialog-surface";
import { useTranslation } from "./i18n";
import { parseRepositoryLink, type RepositoryLinkDraft } from "./repository-link";
import type { GuiRpcClient } from "./rpc";

export type DesktopLink =
    | { kind: "vccRepository"; link: string }
    | { kind: "repository"; url: string }
    | { kind: "extension"; url: string }
    | { kind: "repositoryList"; path: string }
    | { kind: "templateFile"; path: string };

export interface DesktopLinkInbox {
    take(): Promise<DesktopLink | null>;
    listen(available: () => void): Promise<() => void>;
}

export const nativeDesktopLinks: DesktopLinkInbox | undefined = isTauri() ? {
    take: () => invoke("gui_take_desktop_link"),
    listen: (available) => getCurrentWindow().listen("alcomd-desktop-link-available", available),
} : undefined;

type LinkDraft = { kind: "repository"; initial: RepositoryLinkDraft } | Extract<DesktopLink, { kind: "extension" | "repositoryList" | "templateFile" }>;

export function DesktopLinkDialog({ client, inbox, blocked }: { client: GuiRpcClient; inbox: DesktopLinkInbox; blocked: boolean }) {
    const { t } = useTranslation();
    const [draft, setDraft] = useState<LinkDraft>();
    const [arrival, setArrival] = useState(0);
    const surface = useSyncExternalStore(subscribeDialogSurface, currentDialogSurface);
    const nextLink = useRef<Promise<DesktopLink | null> | undefined>(undefined);

    useEffect(() => {
        let active = true;
        let unlisten: (() => void) | undefined;
        const notify = () => { if (active) setArrival((previous) => previous + 1); };
        void inbox.listen(notify).then((stop) => {
            if (!active) { stop(); return; }
            unlisten = stop;
            // Recheck the queue after the listener is live, including cold start.
            notify();
        }, notify);
        return () => { active = false; unlisten?.(); };
    }, [inbox]);

    useEffect(() => {
        if (blocked || draft !== undefined || surface !== undefined) return;
        let active = true;
        const receive = async () => {
            while (active && currentDialogSurface() === undefined) {
                // A rerender or another dialog opening can interrupt the read
                // after the native side has taken the link out of its queue.
                const previousRead = nextLink.current;
                const link = await (nextLink.current ??= inbox.take());
                if (!active || currentDialogSurface() !== undefined) return;
                nextLink.current = undefined;
                if (link === null) {
                    // A newer arrival may have restarted an effect whose read found an empty queue.
                    if (previousRead !== undefined) continue;
                    return;
                }
                if (link.kind === "extension" || link.kind === "repositoryList" || link.kind === "templateFile") {
                    setDraft(link);
                    return;
                }
                const initial = link.kind === "vccRepository" ? parseRepositoryLink(link.link) : { url: link.url, headers: [] };
                if (initial !== undefined) {
                    setDraft({ kind: "repository", initial });
                    return;
                }
            }
        };
        void receive().catch(() => { if (active) nextLink.current = undefined; });
        return () => { active = false; };
    }, [arrival, blocked, draft, inbox, surface]);

    if (draft === undefined) return null;
    if (draft.kind === "repositoryList") {
        return <ImportRepositoryList client={client} initialPath={draft.path} onClose={() => setDraft(undefined)} />;
    }
    if (draft.kind === "templateFile") {
        return <ImportDesktopTemplate client={client} path={draft.path} onClose={() => setDraft(undefined)} />;
    }
    return <UtilityActionDialog title={t(draft.kind === "repository" ? "Add repository" : "Install extension")} onClose={() => setDraft(undefined)}>
        {draft.kind === "repository"
            ? <RegisterRepositoryPanel client={client} initial={draft.initial} />
            : <ExtensionInstallPanel client={client} initialUrl={draft.url} />}
    </UtilityActionDialog>;
}
