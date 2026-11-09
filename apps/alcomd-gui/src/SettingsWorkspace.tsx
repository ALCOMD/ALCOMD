import { requestErrorMessage } from "./request-errors";
import type { RpcError } from "@alcomd/sdk";
import { Suspense, use, useCallback, useEffect, useRef, useState, useSyncExternalStore, type FocusEventHandler, type ReactNode, type Ref } from "react";
import { StorageDirectoryConflictError, UnityHubConflictError, type SettingsAutosave } from "./settings-autosave";

import type { NetworkProxy, OfficialSettings, SettingsGetResult, SettingsLocale, UnityInstallation } from "./core-models";
import type { GuiRpcClient } from "./rpc";
import { normalizeRequestError } from "./request-errors";
import { useTranslation } from "./i18n";
import { UnityRegistryActions } from "./CoreActions";
import { capabilities, useCapability } from "./capabilities";
import { DesktopList, TableLoading } from "./DesktopList";
import { Button, Checkbox, PathField, SegmentedButtons, Select, TextField } from "./Material";
import { UtilityActionDialog } from "./UtilityActionDialog";
import { UtilityWorkspace } from "./UtilityWorkspace";
import { LoadingState } from "./LoadingState";
import { usePageReadPromise, type PageReadResult } from "./PageRead";
import { navigationLabels } from "./navigation-labels";
import { ThemeControls, themePreferences } from "./ThemeControls";
import { displayPath } from "./display-path";
import { UnityDefaultArgumentsSettings } from "./UnityDefaultArgumentsSettings";
import { backupCompressionOptions } from "./backup-compression";
import { SettingsResetBoundary, SettingsResetButton, isSettingsResetTrigger, useSettingsResetPending } from "./SettingsResetButton";
import { useCoreInvalidation } from "./core-events-ui";
import { invoke, isTauri } from "@tauri-apps/api/core";

interface SettingsWorkspaceProps {
    client: GuiRpcClient;
    settingsWriter: SettingsAutosave;
    onApplied(value: SettingsGetResult): void;
}

type SettingsCategory = "general" | "unity" | "appearance";

export function SettingsWorkspace({ client, settingsWriter, onApplied }: SettingsWorkspaceProps) {
    const { t } = useTranslation();
    const load = useCallback(() => client.settingsGet(), [client]);
    const canReadUnity = useCapability(capabilities.unityRead);
    const canManageProjects = useCapability(capabilities.projectsRegistry);
    const initialUnity = usePageReadPromise("settings.unity", () => canReadUnity ? client.unityInstallationsList() : Promise.resolve(undefined));
    const [category, setCategory] = useState<SettingsCategory>("general");
    const [unityGeneration, setUnityGeneration] = useState(0);
    const [projectReset, setProjectReset] = useState(0);
    const [backupReset, setBackupReset] = useState(0);
    const [projectDirty, setProjectDirty] = useState(false);
    const [backupDirty, setBackupDirty] = useState(false);
    const saving = useSyncExternalStore(settingsWriter.subscribe, settingsWriter.getSnapshot);
    return (
        <SettingsResetBoundary><UtilityWorkspace title={t(navigationLabels.settings)} load={load}
            tools={() => saving.failed ? <Button disabled={saving.saving} onClick={() => void settingsWriter.retry()} type="button" variant="text">{t("Retry saving settings")}</Button> : null} navigation={
            <nav className="resource-navigation" aria-label={t("Settings sections")}>
                <SegmentedButtons label={t("Settings sections")} value={category} onChange={(value) => setCategory(value as SettingsCategory)}
                    options={[
                        { value: "general", label: t("General") },
                        { value: "unity", label: "Unity" },
                        { value: "appearance", label: t("Appearance") }
                    ]} />
            </nav>
        }>
            {(snapshot) => {
                const settings = saving.settings ?? snapshot.settings;
                const accepted = saving.accepted ?? snapshot;
                return (
                <div className="settings-workspace">
                    {isTauri() ? <VccLinkRegistration hidden={category !== "general"}
                        enabled={settings.integrations.handleVccLinks}
                        disabled={saving.saving || saving.failed}
                        onChange={(handleVccLinks) => settingsWriter.change({ integrations: { handleVccLinks } })} /> : null}
                    <SettingsSection title={t("Projects")} hidden={category !== "general"} actions={<SettingsResetButton title={t("Projects")} disabled={!canManageProjects || saving.saving || saving.failed || (!projectDirty && settings.paths.projectDirectory === null && settings.projects.autoDiscover)} onRestore={async () => {
                        await settingsWriter.restoreDefaults({ paths: { projectDirectory: null }, projects: { autoDiscover: true } },
                            { paths: { projectDirectory: accepted.settings.paths.projectDirectory }, projects: { autoDiscover: accepted.settings.projects.autoDiscover } });
                        setProjectReset((value) => value + 1);
                    }} />}>
                        <StorageDirectoryRow key={projectReset} client={client} settingsWriter={settingsWriter} snapshot={snapshot} directoryKey="projectDirectory" label={t("Default project location")} onDirtyChange={setProjectDirty} />
                        <Checkbox checked={settings.projects.autoDiscover} disabled={!canManageProjects} label={t("Automatically add projects from the default location")}
                            onChange={(autoDiscover) => settingsWriter.change({ projects: { autoDiscover } })} />
                    </SettingsSection>
                    <SettingsSection title={t("Backups")} hidden={category !== "general"} actions={<SettingsResetButton title={t("Backups")} disabled={saving.saving || saving.failed || (!backupDirty && settings.paths.backupDirectory === null && settings.backups.compressionMode === "fast" && settings.backups.includeVpmPackages && !settings.backups.includeLibrary)} onRestore={async () => {
                        await settingsWriter.restoreDefaults({ paths: { backupDirectory: null }, backups: { compressionMode: "fast", includeVpmPackages: true, includeLibrary: false } },
                            { paths: { backupDirectory: accepted.settings.paths.backupDirectory }, backups: { ...accepted.settings.backups } });
                        setBackupReset((value) => value + 1);
                    }} />}>
                        <StorageDirectoryRow key={backupReset} client={client} settingsWriter={settingsWriter} snapshot={snapshot} directoryKey="backupDirectory" label={t("Default backup location")} onDirtyChange={setBackupDirty} />
                        <div className="settings-row">
                            <label htmlFor="settings-backup-compression">{t("Default compression")}</label>
                            <Select id="settings-backup-compression" label={t("Default compression")} value={settings.backups.compressionMode}
                                onChange={(compressionMode) => settingsWriter.change({ backups: { compressionMode: compressionMode as OfficialSettings["backups"]["compressionMode"] } })}
                                options={backupCompressionOptions(t)} />
                        </div>
                        <Checkbox checked={settings.backups.includeVpmPackages} label={t("Keep packages by default")}
                            onChange={(includeVpmPackages) => settingsWriter.change({ backups: { includeVpmPackages } })} />
                        <Checkbox checked={settings.backups.includeLibrary} label={t("Keep Library cache by default")}
                            onChange={(includeLibrary) => settingsWriter.change({ backups: { includeLibrary } })} />
                    </SettingsSection>
                    <div className="settings-unity-group" hidden={category !== "unity"}>
                        <UnityHubSettingsSection client={client} settingsWriter={settingsWriter} snapshot={snapshot} onInstallationsChanged={() => setUnityGeneration((value) => value + 1)} />
                        <Suspense fallback={<SettingsSection title={t("Unity installations")}><TableLoading label={t("Unity installations")} columns={[{ key: "version", header: t("Version"), width: "20%" }, { key: "path", header: t("Path"), className: "settings-path" }, { key: "source", header: t("Source"), width: 160 }]} /></SettingsSection>}>
                            <UnitySettingsSection client={client} initialRead={initialUnity} externalGeneration={unityGeneration} />
                        </Suspense>
                        <UnityDefaultArgumentsSettings settingsWriter={settingsWriter} snapshot={snapshot} />
                    </div>
                    <NetworkProxySettings hidden={category !== "general"} proxy={accepted.settings.network.proxy}
                        settingsWriter={settingsWriter} disabled={saving.saving || saving.failed} />
                    <SettingsEditor category={category} settingsWriter={settingsWriter} snapshot={snapshot} onApplied={onApplied} />
                </div>
                );
            }}
        </UtilityWorkspace></SettingsResetBoundary>
    );
}

function NetworkProxySettings({ hidden, proxy, settingsWriter, disabled }: {
    hidden: boolean; proxy: NetworkProxy; settingsWriter: SettingsAutosave; disabled: boolean;
}) {
    const { t } = useTranslation();
    const [draft, setDraft] = useState<NetworkProxy>();
    const [error, setError] = useState<RpcError>();
    const selected = draft ?? proxy;
    const save = async (next: NetworkProxy) => {
        setDraft(next);
        setError(undefined);
        try {
            await settingsWriter.saveNetworkProxy(next, proxy);
            setDraft(undefined);
        } catch (cause) {
            setError(normalizeRequestError(cause));
        }
    };
    const select = (mode: string) => {
        setError(undefined);
        if (mode === "custom") {
            setDraft({ mode, url: proxy.mode === "custom" ? proxy.url : "" });
        } else if (mode === "system" || mode === "direct") {
            void save({ mode });
        }
    };
    return <SettingsSection title={t("Network")} hidden={hidden}>
        <div className="settings-row">
            <label htmlFor="settings-network-proxy">{t("Global proxy")}</label>
            <Select id="settings-network-proxy" label={t("Global proxy")} value={selected.mode}
                disabled={disabled} onChange={select} options={[
                    { value: "system", label: t("Follow system proxy") },
                    { value: "direct", label: t("Do not use a proxy") },
                    { value: "custom", label: t("Custom proxy") },
                ]} />
        </div>
        {selected.mode === "custom" ? <form className="settings-row settings-proxy-row" onSubmit={(event) => {
            event.preventDefault();
            if (!disabled && selected.url.trim() !== "") void save({ mode: "custom", url: selected.url.trim() });
        }}>
            <TextField label={t("Proxy address")} value={selected.url} maxLength={2048}
                supportingText={t("Enter an HTTP or HTTPS proxy address, such as http://127.0.0.1:7890.")}
                disabled={disabled} onInput={(url) => { setDraft({ mode: "custom", url }); setError(undefined); }} />
            <Button type="submit" disabled={disabled || selected.url.trim() === "" ||
                (proxy.mode === "custom" && selected.url === proxy.url && error === undefined)}>{t("Save")}</Button>
        </form> : error === undefined ? null :
            <Button type="button" disabled={disabled} onClick={() => void save(selected)}>{t("Retry")}</Button>}
    </SettingsSection>;
}

function VccLinkRegistration({ hidden, enabled, disabled, onChange }: {
    hidden: boolean; enabled: boolean; disabled: boolean; onChange(enabled: boolean): void;
}) {
    const { t } = useTranslation();
    const [available, setAvailable] = useState(false);
    const [busy, setBusy] = useState(false);
    useEffect(() => {
        let active = true;
        void invoke<boolean>("gui_vcc_registration_available").then((value) => {
            if (active) setAvailable(value);
        }, () => {});
        return () => { active = false; };
    }, []);
    const register = async () => {
        setBusy(true);
        try {
            await invoke("gui_register_vcc_links");
        } catch {
            // This action has no result surface; keep the original control available for retry.
        } finally {
            setBusy(false);
        }
    };
    if (!available) return null;
    return <SettingsSection title={t("VCC links")} hidden={hidden}>
        <Checkbox checked={enabled} disabled={disabled || busy} label={t("Handle VCC links")}
            onChange={onChange} />
        {enabled ? <div className="settings-row">
            <Button disabled={disabled || busy} onClick={() => void register()} type="button">{t("Register ALCOMD for VCC links")}</Button>
        </div> : null}
    </SettingsSection>;
}

function SettingsSection({ title, children, actions, sectionRef, onBlur, hidden }: {
    title: string; children: ReactNode; actions?: ReactNode; sectionRef?: Ref<HTMLElement>; onBlur?: FocusEventHandler<HTMLElement>; hidden?: boolean;
}) {
    return (
        <section className="settings-section" aria-label={title} ref={sectionRef} onBlur={onBlur} hidden={hidden}>
            <header className="settings-section-heading"><h2 tabIndex={-1}>{title}</h2>{actions}</header>
            <div className="settings-section-content">{children}</div>
        </section>
    );
}

type UnityHubIntent = { path: string | null; previousPath: string | null; origin: "select" | "auto" | "edit" };
type UnityHubAction = { kind: "select" | "auto" | "retry" } | { kind: "edit"; trigger: "enter" | "blur" };

function StorageDirectoryRow({ client, settingsWriter, snapshot, directoryKey, label, onDirtyChange }: {
    client: GuiRpcClient; settingsWriter: SettingsAutosave; snapshot: SettingsGetResult;
    directoryKey: keyof OfficialSettings["paths"]; label: string; onDirtyChange(dirty: boolean): void;
}) {
    const { t } = useTranslation();
    const resetting = useSettingsResetPending();
    const state = useSyncExternalStore(settingsWriter.subscribe, settingsWriter.getSnapshot);
    const accepted = state.accepted ?? snapshot;
    const acceptedRef = useRef(accepted);
    acceptedRef.current = accepted;
    const [draft, setDraft] = useState(accepted.directories[directoryKey]);
    const edit = useRef({ dirty: false, previousPath: accepted.settings.paths[directoryKey], value: draft });
    const [busy, setBusy] = useState(false);
    const [invalid, setInvalid] = useState(false);
    const [pending, setPending] = useState<{ path: string; previousPath: string | null }>();
    const generation = useRef(0);
    const guard = useRef(false);
    const failedEdit = useRef<string | undefined>(undefined);
    const reset = (value: SettingsGetResult, preserveFailed = false) => {
        const desired = preserveFailed ? state.settings?.paths[directoryKey] : undefined;
        const path = desired ?? value.directories[directoryKey];
        edit.current = { dirty: desired !== undefined && desired !== value.settings.paths[directoryKey], value: path, previousPath: value.settings.paths[directoryKey] };
        setDraft(path);
    };
    useEffect(() => {
        reset(acceptedRef.current, state.failed);
        setBusy(false); setInvalid(false); setPending(undefined); failedEdit.current = undefined;
        return () => { generation.current += 1; guard.current = false; };
    }, [client, settingsWriter, directoryKey]);
    useEffect(() => {
        if (pending !== undefined && accepted.settings.paths[directoryKey] !== pending.previousPath) {
            reset(accepted); setPending(undefined); setInvalid(false); failedEdit.current = undefined;
        } else if (!edit.current.dirty) reset(accepted, state.failed);
    }, [accepted, state.failed, state.settings]);
    useEffect(() => { onDirtyChange(edit.current.dirty); }, [draft, accepted, state.failed, state.settings, onDirtyChange]);
    const save = async (action: "select" | "edit" | "retry", blur = false) => {
        if (guard.current || (state.saving && action !== "edit")) return;
        if (action === "edit" && (!edit.current.dirty || blur && edit.current.value === failedEdit.current)) return;
        let intent = action === "retry" ? pending : action === "edit" ? { path: edit.current.value, previousPath: edit.current.previousPath } : undefined;
        if (intent?.path.length === 0) { setInvalid(true); return; }
        guard.current = true;
        const request = generation.current;
        setBusy(true);
        try {
            if (action === "select") {
                const path = await client.selectDirectory(edit.current.value || acceptedRef.current.directories[directoryKey]);
                if (request !== generation.current || path === undefined) return;
                intent = { path, previousPath: acceptedRef.current.settings.paths[directoryKey] };
            }
            if (intent === undefined) return;
            setPending(intent);
            const result = await settingsWriter.saveDirectory(directoryKey, intent.path, intent.previousPath);
            if (request !== generation.current) return;
            reset(result); setInvalid(false); setPending(undefined); failedEdit.current = undefined;
        } catch (caught: unknown) {
            if (request !== generation.current) return;
            if (action === "edit") failedEdit.current = intent?.path;
            if (caught instanceof StorageDirectoryConflictError) {
                reset(acceptedRef.current);
                setPending(undefined);
                setInvalid(false);
            } else if (intent !== undefined) {
                setInvalid(true);
            }
        } finally {
            if (request === generation.current) { guard.current = false; setBusy(false); }
        }
    };
    return <div className="settings-directory" onBlur={(event) => {
        if (resetting?.current || isSettingsResetTrigger(event.relatedTarget)) return;
        let target: EventTarget | null = event.relatedTarget;
        while (target instanceof Node) {
            if (event.currentTarget.contains(target)) return;
            const root = target.getRootNode();
            target = root instanceof ShadowRoot ? root.host : null;
        }
        void save("edit", true);
    }}>
        <div className="settings-path-row">
            <PathField id={`settings-${directoryKey}`} label={label} error={invalid} readOnly={busy} value={draft}
                onInput={(value) => {
                    if (!edit.current.dirty) edit.current.previousPath = acceptedRef.current.settings.paths[directoryKey];
                    const acceptedPath = acceptedRef.current.directories[directoryKey];
                    if (value === displayPath(acceptedPath)) value = acceptedPath;
                    edit.current.value = value;
                    edit.current.dirty = value !== acceptedPath;
                    setDraft(value); setInvalid(false); failedEdit.current = undefined;
                }} onKeyDown={(event) => {
                    if (event.key === "Escape") {
                        if (state.failed) settingsWriter.change({ paths: { [directoryKey]: acceptedRef.current.settings.paths[directoryKey] } });
                        event.preventDefault(); event.stopPropagation(); reset(acceptedRef.current);
                        setInvalid(false); setPending(undefined); failedEdit.current = undefined;
                    } else if (event.key === "Enter" && !event.nativeEvent.isComposing && event.keyCode !== 229) {
                        event.preventDefault(); void save("edit");
                    }
                }} />
            <Button softDisabled={busy || state.saving} onClick={() => void save(invalid && pending !== undefined ? "retry" : "select")} type="button">{t(invalid && pending !== undefined ? "Retry" : "Select")}</Button>
        </div>
    </div>;
}

function UnityHubSettingsSection({ client, settingsWriter, snapshot, onInstallationsChanged }: {
    client: GuiRpcClient; settingsWriter: SettingsAutosave; snapshot: SettingsGetResult; onInstallationsChanged(): void;
}) {
    const { t } = useTranslation();
    const resetting = useSettingsResetPending();
    const state = useSyncExternalStore(settingsWriter.subscribe, settingsWriter.getSnapshot);
    const accepted = state.accepted ?? snapshot;
    const acceptedRef = useRef(accepted);
    acceptedRef.current = accepted;
    const manageUnity = useCapability(capabilities.unityManage);
    const canOpenHub = useCapability(capabilities.unityHubOpen);
    const [busy, setBusy] = useState(false);
    const [invalid, setInvalid] = useState(false);
    const [pending, setPending] = useState<UnityHubIntent>();
    const [draft, setDraft] = useState(accepted.unityHub.path ?? accepted.settings.unity.hubPath ?? "");
    const edit = useRef({ value: draft, display: draft, previousPath: accepted.settings.unity.hubPath, dirty: false });
    const failedEdit = useRef<string | undefined>(undefined);
    const group = useRef<HTMLElement>(null);
    const guard = useRef(false);
    const active = useRef(true);
    const generation = useRef(0);
    const resetDraft = (value: SettingsGetResult) => {
        const path = value.unityHub.path ?? value.settings.unity.hubPath ?? "";
        edit.current = { value: path, display: displayPath(path), previousPath: value.settings.unity.hubPath, dirty: false };
        setDraft(path);
    };
    useEffect(() => {
        active.current = true;
        setBusy(false);
        setInvalid(false);
        setPending(undefined);
        failedEdit.current = undefined;
        resetDraft(acceptedRef.current);
        return () => { active.current = false; generation.current += 1; guard.current = false; };
    }, [client, settingsWriter]);
    useEffect(() => {
        if (!edit.current.dirty) resetDraft(accepted);
    }, [accepted]);

    const run = async (action: UnityHubAction) => {
        if (!active.current || guard.current || state.failed || (state.saving && action.kind !== "edit")) return;
        if (action.kind === "edit" && !edit.current.dirty) return;
        if (action.kind === "edit" && action.trigger === "blur" && edit.current.value === failedEdit.current) return;
        let intent = action.kind === "retry" ? pending : undefined;
        if (action.kind === "edit" || (intent?.origin === "edit" && action.kind === "retry")) {
            if (!edit.current.dirty) { setInvalid(false); setPending(undefined); return; }
            if (edit.current.value.length === 0) {
                setPending(undefined);
                setInvalid(true);
                return;
            }
            intent = { path: edit.current.value, previousPath: edit.current.previousPath, origin: "edit" };
        }
        guard.current = true;
        const request = generation.current;
        let restoreDraftFocus = false;
        setBusy(true);
        try {
            if (action.kind === "auto" || action.kind === "select") {
                const path = action.kind === "auto" ? null : await client.selectUnityHub(edit.current.value || acceptedRef.current.unityHub.path);
                if (request !== generation.current) return;
                if (path === undefined) { restoreDraftFocus = edit.current.dirty; return; }
                intent = { path, previousPath: acceptedRef.current.settings.unity.hubPath, origin: action.kind };
            }
            if (intent === undefined) return;
            setPending(intent);
            const result = await settingsWriter.saveUnityHub(intent.path,
                action.kind === "retry" || intent.origin === "edit" ? { previousPath: intent.previousPath } : undefined);
            if (request !== generation.current) return;
            setPending(undefined);
            failedEdit.current = undefined;
            resetDraft(result);
            setInvalid(false);
            if (manageUnity && (intent.path !== null || intent.previousPath !== null)) {
                try {
                    await client.unityInstallationsRefresh();
                    if (request === generation.current) onInstallationsChanged();
                } catch {}
            }
        } catch (cause) {
            if (request !== generation.current) return;
            if (intent?.origin === "edit") failedEdit.current = intent.path ?? undefined;
            if (cause instanceof UnityHubConflictError) {
                resetDraft(acceptedRef.current);
                setPending(undefined);
                setInvalid(false);
            } else if (intent !== undefined) {
                setInvalid(true);
            }
        } finally {
            if (request === generation.current) {
                guard.current = false;
                setBusy(false);
                if (restoreDraftFocus) requestAnimationFrame(() => {
                    if (request === generation.current) group.current?.querySelector<HTMLElement>("#settings-unity-hub-path")?.focus();
                });
            }
        }
    };
    const location = accepted.unityHub;
    const pathLabel = location.status === "unavailable" ? t("Unity Hub location is unavailable.") : t("Unity Hub was not found.");
    const download = accepted.settings.unity.hubPath === null && location.status === "not_found";
    const usable = location.status === "available" && location.path !== null;
    const openLabel = download ? t("Download Unity Hub") : t("Open Unity Hub");
    const openDisabledReason = !canOpenHub ? t("Opening Unity Hub is unavailable on this connection.")
        : !usable && !download ? t("Choose a usable Unity Hub path or use Auto detect before opening it.")
            : edit.current.dirty ? t("Finish or cancel the path edit before continuing.")
                : pending !== undefined || state.failed ? t("Resolve the settings save error before continuing.")
                    : busy || state.saving ? t("Another settings action is in progress.") : undefined;
    const openHub = async () => {
        if (!active.current || guard.current || !canOpenHub || (!usable && !download) || edit.current.dirty || pending !== undefined || state.saving || state.failed) return;
        guard.current = true;
        const request = generation.current;
        setBusy(true);
        try {
            const current = acceptedRef.current;
            const result = await client.unityHubOpen(current.revision, current.unityHub.path);
            if (request !== generation.current) return;
            if (result.outcome === "download_required") await client.openUnityHubDownload();
        } catch (cause) {
            if (request !== generation.current) return;
            const { code } = normalizeRequestError(cause);
            if (["revision_conflict", "hub_path_changed", "hub_path_invalid", "hub_unavailable"].includes(code)) {
                await settingsWriter.reload().catch(() => {});
            }
        } finally {
            if (request === generation.current) { guard.current = false; setBusy(false); }
        }
    };
    return (
        <SettingsSection title={t("Unity Hub Path")} sectionRef={group}
            actions={<div className="action-row">
                <Button disabled={state.failed} softDisabled={busy || state.saving} onClick={() => void run({ kind: invalid && pending?.origin === "auto" ? "retry" : "auto" })} type="button" variant="tonal">{t(invalid && pending?.origin === "auto" ? "Retry" : "Auto detect")}</Button>
                <Button disabled={!canOpenHub || (!usable && !download)} softDisabled={busy || state.saving || state.failed || edit.current.dirty || pending !== undefined}
                    aria-describedby={openDisabledReason === undefined ? undefined : "settings-unity-hub-open-reason"} title={openDisabledReason}
                    widthLabels={[t("Open Unity Hub"), t("Download Unity Hub")]} onClick={() => void openHub()} type="button">{openLabel}</Button>
            </div>}
            onBlur={(event) => {
                if (resetting?.current || isSettingsResetTrigger(event.relatedTarget)) return;
                let target: EventTarget | null = event.relatedTarget;
                while (target instanceof Node) {
                    if (event.currentTarget.contains(target)) return;
                    const root = target.getRootNode();
                    target = root instanceof ShadowRoot ? root.host : null;
                }
                void run({ kind: "edit", trigger: "blur" });
            }}>
            <div className="settings-path-row">
                <PathField id="settings-unity-hub-path" label={t("Unity Hub Path")} error={invalid || (!edit.current.dirty && location.configuredPathStatus === "invalid")} readOnly={busy || state.failed}
                    placeholder={location.path === null && accepted.settings.unity.hubPath === null ? pathLabel : undefined} value={draft}
                    onFocus={() => { if (!edit.current.dirty) resetDraft(acceptedRef.current); }}
                    onInput={(value) => {
                        const current = acceptedRef.current;
                        if (value === edit.current.display || value === (current.unityHub.path ?? current.settings.unity.hubPath ?? "")) {
                            resetDraft(current);
                            if (pending?.origin === "edit") {
                                failedEdit.current = undefined;
                                setInvalid(false); setPending(undefined);
                            }
                            return;
                        }
                        failedEdit.current = undefined;
                        setInvalid(false);
                        setPending(undefined);
                        edit.current.value = value;
                        edit.current.dirty = value !== edit.current.display;
                        setDraft(value);
                    }}
                    onKeyDown={(event) => {
                        if (event.key === "Escape") {
                            event.preventDefault(); event.stopPropagation();
                            resetDraft(acceptedRef.current); failedEdit.current = undefined; setInvalid(false); setPending(undefined);
                        } else if (event.key === "Enter" && !event.nativeEvent.isComposing && event.keyCode !== 229) {
                            event.preventDefault();
                            void run({ kind: "edit", trigger: "enter" });
                        }
                    }} />
                <Button disabled={state.failed} softDisabled={busy || state.saving} onClick={() => void run({ kind: invalid && pending !== undefined && pending.origin !== "auto" ? "retry" : "select" })} type="button">{t(invalid && pending !== undefined && pending.origin !== "auto" ? "Retry" : "Select")}</Button>
            </div>
            {openDisabledReason === undefined ? null : <span className="visually-hidden" id="settings-unity-hub-open-reason">{openDisabledReason}</span>}
        </SettingsSection>
    );
}

function UnitySettingsSection({ client, initialRead, externalGeneration }: { client: GuiRpcClient; externalGeneration: number; initialRead: Promise<PageReadResult<Awaited<ReturnType<GuiRpcClient["unityInstallationsList"]>> | undefined>> }) {
    const { t } = useTranslation();
    const available = useCapability(capabilities.unityRead);
    const manage = useCapability(capabilities.unityManage);
    const initial = use(initialRead);
    const [loaded, setLoaded] = useState(initial.kind === "ready");
    const [items, setItems] = useState<UnityInstallation[]>(initial.kind === "ready" ? initial.value?.installations ?? [] : []);
    const [cursor, setCursor] = useState<string | undefined>(initial.kind === "ready" ? initial.value?.nextCursor : undefined);
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState<RpcError | undefined>(initial.kind === "error" ? normalizeRequestError(initial.error) : undefined);
    const [open, setOpen] = useState(false);
    const [generation, setGeneration] = useState(0);
    const requestRef = useRef(0);
    const busyRef = useRef(false);
    const readAgain = useRef(false);
    const readFailure = useRef<unknown>(undefined);

    const read = useCallback(async (next?: string) => {
        if (busyRef.current) { readAgain.current = true; return; }
        const request = ++requestRef.current;
        busyRef.current = true;
        setBusy(true);
        setError(undefined);
        readFailure.current = undefined;
        try {
            const page = await client.unityInstallationsList(next);
            if (request !== requestRef.current) return;
            setLoaded(true);
            setItems((current) => next === undefined ? page.installations : [...current, ...page.installations]);
            setCursor(page.nextCursor);
        } catch (caught: unknown) {
            readFailure.current = caught;
            if (request === requestRef.current) setError(normalizeRequestError(caught));
        } finally {
            if (request === requestRef.current) { busyRef.current = false; setBusy(false);
                if (readAgain.current) { readAgain.current = false; void read(); }
            }
        }
    }, [client]);

    const retryRead = useCoreInvalidation("settings:unity", ["unity"], async () => { await read(); if (readFailure.current !== undefined) throw readFailure.current; });

    useEffect(() => {
        if (available && (generation > 0 || externalGeneration > 0)) void read();
        return () => { requestRef.current += 1; busyRef.current = false; };
    }, [available, read, generation, externalGeneration]);

    return (
        <SettingsSection title={t("Unity installations")} actions={
            <div className="action-row">
                <Button disabled={!manage} onClick={() => setOpen(true)} type="button">{t("Manage installations")}</Button>
            </div>
        }>
            {!available ? <p className="field-hint">{t("Unity installations are unavailable on this connection.")}</p> : (
                <>
                    {error === undefined || loaded ? null : <div role="alert"><strong>{t("Unity installations could not be loaded.")}</strong><p>{t(requestErrorMessage((error).code))}</p><code>{(error).code}</code>{error.diagnosticId === undefined ? null : <p><code>{error.diagnosticId}</code></p>}
                        <Button disabled={busy} onClick={retryRead} type="button" variant="text">{t("Retry")}</Button>
                    </div>}
                    <DesktopList columns={[{ key: "version", header: t("Version"), width: "20%" }, { key: "path", header: t("Path"), className: "settings-path" }, { key: "source", header: t("Source"), width: 160 }]} label={t("Unity installations")} minWidth={600} rows={items.map((item) => ({ key: item.installationId, cells: [item.unityVersion, <span title={displayPath(item.executablePath)}>{displayPath(item.executablePath)}</span>, item.sourceKind === "manual" ? t("Manual") : item.sourceKind === "unity_hub" ? t("Unity Hub") : item.sourceKind.replaceAll("_", " ")] }))} />
                    {items.length === 0 ? loaded ? <p role="status">{t("No Unity installations registered")}</p> : busy ? <LoadingState title={t("Loading Unity installations…")} /> : null : null}
                    {cursor === undefined ? null : <Button disabled={busy} onClick={() => void read(cursor)} type="button" variant="text">{t("Load more installations")}</Button>}
                </>
            )}
            {open ? <UtilityActionDialog title={t("Manage Unity installations")} onClose={() => setOpen(false)}>
                <UnityRegistryActions client={client} installations={items} onChanged={() => setGeneration((current) => current + 1)} />
            </UtilityActionDialog> : null}
        </SettingsSection>
    );
}

function SettingsEditor({ category, settingsWriter, snapshot, onApplied }: Omit<SettingsWorkspaceProps, "client"> & { category: SettingsCategory; snapshot: SettingsGetResult }) {
    const { t } = useTranslation();
    const saving = useSyncExternalStore(settingsWriter.subscribe, settingsWriter.getSnapshot);
    const baseline = saving.accepted ?? snapshot;
    const settings = saving.settings ?? snapshot.settings;
    useEffect(() => {
        settingsWriter.seed(snapshot);
        onApplied(snapshot);
    }, [settingsWriter, snapshot, onApplied]);


    return (
        <div className="settings-editor" hidden={category !== "appearance"}>
            <SettingsSection title={t("Appearance")} hidden={category !== "appearance"} actions={<SettingsResetButton title={t("Appearance")} disabled={saving.saving || saving.failed || (settings.locale === "system" && settings.appearance.motion === "system")} onRestore={() => settingsWriter.restoreDefaults(
                { locale: "system", appearance: { motion: "system" } }, { locale: baseline.settings.locale, appearance: { motion: baseline.settings.appearance.motion } })} />}>
                <div className="settings-row">
                    <label htmlFor="settings-locale">{t("Language")}</label>
                    <Select id="settings-locale" label={t("Language")} onChange={(value) => settingsWriter.change({ locale: value as SettingsLocale })}
                        options={[{ label: t("System"), value: "system" }, { label: "English", value: "en-US" }, { label: "简体中文", value: "zh-CN" }, { label: "日本語", value: "ja-JP" }]} value={settings.locale} />
                </div>
                <div className="settings-row">
                    <label htmlFor="settings-motion">{t("GUI animation")}</label>
                    <Select id="settings-motion" label={t("GUI animation")} value={settings.appearance.motion}
                        onChange={(value) => settingsWriter.change({ appearance: { motion: value as OfficialSettings["appearance"]["motion"] } })}
                        options={[{ label: t("System"), value: "system" }, { label: t("Enable animations"), value: "full" }, { label: t("Disable animations"), value: "reduced" }]} />
                </div>
            </SettingsSection>
            <SettingsSection title={t("Window")} hidden={category !== "appearance"} actions={<SettingsResetButton title={t("Window")} disabled={saving.saving || saving.failed || (!settings.window.rememberState && !settings.window.fullscreen)} onRestore={() => settingsWriter.restoreDefaults(
                { window: { rememberState: false, fullscreen: false } }, { window: { ...baseline.settings.window } })} />}>
                <Checkbox checked={settings.window.fullscreen} label={t("Full screen mode (F11)")}
                    onChange={(fullscreen) => settingsWriter.change({ window: { fullscreen } })} />
                <Checkbox checked={settings.window.rememberState} label={t("Remember window size and maximized state")}
                    onChange={(rememberState) => settingsWriter.change({ window: { rememberState } })} />
            </SettingsSection>
            <SettingsSection title={t("Theme")} hidden={category !== "appearance"} actions={<SettingsResetButton title={t("Theme")} disabled={saving.saving || saving.failed || (settings.appearance.mode === "system" && settings.appearance.sourceColor === null && settings.appearance.scheme === "vibrant")} onRestore={() => settingsWriter.restoreDefaults({ appearance: { mode: "system", sourceColor: null, scheme: "vibrant" } }, { appearance: themePreferences(baseline) })} />}>
                <ThemeControls value={settings.appearance} onChange={(appearance) => settingsWriter.change({ appearance })} />
            </SettingsSection>
        </div>
    );
}
