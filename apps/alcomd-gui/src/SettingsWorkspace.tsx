import type { ThemePreferences } from "@alcomd/ui";
import type { RpcError } from "@alcomd/sdk";
import { use, useCallback, useEffect, useRef, useState, useSyncExternalStore, type FocusEventHandler, type ReactNode, type Ref } from "react";
import { StorageDirectoryConflictError, UnityHubConflictError, type SettingsAutosave } from "./settings-autosave";

import type { OfficialSettings, RepositorySnapshot, SettingsGetResult, SettingsLocale, UnityInstallation } from "./core-models";
import type { GuiRpcClient } from "./rpc";
import { normalizeRequestError } from "./request-errors";
import { useTranslation } from "./i18n";
import { UnityRegistryActions } from "./CoreActions";
import { capabilities, useCapability } from "./capabilities";
import { DesktopList } from "./DesktopList";
import { Button, Checkbox, RefreshButton, SegmentedButtons, Select, TextField } from "./Material";
import { UtilityActionDialog } from "./UtilityActionDialog";
import { UtilityWorkspace } from "./UtilityWorkspace";
import { LoadingState } from "./LoadingState";
import { usePageReadPromise, type PageReadResult } from "./PageRead";
import { navigationLabels } from "./navigation-labels";
import { ThemeControls, themePreferences } from "./ThemeControls";
import { GlobalReminder, Reminder, RequestReminder } from "./Reminder";
import { displayPath } from "./display-path";
import { UnityDefaultArgumentsSettings } from "./UnityDefaultArgumentsSettings";

interface SettingsWorkspaceProps {
    client: GuiRpcClient;
    settingsWriter: SettingsAutosave;
    onApplied(value: SettingsGetResult): void;
    onThemePreview(theme: ThemePreferences | undefined): void;
    onThemeSession(cancel: (() => void) | undefined): void;
    onDirtyChange(dirty: boolean): void;
}

type SettingsCategory = "general" | "unity" | "appearance" | "notifications";

export function SettingsWorkspace({ client, settingsWriter, onApplied, onThemePreview, onThemeSession, onDirtyChange }: SettingsWorkspaceProps) {
    const { t } = useTranslation();
    const load = useCallback(() => client.settingsGet(), [client]);
    const canReadUnity = useCapability(capabilities.unityRead);
    const canReadRepositories = useCapability(capabilities.repositoriesRead);
    const initialUnity = usePageReadPromise("settings.unity", () => canReadUnity ? client.unityInstallationsList() : Promise.resolve(undefined));
    const initialRepositories = usePageReadPromise("settings.repositories", () => canReadRepositories ? client.repositoriesList() : Promise.resolve(undefined));
    const [dirty, setDirty] = useState(false);
    const [category, setCategory] = useState<SettingsCategory>("general");
    const [unityGeneration, setUnityGeneration] = useState(0);
    const saving = useSyncExternalStore(settingsWriter.subscribe, settingsWriter.getSnapshot);
    const dirtyChanged = useCallback((value: boolean) => { setDirty(value); onDirtyChange(value); }, [onDirtyChange]);
    return (
        <UtilityWorkspace title={t(navigationLabels.settings)} load={load} refreshDisabled={dirty || saving.saving}
            tools={() => saving.failed ? <Button disabled={saving.saving} onClick={() => void settingsWriter.retry()} type="button" variant="text">{t("Retry saving settings")}</Button> : null} navigation={
            <nav className="resource-navigation" aria-label={t("Settings sections")}>
                <SegmentedButtons label={t("Settings sections")} value={category} onChange={(value) => setCategory(value as SettingsCategory)}
                    options={[
                        { value: "general", label: t("General") },
                        { value: "unity", label: "Unity" },
                        { value: "appearance", label: t("Appearance") },
                        { value: "notifications", label: t("Notifications") }
                    ]} />
            </nav>
        }>
            {(snapshot) => (
                <div className="settings-workspace">
                    <SettingsSection title={t("Save locations")} hidden={category !== "general"}>
                        <StorageDirectoryRow client={client} settingsWriter={settingsWriter} snapshot={snapshot} directoryKey="projectDirectory" label={t("Default project location")} />
                        <StorageDirectoryRow client={client} settingsWriter={settingsWriter} snapshot={snapshot} directoryKey="backupDirectory" label={t("Default backup location")} />
                    </SettingsSection>
                    <div className="settings-unity-group" hidden={category !== "unity"}>
                        <UnityHubSettingsSection client={client} settingsWriter={settingsWriter} snapshot={snapshot} onInstallationsChanged={() => setUnityGeneration((value) => value + 1)} />
                        <UnitySettingsSection client={client} initialRead={initialUnity} externalGeneration={unityGeneration} />
                        <UnityDefaultArgumentsSettings settingsWriter={settingsWriter} snapshot={snapshot} />
                    </div>
                    <SettingsEditor category={category} client={client} settingsWriter={settingsWriter} snapshot={snapshot} onThemePreview={onThemePreview} onThemeSession={onThemeSession}
                        initialRepositories={initialRepositories} onApplied={onApplied} onDirtyChange={dirtyChanged} />
                </div>
            )}
        </UtilityWorkspace>
    );
}

function SettingsSection({ title, children, actions, sectionRef, onBlur, hidden }: {
    title: string; children: ReactNode; actions?: ReactNode; sectionRef?: Ref<HTMLElement>; onBlur?: FocusEventHandler<HTMLElement>; hidden?: boolean;
}) {
    return (
        <section className="settings-section" aria-label={title} ref={sectionRef} onBlur={onBlur} hidden={hidden}>
            <header className="settings-section-heading"><h2>{title}</h2>{actions}</header>
            <div className="settings-section-content">{children}</div>
        </section>
    );
}

type UnityHubIntent = { path: string | null; previousPath: string | null; origin: "select" | "auto" | "edit" };
type UnityHubAction = { kind: "select" | "auto" | "retry" } | { kind: "edit"; trigger: "enter" | "blur" };
type SettingsFeedback = { message: string; cause?: RpcError };

function StorageDirectoryRow({ client, settingsWriter, snapshot, directoryKey, label }: {
    client: GuiRpcClient; settingsWriter: SettingsAutosave; snapshot: SettingsGetResult;
    directoryKey: keyof OfficialSettings["paths"]; label: string;
}) {
    const { t } = useTranslation();
    const state = useSyncExternalStore(settingsWriter.subscribe, settingsWriter.getSnapshot);
    const accepted = state.accepted ?? snapshot;
    const acceptedRef = useRef(accepted);
    acceptedRef.current = accepted;
    const [draft, setDraft] = useState(accepted.directories[directoryKey]);
    const edit = useRef({ dirty: false, previousPath: accepted.settings.paths[directoryKey], value: draft });
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState<SettingsFeedback>();
    const [pending, setPending] = useState<{ path: string; previousPath: string | null }>();
    const generation = useRef(0);
    const guard = useRef(false);
    const failedEdit = useRef<string | undefined>(undefined);
    const feedbackRef = useRef<HTMLDivElement>(null);
    const reset = (value: SettingsGetResult, preserveFailed = false) => {
        const desired = preserveFailed ? state.settings?.paths[directoryKey] : undefined;
        const path = desired ?? value.directories[directoryKey];
        edit.current = { dirty: desired !== undefined && desired !== value.settings.paths[directoryKey], value: path, previousPath: value.settings.paths[directoryKey] };
        setDraft(path);
    };
    useEffect(() => {
        reset(acceptedRef.current, state.failed);
        setBusy(false); setError(undefined); setPending(undefined); failedEdit.current = undefined;
        return () => { generation.current += 1; guard.current = false; };
    }, [client, settingsWriter, directoryKey]);
    useEffect(() => {
        if (pending !== undefined && accepted.settings.paths[directoryKey] !== pending.previousPath) {
            reset(accepted); setPending(undefined); setError(undefined); failedEdit.current = undefined;
        } else if (!edit.current.dirty) reset(accepted, state.failed);
    }, [accepted, state.failed, state.settings]);
    const save = async (action: "select" | "edit" | "retry", blur = false) => {
        if (guard.current || (state.saving && action !== "edit")) return;
        if (action === "edit" && (!edit.current.dirty || blur && edit.current.value === failedEdit.current)) return;
        let intent = action === "retry" ? pending : action === "edit" ? { path: edit.current.value, previousPath: edit.current.previousPath } : undefined;
        if (intent?.path.length === 0) { setError({ message: "Choose a folder or enter an absolute directory path." }); return; }
        guard.current = true;
        const request = generation.current;
        setBusy(true);
        try {
            if (action === "select") {
                const path = await client.selectDirectory();
                if (request !== generation.current || path === undefined) return;
                intent = { path, previousPath: acceptedRef.current.settings.paths[directoryKey] };
            }
            if (intent === undefined) return;
            setPending(intent);
            const result = await settingsWriter.saveDirectory(directoryKey, intent.path, intent.previousPath);
            if (request !== generation.current) return;
            reset(result); setError(undefined); setPending(undefined); failedEdit.current = undefined;
        } catch (caught: unknown) {
            if (request !== generation.current) return;
            if (action === "edit") failedEdit.current = intent?.path;
            if (caught instanceof StorageDirectoryConflictError) {
                setPending(undefined);
                setError({ message: "This location changed. Press Escape to load the current path before editing again." });
            } else {
                const cause = normalizeRequestError(caught);
                setError({ message: intent === undefined ? "The folder picker could not be opened."
                    : "The current save location could not be checked.", cause });
            }
        } finally {
            if (request === generation.current) { guard.current = false; setBusy(false); }
        }
    };
    return <div className="settings-directory" onBlur={(event) => {
        let target: EventTarget | null = event.relatedTarget;
        while (target instanceof Node) {
            if (event.currentTarget.contains(target) || feedbackRef.current?.contains(target)) return;
            const root = target.getRootNode();
            target = root instanceof ShadowRoot ? root.host : null;
        }
        void save("edit", true);
    }}>
        <div className="settings-path-row">
            <TextField id={`settings-${directoryKey}`} label={label} readOnly={busy} value={draft}
                onInput={(value) => {
                    if (!edit.current.dirty) edit.current.previousPath = acceptedRef.current.settings.paths[directoryKey];
                    edit.current.value = value;
                    edit.current.dirty = value !== acceptedRef.current.directories[directoryKey];
                    setDraft(value); setError(undefined); failedEdit.current = undefined;
                }} onKeyDown={(event) => {
                    if (event.key === "Escape") {
                        if (state.failed) settingsWriter.change({ paths: { [directoryKey]: acceptedRef.current.settings.paths[directoryKey] } });
                        event.preventDefault(); event.stopPropagation(); reset(acceptedRef.current);
                        setError(undefined); setPending(undefined); failedEdit.current = undefined;
                    } else if (event.key === "Enter" && !event.nativeEvent.isComposing && event.keyCode !== 229) {
                        event.preventDefault(); void save("edit");
                    }
                }} />
            <Button softDisabled={busy || state.saving} onClick={() => void save("select")} type="button">{t("Select")}</Button>
        </div>
        {error === undefined || state.failed ? null : error.cause === undefined ? <Reminder tone="error">{t(error.message)}</Reminder>
            : <GlobalReminder ref={feedbackRef}><RequestReminder error={error.cause} title={t(error.message)} message={error.message === "The current save location could not be checked." && ["internal_error", "daemon_unavailable"].includes(error.cause.code) ? "Retry checks the current path before writing again." : undefined}>
                {pending === undefined ? null : <Button disabled={state.failed} softDisabled={busy || state.saving} onClick={() => void save("retry")} type="button" variant="text">{t("Retry")}</Button>}
            </RequestReminder></GlobalReminder>}
    </div>;
}

function UnityHubSettingsSection({ client, settingsWriter, snapshot, onInstallationsChanged }: {
    client: GuiRpcClient; settingsWriter: SettingsAutosave; snapshot: SettingsGetResult; onInstallationsChanged(): void;
}) {
    const { t } = useTranslation();
    const state = useSyncExternalStore(settingsWriter.subscribe, settingsWriter.getSnapshot);
    const accepted = state.accepted ?? snapshot;
    const acceptedRef = useRef(accepted);
    acceptedRef.current = accepted;
    const manageUnity = useCapability(capabilities.unityManage);
    const canOpenHub = useCapability(capabilities.unityHubOpen);
    const [busy, setBusy] = useState(false);
    const [error, setErrorState] = useState<SettingsFeedback>();
    const [openError, setOpenErrorState] = useState<SettingsFeedback>();
    const setError = useCallback((message?: string, cause?: unknown) => setErrorState(message === undefined ? undefined
        : { message, cause: cause === undefined ? undefined : normalizeRequestError(cause) }), []);
    const setOpenError = useCallback((message?: string, cause?: unknown) => setOpenErrorState(message === undefined ? undefined
        : { message, cause: cause === undefined ? undefined : normalizeRequestError(cause) }), []);
    const [pending, setPending] = useState<UnityHubIntent>();
    const [draft, setDraft] = useState(accepted.unityHub.path ?? accepted.settings.unity.hubPath ?? "");
    const edit = useRef({ value: draft, display: draft, previousPath: accepted.settings.unity.hubPath, dirty: false });
    const failedEdit = useRef<string | undefined>(undefined);
    const feedbackRef = useRef<HTMLDivElement>(null);
    const errorOrigin = useRef<UnityHubIntent["origin"] | "refresh" | undefined>(undefined);
    const group = useRef<HTMLElement>(null);
    const guard = useRef(false);
    const active = useRef(true);
    const generation = useRef(0);
    const resetDraft = (value: SettingsGetResult) => {
        const path = value.unityHub.path ?? value.settings.unity.hubPath ?? "";
        edit.current = { value: path, display: path, previousPath: value.settings.unity.hubPath, dirty: false };
        setDraft(path);
    };
    useEffect(() => {
        active.current = true;
        setBusy(false);
        setError(undefined);
        setOpenError(undefined);
        setPending(undefined);
        failedEdit.current = undefined;
        errorOrigin.current = undefined;
        resetDraft(acceptedRef.current);
        return () => { active.current = false; generation.current += 1; guard.current = false; };
    }, [client, settingsWriter, setError, setOpenError]);
    useEffect(() => {
        if (!edit.current.dirty) resetDraft(accepted);
    }, [accepted]);

    const run = async (action: UnityHubAction) => {
        if (!active.current || guard.current || state.failed || (state.saving && action.kind !== "edit")) return;
        if (action.kind === "edit" && !edit.current.dirty) return;
        if (action.kind === "edit" && action.trigger === "blur" && edit.current.value === failedEdit.current) return;
        let intent = action.kind === "retry" ? pending : undefined;
        if (action.kind === "edit" || (intent?.origin === "edit" && action.kind === "retry")) {
            if (!edit.current.dirty) { errorOrigin.current = undefined; setError(undefined); setPending(undefined); return; }
            if (edit.current.value.length === 0) {
                setPending(undefined);
                errorOrigin.current = "edit";
                setError("Enter a Unity Hub executable path, or use Auto detect.");
                return;
            }
            intent = { path: edit.current.value, previousPath: edit.current.previousPath, origin: "edit" };
        }
        guard.current = true;
        const request = generation.current;
        let restoreDraftFocus = false;
        setBusy(true);
        setOpenError(undefined);
        try {
            if (action.kind === "auto" || action.kind === "select") {
                const path = action.kind === "auto" ? null : await client.selectUnityHub(acceptedRef.current.unityHub.path);
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
            let refreshFailed = false;
            if (manageUnity && (intent.path !== null || intent.previousPath !== null)) {
                try {
                    await client.unityInstallationsRefresh();
                    if (request === generation.current) onInstallationsChanged();
                } catch (cause: unknown) {
                    refreshFailed = true;
                    errorOrigin.current = "refresh";
                    if (request === generation.current) setError("Unity Hub path was saved, but Unity installations could not be refreshed.", cause);
                }
            }
            if (!refreshFailed && request === generation.current) { errorOrigin.current = undefined; setError(undefined); }
        } catch (cause) {
            if (request !== generation.current) return;
            errorOrigin.current = intent?.origin ?? "select";
            if (intent?.origin === "edit") failedEdit.current = intent.path ?? undefined;
            if (cause instanceof UnityHubConflictError) {
                setPending(undefined);
                setError(intent?.origin === "auto" ? "Unity Hub path changed. Use Auto detect again to replace it."
                    : intent?.origin === "edit" ? "Unity Hub path changed. Press Escape to load the current path before editing again."
                        : "Unity Hub path changed. Select the file again to replace it.");
            } else {
                const safe = normalizeRequestError(cause);
                setError(intent === undefined ? "The file picker could not be opened."
                    : (safe.code === "daemon_unavailable" || safe.code === "internal_error") && !(intent.path === null && intent.previousPath === null) ? "Unity Hub save result is unknown"
                    : intent.origin === "edit"
                    ? "Unity Hub path could not be saved. Check the path and try again."
                    : intent.path === null ? "Unity Hub location could not be detected. Retry the operation."
                        : "Unity Hub path could not be saved. Select a usable executable or retry.", safe);
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
        setOpenError(undefined);
        try {
            const current = acceptedRef.current;
            const result = await client.unityHubOpen(current.revision, current.unityHub.path);
            if (request !== generation.current) return;
            if (result.outcome === "download_required") {
                try { await client.openUnityHubDownload(); }
                catch (cause: unknown) {
                    if (request === generation.current) setOpenError("The Unity Hub download page could not be opened.", cause);
                    return;
                }
            }
            if (request === generation.current) setOpenError(undefined);
        } catch (cause) {
            if (request !== generation.current) return;
            const error = normalizeRequestError(cause);
            const code = error.code;
            if (["revision_conflict", "hub_path_changed", "hub_path_invalid", "hub_unavailable"].includes(code)) {
                try { await settingsWriter.reload(); }
                catch (caught: unknown) {
                    if (request === generation.current) setOpenError("Unity Hub settings could not be refreshed. Refresh Settings before trying again.", caught);
                    return;
                }
                if (request === generation.current) setOpenError("Unity Hub location changed. Review the current path and click again.", error);
            } else if (code === "hub_open_failed") {
                setOpenError("Unity Hub could not be opened.", error);
            } else if (code === "hub_open_busy") {
                setOpenError("Unity Hub is already being opened. Check its window before trying again.", error);
            } else if (code === "permission_denied") {
                setOpenError("Opening Unity Hub is not permitted on this connection.", error);
            } else {
                setOpenError("The result of opening Unity Hub is unknown. Check Unity Hub before trying again.", error);
            }
        } finally {
            if (request === generation.current) { guard.current = false; setBusy(false); }
        }
    };
    const readUnknown = (error?.message === "Unity Hub path was saved, but Unity installations could not be refreshed."
        || error?.message === "Unity Hub location could not be detected. Retry the operation." && pending?.path === null && pending.previousPath === null)
        && (error.cause?.code === "internal_error" || error.cause?.code === "daemon_unavailable");
    return (
        <SettingsSection title={t("Unity Hub Path")} sectionRef={group}
            actions={<div className="action-row">
                <Button disabled={state.failed} softDisabled={busy || state.saving} onClick={() => void run({ kind: "auto" })} type="button" variant="tonal">{t("Auto detect")}</Button>
                <Button disabled={!canOpenHub || (!usable && !download)} softDisabled={busy || state.saving || state.failed || edit.current.dirty || pending !== undefined}
                    aria-describedby={openDisabledReason === undefined ? undefined : "settings-unity-hub-open-reason"} title={openDisabledReason}
                    widthLabels={[t("Open Unity Hub"), t("Download Unity Hub")]} onClick={() => void openHub()} type="button">{openLabel}</Button>
            </div>}
            onBlur={(event) => {
                let target: EventTarget | null = event.relatedTarget;
                while (target instanceof Node) {
                    if (event.currentTarget.contains(target) || feedbackRef.current?.contains(target)) return;
                    const root = target.getRootNode();
                    target = root instanceof ShadowRoot ? root.host : null;
                }
                void run({ kind: "edit", trigger: "blur" });
            }}>
            <div className="settings-path-row">
                <TextField id="settings-unity-hub-path" label={t("Unity Hub Path")} readOnly={busy || state.failed}
                    placeholder={location.path === null && accepted.settings.unity.hubPath === null ? pathLabel : undefined} value={draft}
                    onFocus={() => { if (!edit.current.dirty) resetDraft(acceptedRef.current); }}
                    onInput={(value) => {
                        const current = acceptedRef.current;
                        if (value === edit.current.display || value === (current.unityHub.path ?? current.settings.unity.hubPath ?? "")) {
                            resetDraft(current);
                            if (errorOrigin.current === "edit") {
                                failedEdit.current = undefined; errorOrigin.current = undefined;
                                setError(undefined); setPending((intent) => intent?.origin === "edit" ? undefined : intent);
                            }
                            return;
                        }
                        edit.current.value = value;
                        edit.current.dirty = value !== edit.current.display;
                        setDraft(value);
                    }}
                    onKeyDown={(event) => {
                        if (event.key === "Escape") {
                            event.preventDefault(); event.stopPropagation();
                            resetDraft(acceptedRef.current); failedEdit.current = undefined; errorOrigin.current = undefined; setError(undefined); setPending(undefined);
                        } else if (event.key === "Enter" && !event.nativeEvent.isComposing && event.keyCode !== 229) {
                            event.preventDefault();
                            void run({ kind: "edit", trigger: "enter" });
                        }
                    }} />
                <Button disabled={state.failed} softDisabled={busy || state.saving} onClick={() => void run({ kind: "select" })} type="button">{t("Select")}</Button>
            </div>
            {location.configuredPathStatus === "invalid" ? <Reminder tone="warning">{t("The saved Unity Hub path is no longer usable.")}</Reminder> : null}
            {error === undefined ? null : error.cause === undefined ? <Reminder tone="error">{t(error.message)}</Reminder>
                : <GlobalReminder ref={feedbackRef}><RequestReminder error={error.cause} title={readUnknown ? null : t(error.message)}
                    message={readUnknown ? error.message : error.message === "Unity Hub save result is unknown" ? "Retry checks the current path before writing again." : undefined}>
                    {pending === undefined ? null : <Button disabled={state.failed} softDisabled={busy || state.saving} onClick={() => void run({ kind: "retry" })} type="button" variant="text">{t("Retry")}</Button>}
                </RequestReminder></GlobalReminder>}
            {openDisabledReason === undefined ? null : <span className="visually-hidden" id="settings-unity-hub-open-reason">{openDisabledReason}</span>}
            {openError === undefined ? null : <GlobalReminder>{openError.cause === undefined ? <Reminder tone="error">{t(openError.message)}</Reminder>
                : <RequestReminder error={openError.cause} title={null} message={openError.message} />}</GlobalReminder>}
        </SettingsSection>
    );
}

function UnitySettingsSection({ client, initialRead, externalGeneration }: { client: GuiRpcClient; externalGeneration: number; initialRead: Promise<PageReadResult<Awaited<ReturnType<GuiRpcClient["unityInstallationsList"]>> | undefined>> }) {
    const { t } = useTranslation();
    const available = useCapability(capabilities.unityRead);
    const manage = useCapability(capabilities.unityManage);
    const initial = use(initialRead);
    const [items, setItems] = useState<UnityInstallation[]>(initial.kind === "ready" ? initial.value?.installations ?? [] : []);
    const [cursor, setCursor] = useState<string | undefined>(initial.kind === "ready" ? initial.value?.nextCursor : undefined);
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState<RpcError | undefined>(initial.kind === "error" ? normalizeRequestError(initial.error) : undefined);
    const [open, setOpen] = useState(false);
    const [generation, setGeneration] = useState(0);
    const requestRef = useRef(0);
    const busyRef = useRef(false);

    const read = useCallback(async (next?: string) => {
        if (busyRef.current) return;
        const request = ++requestRef.current;
        busyRef.current = true;
        setBusy(true);
        setError(undefined);
        try {
            const page = await client.unityInstallationsList(next);
            if (request !== requestRef.current) return;
            setItems((current) => next === undefined ? page.installations : [...current, ...page.installations]);
            setCursor(page.nextCursor);
        } catch (caught: unknown) {
            if (request === requestRef.current) setError(normalizeRequestError(caught));
        } finally {
            if (request === requestRef.current) { busyRef.current = false; setBusy(false); }
        }
    }, [client]);

    useEffect(() => {
        if (available && (generation > 0 || externalGeneration > 0)) void read();
        return () => { requestRef.current += 1; busyRef.current = false; };
    }, [available, read, generation, externalGeneration]);

    return (
        <SettingsSection title={t("Unity installations")} actions={
            <div className="action-row">
                <RefreshButton disabled={!available} label={t("Refresh Unity installations")} onClick={() => setGeneration((current) => current + 1)} refreshing={busy} refreshingLabel={t("Refreshing Unity installations")} />
                <Button disabled={!manage} onClick={() => setOpen(true)} type="button">{t("Manage installations")}</Button>
            </div>
        }>
            {!available ? <p className="field-hint">{t("Unity installations are unavailable on this connection.")}</p> : (
                <>
                    {error === undefined ? null : <RequestReminder error={error} title={t("Unity installations could not be loaded.")} />}
                    <DesktopList columns={[{ key: "version", header: t("Version"), width: "20%" }, { key: "path", header: t("Path"), className: "settings-path" }, { key: "source", header: t("Source"), width: 160 }]} label={t("Unity installations")} minWidth={600} rows={items.map((item) => ({ key: item.installationId, cells: [item.unityVersion, <span title={displayPath(item.executablePath)}>{displayPath(item.executablePath)}</span>, item.sourceKind === "manual" ? t("Manual") : item.sourceKind === "unity_hub" ? t("Unity Hub") : item.sourceKind.replaceAll("_", " ")] }))} />
                    {items.length === 0 && error === undefined ? busy ? <LoadingState title={t("Loading Unity installations…")} /> : <p role="status">{t("No Unity installations registered")}</p> : null}
                    {cursor === undefined ? null : <Button disabled={busy} onClick={() => void read(cursor)} type="button" variant="text">{t("Load more installations")}</Button>}
                </>
            )}
            {open ? <UtilityActionDialog title={t("Manage Unity installations")} onClose={() => setOpen(false)}>
                <UnityRegistryActions client={client} installations={items} onChanged={() => setGeneration((current) => current + 1)} />
            </UtilityActionDialog> : null}
        </SettingsSection>
    );
}

function SettingsEditor({ category, client, settingsWriter, snapshot, initialRepositories, onApplied, onThemePreview, onThemeSession, onDirtyChange }: SettingsWorkspaceProps & { category: SettingsCategory; snapshot: SettingsGetResult; initialRepositories: Promise<PageReadResult<Awaited<ReturnType<GuiRpcClient["repositoriesList"]>> | undefined>> }) {
    const { t } = useTranslation();
    const initial = use(initialRepositories);
    const saving = useSyncExternalStore(settingsWriter.subscribe, settingsWriter.getSnapshot);
    const baseline = saving.accepted ?? snapshot;
    const settings = saving.settings ?? snapshot.settings;
    const [themeDirty, setThemeDirty] = useState(false);
    useEffect(() => {
        settingsWriter.seed(snapshot);
        onApplied(snapshot);
    }, [settingsWriter, snapshot, onApplied]);
    const [repositories, setRepositories] = useState<RepositorySnapshot[]>(initial.kind === "ready" ? initial.value?.repositories ?? [] : []);
    const [repositoryCursor, setRepositoryCursor] = useState<Parameters<GuiRpcClient["repositoriesList"]>[0]>(initial.kind === "ready" ? initial.value?.nextCursor : undefined);
    const [repositoryError, setRepositoryError] = useState<RpcError | undefined>(initial.kind === "error" ? normalizeRequestError(initial.error) : undefined);
    const [repositoryBusy, setRepositoryBusy] = useState(false);
    const repositoryBusyRef = useRef(false);
    const repositoryRequestRef = useRef(0);
    const canReadRepositories = useCapability(capabilities.repositoriesRead);
    useEffect(() => {
        onDirtyChange(themeDirty);
        return () => onDirtyChange(false);
    }, [themeDirty, onDirtyChange]);

    const readRepositories = useCallback(async (cursor?: Parameters<GuiRpcClient["repositoriesList"]>[0]) => {
        if (repositoryBusyRef.current) return;
        const request = ++repositoryRequestRef.current;
        repositoryBusyRef.current = true;
        setRepositoryBusy(true);
        setRepositoryError(undefined);
        try {
            const page = await client.repositoriesList(cursor);
            if (request !== repositoryRequestRef.current) return;
            setRepositories((current) => cursor === undefined ? page.repositories : [...current, ...page.repositories]);
            setRepositoryCursor(page.nextCursor);
        } catch (caught: unknown) {
            if (request === repositoryRequestRef.current) setRepositoryError(normalizeRequestError(caught));
        } finally {
            if (request === repositoryRequestRef.current) { repositoryBusyRef.current = false; setRepositoryBusy(false); }
        }
    }, [client]);

    useEffect(() => {
        return () => { repositoryRequestRef.current += 1; repositoryBusyRef.current = false; };
    }, [client]);

    const packages = <K extends keyof OfficialSettings["packages"]>(key: K, value: OfficialSettings["packages"][K]) => {
        settingsWriter.change({ packages: { [key]: value } });
    };
    return (
        <div className="settings-editor" hidden={category !== "general" && category !== "appearance"}>
            <SettingsSection title={t("Packages")} hidden={category !== "general"}>
                <div className="settings-toggle-row">
                    <Checkbox checked={settings.packages.showPrerelease} label={t("Show prerelease package versions")} onChange={(checked) => packages("showPrerelease", checked)} />
                    <p className="field-hint">{t("Include prerelease versions in package lists.")}</p>
                </div>
                <div className="settings-toggle-row">
                    <Checkbox checked={settings.packages.hideLocalUserPackages} label={t("Hide local User Packages")} onChange={(checked) => packages("hideLocalUserPackages", checked)} />
                </div>
                <div className="settings-source-options">
                    {settings.packages.hiddenRepositoryIds.length < 256 ? null : <Reminder tone="warning">{t("Up to 256 repositories can be hidden. Unhide one before hiding another.")}</Reminder>}
                    {repositories.map((repository) => repository.repositoryId === undefined ? null : (
                        <Checkbox key={repository.repositoryId} checked={settings.packages.hiddenRepositoryIds.includes(repository.repositoryId)}
                            disabled={settings.packages.hiddenRepositoryIds.length >= 256 && !settings.packages.hiddenRepositoryIds.includes(repository.repositoryId)}
                            label={t("Hide {name}", { name: repository.name ?? repository.declaredId ?? repository.repositoryId })}
                            onChange={(checked) => {
                                const hidden = new Set((settingsWriter.getSnapshot().settings ?? settings).packages.hiddenRepositoryIds);
                                if (checked) {
                                    if (!hidden.has(repository.repositoryId as string) && hidden.size >= 256) return;
                                    hidden.add(repository.repositoryId as string);
                                }
                                else hidden.delete(repository.repositoryId as string);
                                packages("hiddenRepositoryIds", [...hidden].sort());
                            }} />
                    ))}
                    {repositoryError === undefined ? null : <RequestReminder error={repositoryError} title={t("Repository visibility options could not be loaded.")}><Button disabled={repositoryBusy} onClick={() => void readRepositories(repositoryCursor)} type="button" variant="text">{t("Retry repository options")}</Button></RequestReminder>}
                    {repositoryCursor === undefined ? null : <Button disabled={repositoryBusy} onClick={() => void readRepositories(repositoryCursor)} type="button" variant="text">{t("Load more repository options")}</Button>}
                </div>
            </SettingsSection>
            <SettingsSection title={t("Appearance")} hidden={category !== "appearance"}>
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
            <SettingsSection title={t("Window")} hidden={category !== "appearance"}>
                <Checkbox checked={settings.window.rememberState} label={t("Remember window size and maximized state")}
                    onChange={(rememberState) => settingsWriter.change({ window: { rememberState } })} />
            </SettingsSection>
            <SettingsSection title={t("Theme")} hidden={category !== "appearance"}>
                <ThemeControls value={themePreferences(baseline)} disabled={saving.saving || saving.failed} onPreview={onThemePreview} onSession={onThemeSession} onDirtyChange={setThemeDirty}
                    onSave={(theme) => settingsWriter.saveTheme(theme)} onReload={() => settingsWriter.reload()} />
            </SettingsSection>
        </div>
    );
}
