import { useTranslation } from "./i18n";
import type { RpcError } from "@alcomd/sdk";
import { useEffect, useRef, useState } from "react";

import { Button, Dialog, Select } from "./Material";
import { capabilities, capabilityUnavailableTitle, useCapability } from "./capabilities";
import type { ProjectSnapshot, UnityLaunchOptionsResult } from "./core-models";
import { normalizeRpcError, type GuiRpcClient } from "./rpc";
import { RequestReminder, useReminder } from "./Reminder";
import { displayPath } from "./display-path";

export function useProjectUnityLaunch({ client, initialOptions, navigate, onLaunchSettled, project }: {
    client: GuiRpcClient;
    initialOptions?: UnityLaunchOptionsResult;
    navigate(path: string): void;
    onLaunchSettled?(): void;
    project: ProjectSnapshot;
}) {
    const { t } = useTranslation();
    const { success, error: showError, clear } = useReminder();
    const canLaunch = useCapability(capabilities.unityLaunch);
    const canMigrate = useCapability(capabilities.projectsUnityMigration);
    const [busy, setBusy] = useState(false);
    const [dialog, setDialog] = useState<"missing" | "choose">();
    const [options, setOptions] = useState(initialOptions);
    const [installationId, setInstallationId] = useState("");
    const [error, setError] = useState<RpcError>();
    const pending = useRef(false);
    const generation = useRef(0);
    const trigger = useRef<HTMLElement | null>(null);
    const projectId = project.projectId;
    const revision = project.revision;

    useEffect(() => {
        setBusy(false); setDialog(undefined); setInstallationId(""); setError(undefined);
        return () => { generation.current += 1; pending.current = false; };
    }, [client, projectId, revision]);
    const run = async (task: (current: number) => Promise<void>, errorContext?: "choose") => {
        if (!canLaunch || projectId === undefined || revision === undefined || pending.current) return;
        const current = generation.current;
        pending.current = true; setBusy(true); setError(undefined);
        clear();
        try { await task(current); }
        catch (caught: unknown) {
            if (current !== generation.current) return;
            const failure = normalizeRpcError(caught);
            if (errorContext === undefined) showError(failure, undefined, trigger.current ?? undefined);
            else setError(failure);
        } finally {
            if (current === generation.current) { pending.current = false; setBusy(false); }
        }
    };
    const launch = async (snapshot: UnityLaunchOptionsResult, selected: string, current: number) => {
        if (projectId === undefined) return;
        try {
            const result = await client.unityLaunch(projectId, selected, snapshot.projectRevision);
            if (current !== generation.current) return;
            setDialog(undefined); setInstallationId("");
            success(t("Unity launch {state}.", { state: t(result.launch.state) }));
        } finally {
            if (current === generation.current) onLaunchSettled?.();
        }
    };
    const open = () => {
        if (pending.current) return;
        if (document.activeElement instanceof HTMLElement) trigger.current = document.activeElement;
        void run(async (current) => {
            if (projectId === undefined || revision === undefined) return;
            const snapshot = await client.unityLaunchOptions(projectId, revision);
            if (current !== generation.current) return;
            setOptions(snapshot); setInstallationId("");
            if (snapshot.exactMatchingInstallations.length === 1) {
                await launch(snapshot, snapshot.exactMatchingInstallations[0]!.installationId, current);
            } else if (snapshot.exactMatchingInstallations.length === 0) {
                setDialog("missing");
            } else {
                setDialog("choose");
            }
        });
    };
    const confirm = () => {
        if (options === undefined || !options.exactMatchingInstallations.some((item) => item.installationId === installationId)) return;
        void run((current) => launch(options, installationId, current), "choose");
    };
    const close = () => { if (!pending.current) { setDialog(undefined); setInstallationId(""); setError(undefined); if (dialog !== undefined) clear(); } };
    const closed = (kind: NonNullable<typeof dialog>) => {
        if (dialog !== undefined && dialog !== kind) return;
        close();
        window.requestAnimationFrame(() => { if (trigger.current?.isConnected) trigger.current.focus(); });
    };

    return {
        busy,
        action: {
            disabled: !canLaunch || busy || projectId === undefined || revision === undefined,
            onClick: open,
            title: capabilityUnavailableTitle(canLaunch, capabilities.unityLaunch, t)
        },
        dialogs: <>
            <Dialog onClose={() => closed("missing")} open={dialog === "missing"} title={t("Unity installation required")}>
                <p>{t("This project requires Unity {version}. No exact matching Unity installation was found.", { version: options?.projectUnityVersion ?? project.unityVersion })}</p>
                <div className="dialog-actions">
                    <Button onClick={close} type="button" variant="tonal">{t("Cancel")}</Button>
                    <Button disabled={!canMigrate} onClick={() => { close(); navigate(`/projects/${projectId}/unity?afterMigration=open`); }} title={capabilityUnavailableTitle(canMigrate, capabilities.projectsUnityMigration, t)} type="button">{t("Migrate Project…")}</Button>
                </div>
            </Dialog>
            <Dialog dismissible={!busy} onClose={() => closed("choose")} open={dialog === "choose"} title={t("Choose Unity for this launch")}>
                <Select disabled={busy} label={t("Unity installation for this launch")} onChange={(value) => { setInstallationId(value); setError(undefined); }} options={[{ label: t("Choose an installation"), value: "" }, ...(options?.exactMatchingInstallations ?? []).map((item) => ({ label: `Unity ${item.unityVersion} · ${item.architecture} · ${displayPath(item.executablePath)}`, value: item.installationId }))]} value={installationId} />
                {error === undefined ? null : <RequestReminder error={error} />}
                <div className="dialog-actions">
                    <Button disabled={busy} onClick={close} type="button" variant="tonal">{t("Cancel")}</Button>
                    <Button disabled={!canLaunch || installationId.length === 0 || busy} onClick={confirm} type="button">{t("Open Unity")}</Button>
                </div>
            </Dialog>
        </>
    };
}
