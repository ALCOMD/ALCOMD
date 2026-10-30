import { useTranslation } from "./i18n";
import type { RpcError } from "@alcomd/sdk";
import { useEffect, useRef, useState } from "react";

import { Button, Dialog, Select } from "./Material";
import { capabilities, capabilityUnavailableTitle, useCapability } from "./capabilities";
import type { ProjectSnapshot, UnityLaunchOptionsResult } from "./core-models";
import { normalizeRpcError, type GuiRpcClient } from "./rpc";

interface LaunchFeedback {
    busy: boolean;
    error?: RpcError;
    message?: string;
}

export function useProjectUnityLaunch({ client, initialOptions, navigate, onFeedback, onLaunchSettled, project }: {
    client: GuiRpcClient;
    initialOptions?: UnityLaunchOptionsResult;
    navigate(path: string): void;
    onFeedback(feedback: LaunchFeedback): void;
    onLaunchSettled?(): void;
    project: ProjectSnapshot;
}) {
    const { t } = useTranslation();
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
    }, [projectId, revision]);

    const run = async (task: (current: number) => Promise<void>) => {
        if (!canLaunch || projectId === undefined || revision === undefined || pending.current) return;
        const current = generation.current;
        pending.current = true; setBusy(true); setError(undefined);
        onFeedback({ busy: true });
        try { await task(current); }
        catch (caught: unknown) {
            if (current !== generation.current) return;
            const failure = normalizeRpcError(caught);
            setError(failure); onFeedback({ busy: false, error: failure });
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
            onFeedback({ busy: false, message: t("Unity launch {state}.", { state: t(result.launch.state) }) });
        } finally {
            if (current === generation.current) onLaunchSettled?.();
        }
    };
    const open = () => {
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
                onFeedback({ busy: false, message: t("Unity {version} is required. No exact matching Unity installation was found.", { version: snapshot.projectUnityVersion }) });
            } else {
                setDialog("choose");
                onFeedback({ busy: false, message: t("Choose the Unity installation for this launch.") });
            }
        });
    };
    const confirm = () => {
        if (options === undefined || !options.exactMatchingInstallations.some((item) => item.installationId === installationId)) return;
        void run((current) => launch(options, installationId, current));
    };
    const close = () => { if (!pending.current) { setDialog(undefined); setInstallationId(""); setError(undefined); } };
    const closed = () => {
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
            <Dialog onClose={closed} open={dialog === "missing"} title={t("Unity installation required")}>
                <p>{t("This project requires Unity {version}. No exact matching Unity installation was found.", { version: options?.projectUnityVersion ?? project.unityVersion })}</p>
                <div className="dialog-actions">
                    <Button onClick={close} type="button" variant="tonal">{t("Cancel")}</Button>
                    <Button disabled={!canMigrate} onClick={() => { close(); navigate(`/projects/${projectId}/unity?afterMigration=open`); }} title={capabilityUnavailableTitle(canMigrate, capabilities.projectsUnityMigration, t)} type="button">{t("Migrate Project…")}</Button>
                </div>
            </Dialog>
            <Dialog dismissible={!busy} onClose={closed} open={dialog === "choose"} title={t("Choose Unity for this launch")}>
                <Select disabled={busy} label={t("Unity installation for this launch")} onChange={setInstallationId} options={[{ label: t("Choose an installation"), value: "" }, ...(options?.exactMatchingInstallations ?? []).map((item) => ({ label: `Unity ${item.unityVersion} · ${item.architecture} · ${item.executablePath}`, value: item.installationId }))]} value={installationId} />
                {error === undefined ? null : <p className="inline-error" role="alert">{t("Unable to open Unity: {code}", { code: error.code })}</p>}
                <div className="dialog-actions">
                    <Button disabled={busy} onClick={close} type="button" variant="tonal">{t("Cancel")}</Button>
                    <Button disabled={!canLaunch || installationId.length === 0 || busy} onClick={confirm} type="button">{t("Open Unity")}</Button>
                </div>
            </Dialog>
        </>
    };
}
