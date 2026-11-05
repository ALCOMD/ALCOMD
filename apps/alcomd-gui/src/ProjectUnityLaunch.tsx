import { requestErrorMessage } from "./request-errors";
import { useTranslation } from "./i18n";
import type { RpcError } from "@alcomd/sdk";
import { flipToFrontIcon, playArrowIcon, taskCenterIcon } from "@alcomd/ui/icons";
import { useEffect, useRef, useState, type ReactNode } from "react";

import { Button, ButtonProgress, Dialog, Icon, Select } from "./Material";
import { capabilities, capabilityUnavailableTitle, useCapability } from "./capabilities";
import type { ProjectSnapshot, UnityLaunchOptionsResult } from "./core-models";
import { normalizeRpcError, type GuiRpcClient } from "./rpc";
import { displayPath } from "./display-path";
import { useUnityWindowState } from "./unity-window-observation";

export interface ProjectUnityLaunchAction {
    disabled: boolean;
    icon: ReactNode;
    label: string;
    onClick(): void;
    title?: string;
    widthLabels?: readonly string[];
    "aria-busy"?: boolean;
}

export function useUnityWindowAction(client: GuiRpcClient, project: ProjectSnapshot, launch: () => void, launchDisabled: boolean, busy: boolean, title?: string) {
    const { t } = useTranslation();
    const canReadProject = useCapability(capabilities.projectsRead);
    const canReadUnity = useCapability(capabilities.unityRead);
    const native = canReadProject && canReadUnity ? client.unityWindows : undefined;
    const { projectId, revision } = project;
    const state = useUnityWindowState(native, projectId, revision);
    const [failure, setFailure] = useState<string>();
    const pending = useRef<object | undefined>(undefined);
    const origin = useRef<HTMLElement | null>(null);
    useEffect(() => {
        setFailure(undefined);
        return () => { pending.current = undefined; };
    }, [native, projectId, revision]);

    const focus = async () => {
        if (native === undefined || projectId === undefined || revision === undefined || pending.current !== undefined) return;
        if (document.activeElement instanceof HTMLElement) origin.current = document.activeElement;
        const request = {};
        pending.current = request;
        try {
            await native.focus(projectId, revision);
        } catch (error: unknown) {
            if (pending.current === request) setFailure(normalizeRpcError(error).code);
        } finally {
            if (pending.current === request) pending.current = undefined;
        }
    };
    const open = t("Open Unity");
    const opening = t("Opening…");
    const alreadyOpen = t("Unity is open");
    const front = t("Bring to front");
    const windowIsActive = state === "opening" || state === "open" || state === "ready";
    const action: ProjectUnityLaunchAction = {
        disabled: busy || state === "opening" || state === "open" || (state !== "ready" && launchDisabled),
        icon: state === "opening" ? <ButtonProgress slot="icon" /> :
            <Icon asset={state === "ready" ? flipToFrontIcon : state === "open" ? taskCenterIcon : playArrowIcon} slot="icon" />,
        label: state === "opening" ? opening : state === "open" ? alreadyOpen : state === "ready" ? front : open,
        onClick: state === "ready" ? () => void focus() : launch,
        title: windowIsActive ? undefined : title,
        widthLabels: native === undefined ? undefined : [open, opening, alreadyOpen, front],
        "aria-busy": state === "opening" || undefined
    };
    const close = () => {
        setFailure(undefined);
        window.requestAnimationFrame(() => { if (origin.current?.isConnected) origin.current.focus(); });
    };
    const feedback = failure === undefined ? null : <Dialog open onClose={close} title={t("Could not bring Unity to the front")}>
        <p>{failure === "unity_window_not_found" ? t("The Unity window for this project is no longer available.")
            : failure === "unity_window_focus_rejected" ? t("Windows did not allow the Unity window to move to the front.")
                : t(requestErrorMessage(failure))}</p>
        <div className="dialog-actions"><Button onClick={close} type="button">{t("Close")}</Button></div>
    </Dialog>;
    return { action, feedback, windowIsActive };
}

export function useProjectUnityLaunch({ client, initialOptions, navigate, onLaunchSettled, project }: {
    client: GuiRpcClient;
    initialOptions?: UnityLaunchOptionsResult;
    navigate(path: string): void;
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
    }, [client, projectId, revision]);
    const run = async (task: (current: number) => Promise<void>) => {
        if (!canLaunch || projectId === undefined || revision === undefined || pending.current) return;
        const current = generation.current;
        pending.current = true; setBusy(true); setError(undefined);
        try { await task(current); }
        catch (caught: unknown) {
            if (current === generation.current && dialog === "choose") setError(normalizeRpcError(caught));
        } finally {
            if (current === generation.current) { pending.current = false; setBusy(false); }
        }
    };
    const launch = async (snapshot: UnityLaunchOptionsResult, selected: string, current: number) => {
        if (projectId === undefined) return;
        try {
            await client.unityLaunch(projectId, selected, snapshot.projectRevision);
            if (current !== generation.current) return;
            setDialog(undefined); setInstallationId("");
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
        void run((current) => launch(options, installationId, current));
    };
    const close = () => { if (!pending.current) { setDialog(undefined); setInstallationId(""); setError(undefined); } };
    const closed = (kind: NonNullable<typeof dialog>) => {
        if (dialog !== undefined && dialog !== kind) return;
        close();
        window.requestAnimationFrame(() => { if (trigger.current?.isConnected) trigger.current.focus(); });
    };
    const desktop = useUnityWindowAction(client, project, open,
        !canLaunch || projectId === undefined || revision === undefined,
        busy, capabilityUnavailableTitle(canLaunch, capabilities.unityLaunch, t));

    return {
        busy,
        action: desktop.action,
        dialogs: <>
            {desktop.feedback}
            <Dialog onClose={() => closed("missing")} open={dialog === "missing"} title={t("Unity installation required")}>
                <p>{t("This project requires Unity {version}. No exact matching Unity installation was found.", { version: options?.projectUnityVersion ?? project.unityVersion })}</p>
                <div className="dialog-actions">
                    <Button onClick={close} type="button" variant="tonal">{t("Cancel")}</Button>
                    <Button disabled={!canMigrate} onClick={() => { close(); navigate(`/projects/${projectId}/unity?afterMigration=open`); }} title={capabilityUnavailableTitle(canMigrate, capabilities.projectsUnityMigration, t)} type="button">{t("Migrate Project…")}</Button>
                </div>
            </Dialog>
            <Dialog dismissible={!busy} onClose={() => closed("choose")} open={dialog === "choose"} title={t("Choose Unity for this launch")}>
                <Select disabled={busy} label={t("Unity installation for this launch")} onChange={(value) => { setInstallationId(value); setError(undefined); }} options={[{ label: t("Choose an installation"), value: "" }, ...(options?.exactMatchingInstallations ?? []).map((item) => ({ label: `Unity ${item.unityVersion} · ${item.architecture} · ${displayPath(item.executablePath)}`, value: item.installationId }))]} value={installationId} />
                {error === undefined ? null : <div role="alert"><p>{t(requestErrorMessage((error).code))}</p><code>{(error).code}</code>{error.diagnosticId === undefined ? null : <p><code>{error.diagnosticId}</code></p>}</div>}
                <div className="dialog-actions">
                    <Button disabled={busy} onClick={close} type="button" variant="tonal">{t("Cancel")}</Button>
                    <Button disabled={!canLaunch || installationId.length === 0 || busy || desktop.windowIsActive} onClick={confirm} type="button">{t("Open Unity")}</Button>
                </div>
            </Dialog>
        </>
    };
}
