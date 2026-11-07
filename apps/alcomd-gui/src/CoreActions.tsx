import { requestErrorMessage } from "./request-errors";
import { useCoreEvents } from "./core-events-ui";
import { Translated, useTranslation } from "./i18n";
import type { Translate } from "./i18n-types";
import type { ExtensionRecord, RepositoryCredentials, RpcError } from "@alcomd/sdk";
import { queryCandidates } from "./package-candidates";
import { useUtilityDialogBusy, useUtilityDialogClose, UtilityActionDialog, UtilityDialogCancel } from "./UtilityActionDialog";
import { useCallback, useEffect, useRef, useState, type FormEvent, type ReactNode } from "react";

import type {
    BackupCreateResult,
    ExtensionPlan,
    Operation,
    OfficialSettings,
    PackagePlan,
    PackageSourceSelector,
    ProjectSnapshot,
    ProjectUnityLaunchConfigResult,
    ProjectUnityMigrationPlan,
    RepositorySnapshot,
    UnityInstallation,
    UnityLaunchOptionsResult
} from "./core-models";
import type { GuiRpcClient } from "./rpc";
import { cancelIcon, closeIcon, editIcon, moreVertIcon } from "@alcomd/ui/icons";
import { DialogCloseButton, Button, Checkbox, Dialog as MaterialDialog, Icon, IconButton, PathField, Progress, RefreshButton, Select, TextField, type DialogOrigin } from "./Material";
import { LoadingState, useDelayedPending } from "./LoadingState";
import { useProjectUnityLaunch, useUnityWindowAction, type ProjectUnityLaunchAction } from "./ProjectUnityLaunch";
import { capabilities, capabilityUnavailableTitle, useCapability } from "./capabilities";
import { displayPath } from "./display-path";
import { backupCompressionOptions } from "./backup-compression";
import { repositoryDefaultName, repositoryDisplayName } from "./repository-display";
import type { RepositoryLinkDraft } from "./repository-link";
import { UnityArgumentsEditor } from "./UnityArgumentsEditor";
import { ExtensionPermissions, ExtensionPermissionRequests, hasPermissionRequests } from "./ExtensionPermissions";

interface ActionProps {
    client: GuiRpcClient;
    onChanged?(): void;
}

interface FeedbackState {
    busy: boolean;
    error?: RpcError;
    operationId?: string;
}

const INITIAL_FEEDBACK: FeedbackState = { busy: false };

interface RepositoryAuthenticationDraft {
    headers: Array<{ name: string; value: string }>;
}

const EMPTY_REPOSITORY_AUTH: RepositoryAuthenticationDraft = { headers: [] };

function repositoryCredentials(draft: RepositoryAuthenticationDraft): RepositoryCredentials | undefined {
    if (draft.headers.length === 0) return undefined;
    return {
        headers: Object.fromEntries(draft.headers.map(({ name, value }) => [name.trim(), value])),
    };
}

function hasDuplicateRepositoryHeaderNames(draft: RepositoryAuthenticationDraft): boolean {
    const names = draft.headers.map(({ name }) => name.trim().toLowerCase()).filter(Boolean);
    return new Set(names).size !== names.length;
}

function readyRepositoryHeaders(draft: RepositoryAuthenticationDraft): boolean {
    return !hasDuplicateRepositoryHeaderNames(draft)
        && draft.headers.every(({ name, value }) => name.trim() !== "" && value !== "");
}

function RepositoryAuthenticationFields({ draft, onChange, disabled }: {
    draft: RepositoryAuthenticationDraft;
    onChange(value: RepositoryAuthenticationDraft): void;
    disabled: boolean;
}) {
    const { t } = useTranslation();
    return <>
        {draft.headers.map((header, index) => <div key={index} className="form-row">
            <TextField disabled={disabled} label={t("Header name")} value={header.name} maxLength={128} required
                onInput={(name) => onChange({ ...draft, headers: draft.headers.map((row, i) => i === index ? { ...row, name } : row) })} />
            <TextField disabled={disabled} label={t("Header value")} value={header.value} maxLength={2048} type="password" required
                onInput={(value) => onChange({ ...draft, headers: draft.headers.map((row, i) => i === index ? { ...row, value } : row) })} />
            <IconButton disabled={disabled} label={t("Remove request header")} type="button"
                onClick={() => onChange({ ...draft, headers: draft.headers.filter((_, i) => i !== index) })}><Icon asset={closeIcon} /></IconButton>
        </div>)}
        {hasDuplicateRepositoryHeaderNames(draft) ? <p role="alert">{t("Request header names must be unique, regardless of case.")}</p> : null}
        <Button disabled={disabled || draft.headers.length >= 16} type="button" variant="text"
            onClick={() => onChange({ ...draft, headers: [...draft.headers, { name: "", value: "" }] })}>{t("Add request header")}</Button>
    </>;
}

export function RegisterRepositoryPanel({ client, onChanged, initial }: ActionProps & { initial?: RepositoryLinkDraft }) {
    const { t } = useTranslation();

    const close = useUtilityDialogClose();
    const current = useActionScope(client);
    const available = useCapability(capabilities.repositoriesRegistry);
    const [kind, setKind] = useState<"remote" | "local">("remote");
    const [value, setValue] = useState(initial?.url ?? "");
    const [authentication, setAuthentication] = useState<RepositoryAuthenticationDraft>(() => initial
        ? { headers: initial.headers } : EMPTY_REPOSITORY_AUTH);
    const [preview, setPreview] = useState<RepositorySnapshot>();
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    useUtilityDialogBusy(feedback.busy);
    const pending = useRef(false);
    const source = () => kind === "remote" ? { kind, url: value } as const : { kind, path: value } as const;
    const inspect = async (event: FormEvent) => {
        event.preventDefault();
        if (pending.current || !available || !readyRepositoryHeaders(authentication)) return;
        pending.current = true;
         setFeedback({ busy: true });
        try {
            const selected = source();
            const result = await client.repositoriesInspect(selected,
                selected.kind === "remote" ? repositoryCredentials(authentication) : undefined);
            if (current()) { setPreview(result.repository); setFeedback({ busy: false }); }
        }
        catch (caught: unknown) { if (current()) setFeedback({ busy: false, error: safeError(caught) }); }
        finally { pending.current = false; }
    };
    const register = async () => {
        if (pending.current || !available || preview === undefined) return;
        pending.current = true;
        setFeedback({ busy: true });
        try {
            await client.repositoryRegister(preview.source,
                preview.source.kind === "remote" ? repositoryCredentials(authentication) : undefined);
            if (!current()) return;
            setPreview(undefined); setValue(""); setAuthentication(EMPTY_REPOSITORY_AUTH);
            setFeedback(INITIAL_FEEDBACK);  close(); onChanged?.();
        } catch (caught: unknown) { if (current()) setFeedback({ busy: false, error: safeError(caught) }); }
        finally { pending.current = false; }
    };
    const chooseFile = async () => {
        if (pending.current) return;
        pending.current = true;
        setFeedback({ busy: true });
        try {
            const selected = await client.selectFile("repositoryManifest");
            if (current()) {
                if (selected !== undefined) setValue(selected);
                setFeedback(INITIAL_FEEDBACK);
            }
        } catch (caught) { if (current()) setFeedback({ busy: false, error: safeError(caught) }); }
        finally { pending.current = false; }
    };
    const fieldError = preview === undefined && feedback.error !== undefined && ["repository_source_invalid", "repository_document_invalid", "invalid_params"].includes(feedback.error.code) ? feedback.error : undefined;
    const error = feedback.error === undefined || fieldError !== undefined ? null : <div role="alert"><p>{t(requestErrorMessage((feedback.error).code))}</p><code>{(feedback.error).code}</code>{feedback.error.diagnosticId === undefined ? null : <p><code>{feedback.error.diagnosticId}</code></p>}</div>;
    return <ActionSection title={t("Add repository")}>
        {preview === undefined ? <form onSubmit={(event) => void inspect(event)}>
            <Select disabled={feedback.busy} id="repository-kind" label={t("Source type")} onChange={(next) => { setKind(next as "remote" | "local"); setAuthentication(EMPTY_REPOSITORY_AUTH); setFeedback(INITIAL_FEEDBACK); }} options={[{ label: t("Remote URL"), value: "remote" }, { label: t("Local manifest"), value: "local" }]} value={kind} />
            {kind === "remote" ? <TextField disabled={feedback.busy} error={fieldError !== undefined} errorText={fieldError === undefined ? undefined : t("Check the repository URL or manifest path.")} id="repository-source" label={t("Repository URL")} maxLength={2048} onInput={(next) => { setValue(next); setFeedback(INITIAL_FEEDBACK); }} required type="url" value={value} />
                : <div className="project-target-picker">
                    <PathField disabled={feedback.busy} error={fieldError !== undefined} errorText={fieldError === undefined ? undefined : t("Check the repository URL or manifest path.")} id="repository-source" label={t("Local manifest path")} maxLength={2048} onInput={(next) => { setValue(next); setFeedback(INITIAL_FEEDBACK); }} required value={value} />
                    <Button disabled={feedback.busy} onClick={() => void chooseFile()} type="button" variant="tonal">{t("Select")}</Button>
                </div>}
            {kind === "remote" ? <RepositoryAuthenticationFields draft={authentication} disabled={feedback.busy} onChange={setAuthentication} /> : null}
            {error}
            <div className="dialog-actions"><UtilityDialogCancel disabled={feedback.busy} /><Button disabled={!available || feedback.busy || value.trim().length === 0 || !readyRepositoryHeaders(authentication)} title={capabilityUnavailableTitle(available, capabilities.repositoriesRegistry, t)} type="submit">{t("Review repository")}</Button></div>
        </form> : <>
            <dl className="dialog-summary"><div><dt>{t("Name")}</dt><dd>{preview.name ?? t("Unnamed repository")}</dd></div><div><dt>{t("Source")}</dt><dd>{preview.source.kind === "remote" ? preview.source.url : displayPath(preview.source.path)}</dd></div></dl>
            {authentication.headers.length > 0 ? <p>{t("Uses authentication")}</p> : null}
            {preview.issues.length === 0 ? null : <div>{t("This repository has {count} reported issues.", { count: preview.issues.length })}</div>}
            {error}
            <div className="dialog-actions"><UtilityDialogCancel disabled={feedback.busy} /><Button disabled={feedback.busy} onClick={() => { setPreview(undefined); setFeedback(INITIAL_FEEDBACK); }} type="button" variant="text">{t("Back")}</Button><Button disabled={!available || feedback.busy} onClick={() => void register()} type="button">{t("Add repository")}</Button></div>
        </>}
    </ActionSection>;
}

export function RepositoryActions({ client, onChanged, repository }: ActionProps & { repository: RepositorySnapshot }) {
    const { t } = useTranslation();

    const current = useActionScope(client);
    const available = useCapability(capabilities.repositoriesRegistry);
    const [confirmRemove, setConfirmRemove] = useState(false);
    const [managing, setManaging] = useState(false);
    const [nameDraft, setNameDraft] = useState<{ revision: number; initial: string; value: string }>();
    const [authentication, setAuthentication] = useState(EMPTY_REPOSITORY_AUTH);
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const pending = useRef<"refresh" | "remove" | "credentials" | "name" | undefined>(undefined);
    const ready = repository.repositoryId !== undefined && repository.revision !== undefined;
    const canRemove = repository.actions?.canRemove === true;
    const canEditAuthentication = repository.actions?.canEditAuthentication === true;
    const savingSettings = feedback.busy && (pending.current === "credentials" || pending.current === "name");
    const openManagement = () => {
        if (repository.revision === undefined) return;
        const value = repository.displayName ?? "";
        setNameDraft({ revision: repository.revision, initial: value, value });
        setManaging(true);
    };
    const closeManagement = () => {
        setManaging(false);
        setNameDraft(undefined);
        setAuthentication(EMPTY_REPOSITORY_AUTH);
        if (pending.current !== "refresh") setFeedback(INITIAL_FEEDBACK);
    };
    const saveName = async () => {
        if (!available || pending.current || repository.repositoryId === undefined
            || nameDraft === undefined || nameDraft.value === nameDraft.initial) return;
        pending.current = "name";
        setFeedback({ busy: true });
        try {
            await client.repositorySetDisplayName(repository.repositoryId, nameDraft.revision, nameDraft.value || null);
            if (current()) { closeManagement(); onChanged?.(); }
        } catch (caught: unknown) {
            if (current()) {
                const error = safeError(caught);
                setFeedback({ busy: false, error });
                if (error.code === "revision_conflict") onChanged?.();
            }
        } finally { pending.current = undefined; }
    };
    const changeAuthentication = async (clear: boolean) => {
        if (!ready || !available || pending.current || !canEditAuthentication
            || (!clear && (authentication.headers.length === 0 || !readyRepositoryHeaders(authentication)))) return;
        pending.current = "credentials";
        setFeedback({ busy: true });
        try {
            const credentials = clear ? null : repositoryCredentials(authentication);
            if (credentials === undefined) return;
            await client.repositorySetCredentials(repository.repositoryId!, repository.revision!, credentials);
            if (current()) { closeManagement(); onChanged?.(); }
        } catch (caught: unknown) {
            if (current()) setFeedback({ busy: false, error: safeError(caught) });
        } finally { pending.current = undefined; }
    };
    const run = async (remove: boolean) => {
        if (!ready || !available || pending.current || (remove && !canRemove)) return;
        pending.current = remove ? "remove" : "refresh";
        setFeedback({ busy: true });
        try {
            if (remove) await client.repositoryUnregister(repository.repositoryId!, repository.revision!);
            else {
                const refreshed = await client.repositoryRefresh(repository.repositoryId!, repository.revision!);
                const revision = refreshed.repository.revision;
                if (current() && revision !== undefined) {
                    setNameDraft((draft) => draft !== undefined && draft.revision === repository.revision
                        ? { ...draft, revision } : draft);
                }
            }
            if (!current()) return;
            setFeedback(INITIAL_FEEDBACK);
            setConfirmRemove(false); onChanged?.();
        } catch (caught: unknown) {
            if (!current()) return;
            const error = safeError(caught);
            setFeedback({ busy: false, error: remove ? error : undefined });
            if (!remove && error.code === "revision_conflict") onChanged?.();
        }
        finally { pending.current = undefined; }
    };
    return <div className="repository-row-actions">
        <IconButton disabled={!available || !ready || feedback.busy} label={t("Manage repository")} onClick={openManagement} title={t("Manage repository")} type="button"><Icon asset={editIcon} /></IconButton>
        <IconButton disabled={!canRemove || !available || !ready || feedback.busy} label={t("Remove")} onClick={() => { setFeedback(INITIAL_FEEDBACK); setConfirmRemove(true); }} title={t("Remove repository")} type="button"><Icon asset={cancelIcon} /></IconButton>
        <MaterialDialog open={managing} title={t("Manage repository")} dismissible={!savingSettings} onClose={closeManagement}>
            {nameDraft === undefined ? null : <form onSubmit={(event) => { event.preventDefault(); void saveName(); }}>
                <TextField disabled={feedback.busy} label={t("Display name")}
                    onInput={(value) => setNameDraft({ ...nameDraft, value })}
                    placeholder={repositoryDefaultName(t, repository, t("Repository"))}
                    supportingText={t("Leave blank to use the default name.")} value={nameDraft.value} />
                <Button disabled={feedback.busy || nameDraft.value === nameDraft.initial} type="submit">{t("Save name")}</Button>
            </form>}
            <RefreshButton disabled={!available || !ready || feedback.busy} label={t("Refresh repository")} onClick={() => void run(false)} refreshing={feedback.busy && pending.current === "refresh"} refreshingLabel={t("Refreshing repository")} />
            {canEditAuthentication ? <>
                <h3>{t("Repository authentication")}</h3>
                {repository.credentialsConfigured ? <p>{t("Uses authentication")}</p> : null}
                <RepositoryAuthenticationFields draft={authentication} disabled={feedback.busy} onChange={setAuthentication} />
            </> : null}
            <MutationFeedback client={client} feedback={feedback} />
            <div className="dialog-actions">
                {canEditAuthentication && repository.credentialsConfigured ? <Button disabled={feedback.busy} onClick={() => void changeAuthentication(true)}
                    type="button" variant="text">{t("Clear authentication")}</Button> : null}
                <DialogCloseButton disabled={savingSettings} variant="text">{t("Close")}</DialogCloseButton>
                {canEditAuthentication ? <Button disabled={feedback.busy || authentication.headers.length === 0 || !readyRepositoryHeaders(authentication)}
                    onClick={() => void changeAuthentication(false)} type="button">{t("Save")}</Button> : null}
            </div>
        </MaterialDialog>
        <MaterialDialog open={confirmRemove} title={t("Remove this repository?")} dismissible={!feedback.busy} onClose={() => { setConfirmRemove(false); setFeedback(INITIAL_FEEDBACK); }}>
            <p><Translated message="Remove {name} from your sources?" values={{ name: <strong>{repositoryDisplayName(t, repository, t("this repository"))}</strong> }} /></p><p>{t("Packages already installed in projects stay installed.")}</p>
            <MutationFeedback client={client} feedback={feedback} />
            <div className="dialog-actions"><DialogCloseButton disabled={feedback.busy} variant="text">{t("Cancel")}</DialogCloseButton><Button disabled={feedback.busy || !available} onClick={() => void run(true)} type="button">{t("Remove repository")}</Button></div>
        </MaterialDialog>
    </div>;
}

export interface PackageActionSelection {
    action: "install" | "remove" | "upgrade" | "downgrade" | "resolve" | "reinstall" | "reinstall-all" | "bulk-candidates";
    key: number;
    origin: DialogOrigin;
    packageId: string;
    source?: PackageSourceSelector;
    version?: string;
    includePrerelease?: boolean;
    intents?: import("./core-models").PackageBulkIntent[];
    exclusions?: string[];
    candidateSnapshot?: string;
}

export function PackageActions({ client, project, onChanged, selection, onEvidenceStale, onBusyChanged, returnFocusReady }: ActionProps & { project: ProjectSnapshot; selection?: PackageActionSelection; onEvidenceStale?(): void; onBusyChanged?(busy: boolean): void; returnFocusReady: boolean }) {
    const { t } = useTranslation();
    const canApply = useCapability(capabilities.packagesApply);
    const canPlanV1 = useCapability(capabilities.packagesPlanV1);
    const canPlanV2 = useCapability(capabilities.packagesPlanV2);
    const [plan, setPlan] = useState<PackagePlan>();
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const [operationFinished, setOperationFinished] = useState(false);
    useEffect(() => onBusyChanged?.(feedback.busy || plan !== undefined || (feedback.operationId !== undefined && !operationFinished)), [feedback.busy, feedback.operationId, onBusyChanged, operationFinished, plan]);
    const handledSelectionKey = useRef<number | undefined>(undefined);
    const revision = project.revision;
    const projectId = project.projectId;
    const prepareChanges = useCallback(async (action: PackageActionSelection["action"], selectedPackageId: string, selectedVersion = "", selectedSource?: PackageSourceSelector, includePrerelease = false) => {
        if (revision === undefined || projectId === undefined) return;
        const requiresV2 = ((action === "install" || action === "upgrade") && selectedSource !== undefined) || action === "reinstall" || action === "reinstall-all" || action === "bulk-candidates" || action === "downgrade";
        if ((requiresV2 && !canPlanV2) || (!requiresV2 && !canPlanV1)) {
            setFeedback({ busy: false, error: { code: "capability_required", message: t("The connected daemon did not negotiate {capability}.", { capability: requiresV2 ? capabilities.packagesPlanV2 : capabilities.packagesPlanV1 }) } });
            return;
        }
        setPlan(undefined);
        setFeedback({ busy: true });
        setOperationFinished(false);
        try {
            if (selection?.candidateSnapshot && action !== "remove") {
                const ids = selection.intents?.map((intent) => intent.packageId) ?? (selectedPackageId ? [selectedPackageId] : undefined);
                const evidence = await queryCandidates(client, { projectId, expectedRevision: revision, expectedSnapshot: selection.candidateSnapshot, view: { kind: "summary", ...(ids ? { packageIds: ids } : {}) }, limit: 1 });
                if (!evidence.catalogComplete) throw { code: "package_catalog_incomplete" };
            }
            let result: PackagePlan;
            if (action === "bulk-candidates") {
                if (!selection?.intents?.length) throw { code: "invalid_input" };
                result = await client.packagePlanBulk({ projectId, expectedRevision: revision, intents: selection.intents });
            }
            else if (action === "remove") result = await client.packagePlanRemove({ projectId, expectedRevision: revision, packageId: selectedPackageId });
            else if (action === "resolve") result = await client.packagePlanResolve({ projectId, expectedRevision: revision, includePrerelease: false });
            else if (action === "reinstall-all") result = await client.packagePlanReinstall({ projectId, expectedRevision: revision, selection: { kind: "all" } });
            else if (action === "reinstall") result = await client.packagePlanReinstall({ projectId, expectedRevision: revision, selection: { kind: "packages", packageIds: [selectedPackageId] }, ...(selectedSource === undefined ? {} : { sources: [{ packageId: selectedPackageId, source: selectedSource }] }) });
            else if (action === "downgrade") result = await client.packagePlanDowngrade({ projectId, expectedRevision: revision, packageId: selectedPackageId, version: selectedVersion, ...(selectedSource === undefined ? {} : { source: selectedSource }) });
            else {
                const params = { projectId, expectedRevision: revision, packageId: selectedPackageId, includePrerelease, ...(selectedVersion.length === 0 ? {} : { versionRange: selectedVersion }), ...(selectedSource === undefined ? {} : { source: selectedSource }) };
                result = action === "upgrade" ? await client.packagePlanUpgrade(params) : await client.packagePlanInstall(params);
            }
            setPlan(result);
            setFeedback({ busy: false });
        } catch (caught: unknown) {
            if (safeError(caught).code === "package_candidate_evidence_stale") onEvidenceStale?.();
            setFeedback({ busy: false, error: safeError(caught) });
        }
    }, [canPlanV1, canPlanV2, client, projectId, revision, selection, t]);
    useEffect(() => {
        if (selection === undefined) return;
        if (handledSelectionKey.current === selection.key) return;
        handledSelectionKey.current = selection.key;
        setPlan(undefined);
        setFeedback(INITIAL_FEEDBACK);
        void prepareChanges(selection.action, selection.packageId, selection.version, selection.source, selection.includePrerelease);
    }, [prepareChanges, selection]);
    const apply = async () => {
        if (plan === undefined) return;
        setFeedback({ busy: true });
        try {
            const result = await client.packageApplyPlan({ planId: plan.planId, expectedRevision: plan.projectRevision });
            setPlan(undefined);
            setFeedback({ busy: false, operationId: result.operationId });
            onChanged?.();
        } catch (caught: unknown) {
            setPlan(undefined);
            setFeedback({ busy: false, error: safeError(caught) });
        }
    };
    const closeChanges = () => { if (!feedback.busy) setPlan(undefined); };
    const legacyRemovals = plan?.changeSet.legacyRemovals ?? [];
    const hasChanges = (plan?.changeSet.mutations.length ?? 0) > 0 || legacyRemovals.length > 0;
    return (
        <>
            <MaterialDialog onClose={closeChanges} open={plan !== undefined} returnFocus={selection?.origin} returnFocusReady={returnFocusReady && feedback.operationId === undefined} title={hasChanges ? t("Apply package changes?") : t("Packages are up to date")}>
                {selection?.exclusions?.length ? <div role="status"><p>{t("Not included in this update:")}</p><ul>{selection.exclusions.map((reason) => <li key={reason}>{reason}</li>)}</ul></div> : null}
                {plan === undefined ? null : hasChanges ? (
                    <div className="package-plan-review">
                        <p>{t("Review the changes ALCOMD will make to this project.")}</p>
                        <ul className="change-list">{plan.changeSet.mutations.map((mutation) => <li key={`${mutation.kind}-${mutation.packageId}`}><strong>{t(packageChangeLabel(mutation.kind))}</strong><span>{mutation.packageId}</span>{mutation.fromVersion == null && mutation.toVersion == null ? null : <small>{packageVersionChange(mutation.fromVersion, mutation.toVersion, t)}</small>}</li>)}</ul>
                        {legacyRemovals.length === 0 ? null : <>
                            <p>{t("The following legacy files and folders will be removed.")}</p>
                            <ul className="change-list">{legacyRemovals.map(({ path, packageId }) => (
                                <li key={path}><span>{displayPath(path)}</span><small>{packageId}</small></li>
                            ))}</ul>
                        </>}
                        <div className="dialog-actions">
                            <DialogCloseButton disabled={feedback.busy} variant="text">{t("Cancel")}</DialogCloseButton>
                            <Button disabled={!canApply || feedback.busy} onClick={() => void apply()} title={capabilityUnavailableTitle(canApply, capabilities.packagesApply, t)} type="button">{t("Apply changes")}</Button>
                        </div>
                    </div>
                ) : (
                    <div className="package-plan-review"><p>{t("No package changes are required for this project.")}</p><div className="dialog-actions"><DialogCloseButton>{t("Close")}</DialogCloseButton></div></div>
                )}
            </MaterialDialog>
            {feedback.error === undefined ? null : <div role="alert"><strong>{t("Package changes were not applied")}</strong><p>{t((packageErrorMessage(feedback.error)) ?? requestErrorMessage((feedback.error).code))}</p><code>{(feedback.error).code}</code>{feedback.error.diagnosticId === undefined ? null : <p><code>{feedback.error.diagnosticId}</code></p>}</div>}
            {feedback.operationId === undefined ? null : <OperationFollow client={client} operationId={feedback.operationId} onTerminal={() => { setOperationFinished(true); onChanged?.(); }} title={t("Package changes")} />}
        </>
    );
}

export function UnityRegistryActions({ client, installations, onChanged }: ActionProps & { installations: UnityInstallation[] }) {
    const { t } = useTranslation();
    const available = useCapability(capabilities.unityManage);
    const [path, setPath] = useState("");
    const [remove, setRemove] = useState<UnityInstallation>();
    const [confirmRemove, setConfirmRemove] = useState(false);
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const pending = useRef(false);
    const isCurrent = useActionScope(client);
    useUtilityDialogBusy(feedback.busy);
    const mutate = async (action: "register" | "refresh" | "remove") => {
        if (!available || pending.current || (action === "remove" && remove === undefined)) return;
        pending.current = true; setFeedback({ busy: true });
        try {
            if (action === "register") { await client.unityInstallationRegister(path); setPath(""); }
            else if (action === "refresh") await client.unityInstallationsRefresh();
            else { await client.unityInstallationRemove(remove!.installationId, remove!.revision); setRemove(undefined); setConfirmRemove(false); }
            if (!isCurrent()) return;
            setFeedback({ busy: false });
            onChanged?.();
        } catch (caught: unknown) { if (isCurrent()) setFeedback({ busy: false, error: safeError(caught) }); }
        finally { pending.current = false; }
    };
    const chooseFile = async () => {
        if (pending.current) return;
        pending.current = true;
        setFeedback({ busy: true });
        try {
            const selected = await client.selectFile("unityEditor");
            if (isCurrent()) {
                if (selected !== undefined) setPath(selected);
                setFeedback(INITIAL_FEEDBACK);
            }
        } catch (caught) { if (isCurrent()) setFeedback({ busy: false, error: safeError(caught) }); }
        finally { pending.current = false; }
    };
    return <ActionSection title={confirmRemove ? t("Remove Unity installation") : t("Manage Unity installations")}>
        {confirmRemove && remove !== undefined ? <>
            <p><Translated message="Remove {name} from the list?" values={{ name: <strong>Unity {remove.unityVersion}</strong> }} /></p>
            <p>{t("Only the ALCOMD registry entry is removed. The editor remains installed.")}</p>
            <div className="dialog-actions"><UtilityDialogCancel disabled={feedback.busy} /><Button disabled={feedback.busy} onClick={() => { setConfirmRemove(false); setFeedback(INITIAL_FEEDBACK); }} type="button" variant="text">{t("Back")}</Button><Button className="material-button--danger" disabled={!available || feedback.busy} onClick={() => void mutate("remove")} type="button">{t("Remove installation")}</Button></div>
        </> : <>
            <form onSubmit={(event) => { event.preventDefault(); void mutate("register"); }}>
                <div className="project-target-picker">
                    <PathField disabled={feedback.busy} id="unity-executable" label={t("Unity executable")} maxLength={1024} onInput={(value) => { setPath(value); setFeedback(INITIAL_FEEDBACK); }} required value={path} />
                    <Button disabled={feedback.busy} onClick={() => void chooseFile()} type="button" variant="tonal">{t("Select")}</Button>
                </div>
                <div className="action-row"><Button disabled={!available || feedback.busy || path.trim().length === 0} title={capabilityUnavailableTitle(available, capabilities.unityManage, t)} type="submit">{t("Register")}</Button><Button disabled={!available || feedback.busy} onClick={() => void mutate("refresh")} title={capabilityUnavailableTitle(available, capabilities.unityManage, t)} type="button" variant="tonal">{t("Discover and refresh")}</Button></div>
            </form>
            {installations.length === 0 ? null : <Select aria-label={t("Installation to remove")} disabled={feedback.busy} label={t("Remove installation")} onChange={(next) => setRemove(installations.find((item) => item.installationId === next))} options={[{ label: t("Select an installation"), value: "" }, ...installations.map((item) => ({ label: "Unity " + item.unityVersion + " · " + item.architecture, value: item.installationId }))]} value={remove?.installationId ?? ""} />}
            <div className="dialog-actions"><UtilityDialogCancel disabled={feedback.busy} /><Button className="material-button--danger" disabled={!available || remove === undefined || feedback.busy} onClick={() => { setConfirmRemove(true); setFeedback(INITIAL_FEEDBACK); }} title={capabilityUnavailableTitle(available, capabilities.unityManage, t)} type="button" variant="text">{t("Review removal")}</Button></div>
        </>}
        <MutationFeedback client={client} errorTitle={feedback.error?.code === "unity_version_unverified" ? t("Unity Editor version could not be verified.") : feedback.error?.code === "unity_installation_invalid" ? t("Unity installation could not be validated.") : undefined} feedback={feedback} />
    </ActionSection>;
}

export function ProjectLaunchArgumentsDialog({ client, launchConfig, onChanged, onClose, projectId }: ActionProps & { launchConfig?: ProjectUnityLaunchConfigResult; onClose(): void; projectId: string }) {
    const { t } = useTranslation();
    const canManage = useCapability(capabilities.unityManage);
    const [open, setOpen] = useState(true);
    const initialConfig = useRef(launchConfig).current;
    const [snapshot, setSnapshot] = useState<ProjectUnityLaunchConfigResult>();
    const [arguments_, setArguments] = useState<string[]>([]);
    const [inherit, setInherit] = useState(true);
    const [conflict, setConflict] = useState(false);
    const [unknown, setUnknown] = useState(false);
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const pending = useRef(false);
    useEffect(() => {
        let active = true;
        setSnapshot(undefined);
        setFeedback(INITIAL_FEEDBACK);
        const read = initialConfig === undefined ? client.unityProjectLaunchConfigGet(projectId) : Promise.resolve(initialConfig);
        void read.then((value) => {
            if (!active) return;
            setSnapshot(value); setArguments([...value.effectiveArguments]); setInherit(value.config.revision === 0);
        }).catch((caught: unknown) => { if (active) setFeedback({ busy: false, error: safeError(caught) }); });
        return () => { active = false; };
    }, [client, initialConfig, projectId]);
    const finish = () => {
        setFeedback(INITIAL_FEEDBACK); setOpen(false);
        onChanged?.();
    };
    const save = async () => {
        if (!canManage || snapshot === undefined || pending.current || conflict) return;
        pending.current = true; setFeedback({ busy: true }); setUnknown(false);
        try {
            // Reconcile an earlier lost response before repeating a write.
            const current = await client.unityProjectLaunchConfigGet(projectId);
            if (inherit ? current.config.revision === 0 : current.config.revision > 0 && JSON.stringify(current.config.arguments) === JSON.stringify(arguments_)) { finish(); return; }
            if (current.config.revision !== snapshot.config.revision) { setConflict(true); setFeedback(INITIAL_FEEDBACK); return; }
            if (inherit) await client.unityProjectLaunchConfigClear(projectId, current.config.revision);
            else await client.unityProjectLaunchConfigSet(projectId, arguments_, current.config.revision);
            finish();
        } catch (caught: unknown) {
            try {
                const current = await client.unityProjectLaunchConfigGet(projectId);
                if (inherit ? current.config.revision === 0 : current.config.revision > 0 && JSON.stringify(current.config.arguments) === JSON.stringify(arguments_)) { finish(); return; }
                if (current.config.revision !== snapshot.config.revision) { setConflict(true); setFeedback(INITIAL_FEEDBACK); return; }
            } catch { setUnknown(true); }
            setFeedback({ busy: false, error: safeError(caught) });
        }
        finally { pending.current = false; }
    };
    return <MaterialDialog dismissible={!feedback.busy} open={open} onClose={onClose} title={t("Unity launch arguments")}>
        {snapshot === undefined ? feedback.error === undefined ? <LoadingState title={t("Loading launch arguments…")} /> : null : <form onSubmit={(event) => { event.preventDefault(); void save(); }}>
            <Select label={t("Launch arguments")} value={inherit ? "default" : "custom"} disabled={!canManage || feedback.busy || conflict}
                onChange={(value) => { setInherit(value === "default"); setFeedback(INITIAL_FEEDBACK); }}
                options={[{ value: "default", label: t("Use global defaults") }, { value: "custom", label: t("Custom arguments") }]} />
            <UnityArgumentsEditor id="project-unity-argument" value={inherit ? snapshot.defaultArguments : arguments_}
                onChange={(next) => { setArguments(next); setFeedback(INITIAL_FEEDBACK); }} disabled={!canManage || feedback.busy || conflict} readOnly={inherit} />
            {conflict ? <div>{t("These arguments changed elsewhere. Close and reopen this editor to load the current values.")}</div> : null}
            <div className="dialog-actions">
                <DialogCloseButton disabled={feedback.busy} variant="text">{t("Cancel")}</DialogCloseButton>
                <Button disabled={!canManage || feedback.busy || conflict || (inherit && snapshot.config.revision === 0)} type="submit">{t(feedback.error === undefined ? "Save launch arguments" : "Retry")}</Button>
            </div>
        </form>}
        {snapshot === undefined ? <div className="dialog-actions"><DialogCloseButton variant="text">{t("Cancel")}</DialogCloseButton></div> : null}
        <MutationFeedback client={client} errorTitle={unknown ? t("Launch argument save result is unknown") : undefined} feedback={feedback} />
    </MaterialDialog>;
}

export function ProjectUnityActions({ afterMigrationOpen = false, client, launchConfig, launchOptions, project, onChanged }: ActionProps & { afterMigrationOpen?: boolean; launchConfig: ProjectUnityLaunchConfigResult; launchOptions: UnityLaunchOptionsResult; project: ProjectSnapshot }) {
    const { t } = useTranslation();
    const canLaunch = useCapability(capabilities.unityLaunch);
    const canManage = useCapability(capabilities.unityManage);
    const canMigrate = useCapability(capabilities.projectsUnityMigration);
    const [argumentsOpen, setArgumentsOpen] = useState(false);
    const [launchInstallationId, setLaunchInstallationId] = useState("");
    const [postMigrationLaunchOptions, setPostMigrationLaunchOptions] = useState<UnityLaunchOptionsResult>();
    const [targetVersion, setTargetVersion] = useState("");
    const [targetInstallationId, setTargetInstallationId] = useState("");
    const [migrationPlan, setMigrationPlan] = useState<ProjectUnityMigrationPlan>();
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const projectId = project.projectId;
    const projectRevision = project.revision;
    const handledMigrationOperation = useRef<string | undefined>(undefined);
    const migrationVersions = launchOptions.migrationVersionOptions ?? [];
    const targetInstallations = migrationVersions.find((option) => option.unityVersion === targetVersion)?.installations ?? [];
    const effectiveLaunchOptions = postMigrationLaunchOptions ?? launchOptions;
    const candidates = effectiveLaunchOptions.exactMatchingInstallations;
    const launchInstallation = candidates.length === 1 ? candidates[0] : candidates.find((item) => item.installationId === launchInstallationId);
    const launch = async () => {
        if (projectId === undefined || launchInstallation === undefined) return;
        setFeedback({ busy: true });
        try {
            await client.unityLaunch(projectId, launchInstallation.installationId, effectiveLaunchOptions.projectRevision);
            setFeedback(INITIAL_FEEDBACK);
        } catch (caught: unknown) {
            setFeedback({ busy: false, error: safeError(caught) });
        }
    };
    const chooseTargetVersion = (version: string) => {
        setTargetVersion(version);
        const option = migrationVersions.find((item) => item.unityVersion === version);
        setTargetInstallationId(option?.defaultInstallationId ?? "");
        setMigrationPlan(undefined);
    };
    const planMigration = async () => {
        if (projectId === undefined || projectRevision === undefined || targetInstallationId.length === 0) return;
        setFeedback({ busy: true });
        try {
            const result = await client.projectPlanUnityMigration(projectId, targetInstallationId, projectRevision);
            if (result.kind === "no_change") {
                setFeedback(INITIAL_FEEDBACK);
                return;
            }
            setMigrationPlan(result.plan);
            setFeedback({ busy: false });
        } catch (caught: unknown) {
            setFeedback({ busy: false, error: safeError(caught) });
        }
    };
    const applyMigration = async () => {
        if (migrationPlan === undefined || !migrationPlan.classification.supportedForApply) return;
        setFeedback({ busy: true });
        try {
            const result = await client.projectApplyUnityMigration(migrationPlan.planId);
            setMigrationPlan(undefined);
            setFeedback({ busy: false, operationId: result.operationId });
        } catch (caught: unknown) {
            setFeedback({ busy: false, error: safeError(caught) });
        }
    };
    const migrationFinished = useCallback(async (operation: Operation) => {
        if (!afterMigrationOpen || handledMigrationOperation.current === operation.operationId) return;
        handledMigrationOperation.current = operation.operationId;
        if (operation.state !== "succeeded") {
            setFeedback({ busy: false, error: { code: operation.errorCode ?? "internal_error", message: t("The Unity migration did not complete.") } });
            return;
        }
        if (projectId === undefined) return;
        setFeedback({ busy: true });
        try {
            const freshProject = (await client.projectGet(projectId)).project;
            if (freshProject.revision === undefined) throw { code: "project_not_registered" };
            const options = await client.unityLaunchOptions(projectId, freshProject.revision);
            setPostMigrationLaunchOptions(options);
            onChanged?.();
            if (options.exactMatchingInstallations.length === 0) {
                setFeedback(INITIAL_FEEDBACK);
                return;
            }
            if (options.exactMatchingInstallations.length > 1) {
                setLaunchInstallationId("");
                setFeedback(INITIAL_FEEDBACK);
                return;
            }
            await client.unityLaunch(projectId, options.exactMatchingInstallations[0]!.installationId, freshProject.revision);
            setFeedback(INITIAL_FEEDBACK);
        } catch (caught: unknown) {
            setFeedback({ busy: false, error: safeError(caught) });
        }
    }, [afterMigrationOpen, client, onChanged, projectId, t]);
    const unityWindow = useUnityWindowAction(client, project, () => void launch(),
        !canLaunch || projectRevision === undefined || launchInstallation === undefined,
        feedback.busy, capabilityUnavailableTitle(canLaunch, capabilities.unityLaunch, t));
    const { icon: unityIcon, label: unityLabel, ...unityButton } = unityWindow.action;
    return (
        <ActionSection title={t("Unity actions")}>
            <p><Translated message="Project Unity version: {version}" values={{ version: <strong>{project.unityVersion}</strong> }} /></p>
            {effectiveLaunchOptions.exactMatchingInstallations.length > 1 ? <Select id="launch-installation" label={t("Unity installation for this launch")} onChange={setLaunchInstallationId} options={[{ label: t("Choose an installation"), value: "" }, ...effectiveLaunchOptions.exactMatchingInstallations.map((item) => ({ label: `Unity ${item.unityVersion} · ${item.architecture}`, value: item.installationId }))]} value={launchInstallationId} /> : null}
            <Button {...unityButton} type="button">{unityIcon}{unityLabel}</Button>
            {unityWindow.feedback}
            <Button disabled={!canManage || feedback.busy || projectId === undefined} onClick={() => setArgumentsOpen(true)} title={capabilityUnavailableTitle(canManage, capabilities.unityManage, t)} type="button" variant="tonal">{t("Edit launch arguments")}</Button>
            {!argumentsOpen || projectId === undefined ? null : <ProjectLaunchArgumentsDialog client={client} launchConfig={launchConfig} onChanged={onChanged} onClose={() => setArgumentsOpen(false)} projectId={projectId} />}
            <h3>{t("Migrate Project Unity version")}</h3>
            <Select id="project-unity-version" label={t("Target Unity version")} onChange={chooseTargetVersion} options={[{ label: t("Current · {version}", { version: launchOptions.projectUnityVersion }), value: "" }, ...migrationVersions.map(({ unityVersion }) => ({ label: unityVersion, value: unityVersion }))]} value={targetVersion} />
            {targetInstallations.length > 1 ? <Select id="migration-installation" label={t("Target Unity installation")} onChange={setTargetInstallationId} options={[{ label: t("Choose an installation"), value: "" }, ...targetInstallations.map((item) => ({ label: `${targetVersion} · ${item.architecture}`, value: item.installationId }))]} value={targetInstallationId} /> : null}
            <Button disabled={!canMigrate || feedback.busy || targetInstallationId.length === 0 || projectRevision === undefined} onClick={() => void planMigration()} title={capabilityUnavailableTitle(canMigrate, capabilities.projectsUnityMigration, t)} type="button" variant="tonal">{t("Review migration")}</Button>
            <PlanDialog applyDisabled={migrationPlan?.classification.supportedForApply === false} busy={feedback.busy} onApply={applyMigration} onClose={() => setMigrationPlan(undefined)} open={migrationPlan !== undefined} title={t("Review Unity migration")}>
                {migrationPlan === undefined ? null : <dl className="dialog-summary"><div><dt>{t("From")}</dt><dd>{migrationPlan.sourceUnityVersion}</dd></div><div><dt>{t("To")}</dt><dd>{migrationPlan.targetUnityVersion}</dd></div><div><dt>{t("Classification")}</dt><dd>{t(humanize(migrationPlan.classification.kind))}</dd></div><div><dt>{t("Supported")}</dt><dd>{migrationPlan.classification.supportedForApply ? t("Yes") : t("No")}</dd></div></dl>}
                {feedback.error === undefined ? null : <div role="alert"><p>{t(requestErrorMessage((feedback.error).code))}</p><code>{(feedback.error).code}</code>{feedback.error.diagnosticId === undefined ? null : <p><code>{feedback.error.diagnosticId}</code></p>}</div>}
            </PlanDialog>
            {feedback.operationId === undefined ? null : <OperationFollow client={client} operationId={feedback.operationId} onTerminal={migrationFinished} background />}
        </ActionSection>
    );
}

export function ProjectUnityWorkspaceActions({ client, launchOptions, navigate, onChanged, project, renderLaunch }: {
    client: GuiRpcClient;
    launchOptions: UnityLaunchOptionsResult;
    navigate(path: string): void;
    onChanged(): void;
    project: ProjectSnapshot;
    renderLaunch(action: ProjectUnityLaunchAction): ReactNode;
}) {
    const { t } = useTranslation();
    const canMigrate = useCapability(capabilities.projectsUnityMigration);
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const unityLaunch = useProjectUnityLaunch({ client, initialOptions: launchOptions, navigate, onLaunchSettled: onChanged, project });
    const [migrationVersion, setMigrationVersion] = useState<string>();
    const [migrationInstallationId, setMigrationInstallationId] = useState("");
    const [migrationChooserOpen, setMigrationChooserOpen] = useState(false);
    const [migrationPlan, setMigrationPlan] = useState<ProjectUnityMigrationPlan>();
    const [submitted, setSubmitted] = useState(false);
    const pending = useRef(false);
    const isCurrent = useActionScope(client);
    const projectId = project.projectId;
    const projectRevision = project.revision;
    const migrationVersions = launchOptions.migrationVersionOptions ?? [];
    const migrationInstallations = migrationVersions.find((option) => option.unityVersion === migrationVersion)?.installations ?? [];

    const planMigration = async (installationId: string) => {
        if (!canMigrate || pending.current || projectId === undefined || projectRevision === undefined) return;
        pending.current = true;
        setFeedback({ busy: true });
        try {
            const result = await client.projectPlanUnityMigration(projectId, installationId, projectRevision);
            if (!isCurrent()) return;
            setSubmitted(false);
            setMigrationChooserOpen(false);
            if (result.kind === "no_change") {
                setMigrationVersion(undefined);
                setFeedback(INITIAL_FEEDBACK);
                return;
            }
            setMigrationPlan(result.plan);
            setFeedback({ busy: false });
        } catch (caught: unknown) {
            if (!isCurrent()) return;
            setMigrationVersion(undefined);
            setFeedback({ busy: false, error: safeError(caught) });
        } finally { pending.current = false; }
    };
    const chooseMigrationVersion = (version: string) => {
        const option = migrationVersions.find((item) => item.unityVersion === version);
        setMigrationVersion(option?.unityVersion);
        setFeedback(INITIAL_FEEDBACK);
        setMigrationPlan(undefined);
        if (option === undefined) return;
        if (option.defaultInstallationId !== undefined) {
            void planMigration(option.defaultInstallationId);
            return;
        }
        setMigrationInstallationId("");
        setMigrationChooserOpen(true);
    };
    const applyMigration = async () => {
        if (!canMigrate || pending.current || submitted || migrationPlan === undefined || !migrationPlan.classification.supportedForApply) return;
        pending.current = true; setSubmitted(true);
        setFeedback({ busy: true });
        try {
            const result = await client.projectApplyUnityMigration(migrationPlan.planId);
            if (!isCurrent()) return;
            setMigrationPlan(undefined);
            navigate(`/operations/${result.operationId}`);
        } catch (caught: unknown) {
            if (isCurrent()) setFeedback({ busy: false, error: safeError(caught) });
        } finally { pending.current = false; }
    };

    return (
        <>
            <Select
                className="project-unity-version"
                disabled={!canMigrate || feedback.busy || projectRevision === undefined}
                label={t("Unity version")}
                onChange={chooseMigrationVersion}
                options={[
                    { label: `Unity ${launchOptions.projectUnityVersion}`, value: launchOptions.projectUnityVersion },
                    ...migrationVersions.map(({ unityVersion }) => ({ label: `Unity ${unityVersion}`, value: unityVersion }))
                ]}
                value={migrationVersion ?? launchOptions.projectUnityVersion}
            />
            {renderLaunch({ ...unityLaunch.action, disabled: unityLaunch.action.disabled || feedback.busy })}
            {unityLaunch.dialogs}
            <ModalDialog dismissible={!feedback.busy} onClose={() => { setMigrationChooserOpen(false); setMigrationVersion(undefined); setFeedback(INITIAL_FEEDBACK); }} open={migrationChooserOpen} title={t("Choose Unity {version} installation", { version: migrationVersion ?? "" })}>
                <Select label={t("Target Unity installation")} onChange={setMigrationInstallationId} options={[{ label: t("Choose an installation"), value: "" }, ...migrationInstallations.map((item) => ({ label: `${migrationVersion} · ${item.architecture}`, value: item.installationId }))]} value={migrationInstallationId} />
                {feedback.error === undefined ? null : <div role="alert"><p>{t(requestErrorMessage((feedback.error).code))}</p><code>{(feedback.error).code}</code>{feedback.error.diagnosticId === undefined ? null : <p><code>{feedback.error.diagnosticId}</code></p>}</div>}
                <div className="dialog-actions">
                    <DialogCloseButton variant="tonal">{t("Cancel")}</DialogCloseButton>
                    <Button disabled={migrationInstallationId.length === 0 || feedback.busy} onClick={() => void planMigration(migrationInstallationId)} type="button">{t("Review migration")}</Button>
                </div>
            </ModalDialog>
            <PlanDialog applyDisabled={submitted || migrationPlan?.classification.supportedForApply === false} busy={feedback.busy} onApply={applyMigration} onClose={() => { setMigrationPlan(undefined); setMigrationVersion(undefined); setFeedback(INITIAL_FEEDBACK); }} open={migrationPlan !== undefined} title={t("Review Unity migration")}>
                {migrationPlan === undefined ? null : <dl className="dialog-summary"><div><dt>{t("From")}</dt><dd>{migrationPlan.sourceUnityVersion}</dd></div><div><dt>{t("To")}</dt><dd>{migrationPlan.targetUnityVersion}</dd></div><div><dt>{t("Classification")}</dt><dd>{t(humanize(migrationPlan.classification.kind))}</dd></div><div><dt>{t("Supported")}</dt><dd>{migrationPlan.classification.supportedForApply ? t("Yes") : t("No")}</dd></div></dl>}
                {feedback.error === undefined ? null : <div role="alert"><p>{t(requestErrorMessage((feedback.error).code))}</p><code>{(feedback.error).code}</code>{feedback.error.diagnosticId === undefined ? null : <p><code>{feedback.error.diagnosticId}</code></p>}{submitted && operationRequestOutcomeUnknown(feedback.error) ? t("The result of this request is unknown. Check Tasks and refresh before reviewing it again.") : undefined}</div>}
            </PlanDialog>

        </>
    );
}

export function useActionScope(client: GuiRpcClient, active = true) {
    const holder = useRef({ client, active: true });
    if (holder.current.client !== client) {
        holder.current.active = false;
        holder.current = { client, active: true };
    }
    const scope = holder.current;
    const visible = useRef(active);
    visible.current = active;
    useEffect(() => { scope.active = true; return () => { scope.active = false; }; }, [scope]);
    return () => scope.active && holder.current === scope && visible.current;
}

export function BackupCreatePanel({ client, navigate, onAccepted, project }: { client: GuiRpcClient; navigate(path: string): void; onAccepted(result: BackupCreateResult): void; project: ProjectSnapshot }) {
    const { t } = useTranslation();
    const available = useCapability(capabilities.backupsCreate);
    const [options, setOptions] = useState<OfficialSettings["backups"]>();
    const [accepted, setAccepted] = useState<BackupCreateResult>();
    const [readGeneration, setReadGeneration] = useState(0);
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const showLoading = useDelayedPending(options === undefined && feedback.error === undefined);
    const unknown = operationRequestOutcomeUnknown(feedback.error);
    const pending = useRef(false);
    const isCurrent = useActionScope(client);
    useUtilityDialogBusy(feedback.busy);
    useEffect(() => {
        let active = true;
        setOptions(undefined);
        setFeedback(INITIAL_FEEDBACK);
        void client.settingsGet().then((snapshot) => {
            if (active) setOptions(snapshot.settings.backups);
        }).catch((caught: unknown) => { if (active) setFeedback({ busy: false, error: safeError(caught) }); });
        return () => { active = false; };
    }, [client, readGeneration]);
    const create = async () => {
        if (!available || pending.current || unknown || options === undefined || project.projectId === undefined || project.revision === undefined) return;
        pending.current = true; setFeedback({ busy: true });
        try {
            const result = await client.backupCreate(project.projectId, project.revision, options.compressionMode, !options.includeVpmPackages, options.includeLibrary);
            if (!isCurrent()) return;
            setFeedback(INITIAL_FEEDBACK); setAccepted(result); onAccepted(result);
        }
        catch (caught: unknown) { if (isCurrent()) setFeedback({ busy: false, error: safeError(caught) }); }
        finally { pending.current = false; }
    };
    if (accepted !== undefined) return <OperationFollow client={client} operationId={accepted.operationId}>
        {(operation, error) => <>
            <OperationProgress error={error} operation={operation} title={t("Create backup")} />
            <div className="dialog-actions">
                <DialogCloseButton variant="text">{t("Close")}</DialogCloseButton>
                <Button onClick={() => navigate(`/operations/${accepted.operationId}`)} type="button" variant="text">{t("View operation")}</Button>
                {operation?.state !== "succeeded" ? null : <Button onClick={() => navigate(`/backups/${accepted.backupId}`)} type="button">{t("View backup")}</Button>}
            </div>
        </>}
    </OperationFollow>;
    return <ActionSection title={t("Create backup")}>
        <p><Translated message="Back up {name}." values={{ name: <strong>{project.displayName ?? t("Unity project")}</strong> }} /></p>
        {options === undefined ? showLoading ? <LoadingState title={t("Loading backup defaults…")} /> : null : <>
            <Select disabled={feedback.busy || unknown} id="backup-compression" label={t("Compression")}
                onChange={(compressionMode) => setOptions({ ...options, compressionMode: compressionMode as typeof options.compressionMode })}
                options={backupCompressionOptions(t)} value={options.compressionMode} />
            <Checkbox disabled={feedback.busy || unknown} checked={options.includeVpmPackages} label={t("Keep packages in backups")} onChange={(includeVpmPackages) => setOptions({ ...options, includeVpmPackages })} />
            <Checkbox disabled={feedback.busy || unknown} checked={options.includeLibrary} label={t("Keep Library cache in backups")} onChange={(includeLibrary) => setOptions({ ...options, includeLibrary })} />
        </>}
        <div className="dialog-actions"><UtilityDialogCancel disabled={feedback.busy} label={options !== undefined && unknown ? t("Close") : undefined} />
            <Button disabled={!available || feedback.busy || unknown || options === undefined || project.revision === undefined} onClick={() => void create()} type="button">{t("Create backup")}</Button>
        </div>
        {feedback.error === undefined ? null : options === undefined ? <div role="alert"><strong>{t("Backup defaults could not be loaded.")}</strong><p>{t(requestErrorMessage((feedback.error).code))}</p><code>{(feedback.error).code}</code>{feedback.error.diagnosticId === undefined ? null : <p><code>{feedback.error.diagnosticId}</code></p>}
            <Button onClick={() => setReadGeneration((value) => value + 1)} type="button" variant="text">{t("Retry")}</Button>
        </div> : <div role="alert"><p>{t(requestErrorMessage((feedback.error).code))}</p><code>{(feedback.error).code}</code>{feedback.error.diagnosticId === undefined ? null : <p><code>{feedback.error.diagnosticId}</code></p>}{unknown ? t("The result of this request is unknown. Check Tasks and refresh before reviewing it again.") : undefined}</div>}
    </ActionSection>;
}

export function OperationActions({ client, operation, onChanged }: ActionProps & { operation: Operation }) {
    const { t } = useTranslation();
    const available = useCapability(capabilities.operations);
    const [confirm, setConfirm] = useState(false);
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const pending = useRef(false);
    const isCurrent = useActionScope(client);
    const terminal = isTerminal(operation.state);
    const cancel = async () => {
        if (!available || terminal || operation.state === "cancelling" || pending.current) return;
        pending.current = true; setFeedback({ busy: true });
        try {
            await client.operationCancel(operation.operationId, operation.revision);
            if (!isCurrent()) return;
            setConfirm(false); setFeedback({ busy: false }); onChanged?.();
        } catch (caught: unknown) {
            if (isCurrent()) setFeedback({ busy: false, error: safeError(caught) });
        } finally { pending.current = false; }
    };
    return <ActionSection title={t("Operation control")}>
        <Button className="material-button--danger" disabled={!available || terminal || operation.state === "cancelling" || feedback.busy} onClick={() => { setFeedback(INITIAL_FEEDBACK); setConfirm(true); }} title={capabilityUnavailableTitle(available, capabilities.operations, t)} type="button" variant="text">{t("Cancel operation")}</Button>
        <MaterialDialog dismissible={!feedback.busy} onClose={() => { setConfirm(false); setFeedback(INITIAL_FEEDBACK); }} open={confirm} title={t("Request cancellation?")}>
            <p>{t("Cancellation is cooperative. The operation remains authoritative until the daemon reports a terminal state.")}</p>
            {feedback.error === undefined ? null : <div role="alert"><p>{t(requestErrorMessage((feedback.error).code))}</p><code>{(feedback.error).code}</code>{feedback.error.diagnosticId === undefined ? null : <p><code>{feedback.error.diagnosticId}</code></p>}</div>}
            <div className="dialog-actions"><DialogCloseButton disabled={feedback.busy} variant="text">{t("Back")}</DialogCloseButton><Button className="material-button--danger" disabled={!available || feedback.busy || terminal || operation.state === "cancelling"} onClick={() => void cancel()} type="button">{t("Confirm")}</Button></div>
        </MaterialDialog>
    </ActionSection>;
}

export function ExtensionInstallPanel({ client, onChanged, onAccepted }: ActionProps & { onAccepted?(operationId: string): void }) {
    const { t } = useTranslation();
    const available = useCapability(capabilities.extensionsLifecycle);
    const [path, setPath] = useState("");
    const [approvePublisher, setApprovePublisher] = useState(false);
    const [plan, setPlan] = useState<ExtensionPlan>();
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const [submitted, setSubmitted] = useState(false);
    useUtilityDialogBusy(feedback.busy);
    const current = useActionScope(client);
    const pending = useRef(false);
    const run = async (apply: boolean) => {
        if (!available || pending.current || submitted || (apply ? plan === undefined || !hasPermissionRequests(plan) : path.trim().length === 0)) return;
        pending.current = true;
        setFeedback({ busy: true });

        try {
            if (apply && plan !== undefined) {
                setSubmitted(true);
                const result = await client.extensionApplyInstall(plan.planId);
                if (!current()) return;
                setFeedback({ busy: false, operationId: result.operationId });
                onAccepted?.(result.operationId);
            } else {
                // Core accepts new installs only; zero binds the plan to expected absence.
                const result = await client.extensionPlanInstall(path, 0, approvePublisher ? "approve_for_extension" : "none");
                if (!current()) return;
                setPlan(result.plan);
                setFeedback({ busy: false });
            }
        } catch (caught: unknown) {
            if (!current()) return;
            const error = safeError(caught);
            if (!operationRequestOutcomeUnknown(error)) setSubmitted(false);
            setFeedback({ busy: false, error });
        }
        finally { pending.current = false; }
    };
    const chooseFile = async () => {
        if (pending.current) return;
        pending.current = true;
        setFeedback({ busy: true });
        try {
            const selected = await client.selectFile("extensionPackage");
            if (current()) {
                if (selected !== undefined) setPath(selected);
                setFeedback(INITIAL_FEEDBACK);
            }
        } catch (caught) { if (current()) setFeedback({ busy: false, error: safeError(caught) }); }
        finally { pending.current = false; }
    };
    return <section aria-label={plan === undefined ? t("Extension install input") : t("Review extension install")} aria-busy={feedback.busy}>
        {feedback.operationId !== undefined ? <div className="dialog-actions"><DialogCloseButton variant="text">{t("Close")}</DialogCloseButton></div> : plan === undefined ? <form onSubmit={(event) => { event.preventDefault(); void run(false); }}>
            <div className="project-target-picker">
                <PathField disabled={feedback.busy} id="extension-package" label={t("Extension package")} maxLength={32768} onInput={(value) => { setPath(value); setFeedback(INITIAL_FEEDBACK); }} required value={path} />
                <Button disabled={feedback.busy} onClick={() => void chooseFile()} type="button" variant="tonal">{t("Select")}</Button>
            </div>
            <Checkbox disabled={feedback.busy} checked={approvePublisher} label={t("Approve this publisher for this extension")} onChange={(value) => { setApprovePublisher(value); setFeedback(INITIAL_FEEDBACK); }} />
            <div className="dialog-actions"><UtilityDialogCancel disabled={feedback.busy} /><Button disabled={!available || feedback.busy || path.trim().length === 0} title={capabilityUnavailableTitle(available, capabilities.extensionsLifecycle, t)} type="submit">{t("Create install plan")}</Button></div>
        </form> : <>
            <h2>{t("Review extension install")}</h2>
            <dl className="dialog-summary"><div><dt>{t("Extension")}</dt><dd>{plan.extensionId}</dd></div><div><dt>{t("Version")}</dt><dd>{plan.version}</dd></div><div><dt>{t("Publisher")}</dt><dd><code>{plan.publisherFingerprint}</code></dd></div><div><dt>{t("Trust")}</dt><dd>{t(humanize(plan.trustDecision))}</dd></div></dl>
            {hasPermissionRequests(plan) ? <ExtensionPermissionRequests required={plan.requiredPermissions} optional={plan.optionalPermissions} /> : <div>{t("Permission details are unavailable from this Core.")}</div>}
            <div className="dialog-actions"><UtilityDialogCancel disabled={feedback.busy} /><Button disabled={feedback.busy || submitted} onClick={() => { setPlan(undefined); setFeedback(INITIAL_FEEDBACK); }} type="button" variant="text">{t("Discard plan")}</Button><Button disabled={!available || feedback.busy || submitted || !hasPermissionRequests(plan)} onClick={() => void run(true)} type="button">{t("Apply reviewed plan")}</Button></div>
        </>}
        {submitted && operationRequestOutcomeUnknown(feedback.error) ? <div role="alert"><p>{t(requestErrorMessage((feedback.error!).code))}</p><code>{(feedback.error!).code}</code>{feedback.error!.diagnosticId === undefined ? null : <p><code>{feedback.error!.diagnosticId}</code></p>}{t("The result of this request is unknown. Check Tasks and refresh before reviewing it again.")}</div> : <MutationFeedback client={client} feedback={feedback} onOperationTerminal={onAccepted === undefined ? () => onChanged?.() : undefined} />}
    </section>;
}

export function ExtensionActions({ client, extension, onChanged, onAccepted, operationActive = false }: ActionProps & { extension: ExtensionRecord; onAccepted?(operationId: string): void; operationActive?: boolean }) {
    const { t } = useTranslation();
    const canManageLifecycle = useCapability(capabilities.extensionsLifecycle);
    const [mode, setMode] = useState<"none" | "permissions" | "uninstall">("none");
    const [plan, setPlan] = useState<ExtensionPlan>();
    const [deleteData, setDeleteData] = useState(false);
    const [permissionsBusy, setPermissionsBusy] = useState(false);
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const [submitted, setSubmitted] = useState(false);
    const current = useActionScope(client);
    const pending = useRef(false);
    const close = () => { if (!pending.current && !permissionsBusy) { setMode("none"); setPlan(undefined); setFeedback(INITIAL_FEEDBACK); } };
    const open = (next: "permissions" | "uninstall") => { if (!pending.current && !permissionsBusy && !operationActive) { setMode(next); setPlan(undefined); setSubmitted(false); setDeleteData(false); setFeedback(INITIAL_FEEDBACK); } };
    const run = async (kind: "enable" | "disable" | "plan" | "apply") => {
        if (pending.current || operationActive || !canManageLifecycle) return;
        if (kind === "apply" && (plan === undefined || submitted)) return;
        pending.current = true;
        setFeedback({ busy: true });
        try {
            if (kind === "plan") {
                const result = await client.extensionPlanUninstall(extension.extensionId, extension.revision, deleteData ? "delete_data" : "retain_data");
                if (!current()) return;
                setPlan(result.plan);
                setFeedback({ busy: false });
            } else if (kind === "apply" && plan !== undefined) {
                setSubmitted(true);
                const result = await client.extensionApplyUninstall(plan.planId);
                if (!current()) return;
                setPlan(undefined);
                setFeedback({ busy: false, operationId: result.operationId });
                onAccepted?.(result.operationId);
            } else if (kind === "enable" || kind === "disable") {
                await (kind === "enable" ? client.extensionEnable(extension.extensionId, extension.revision) : client.extensionDisable(extension.extensionId, extension.revision));
                if (!current()) return;
                setFeedback({ busy: false });
                onChanged?.();
            }
        } catch (caught: unknown) { if (current()) { const error = safeError(caught); if (!operationRequestOutcomeUnknown(error)) setSubmitted(false); setFeedback({ busy: false, error }); } }
        finally { pending.current = false; }
    };
    const blocked = extension.desiredState === "uninstalling" || operationActive;
    const quarantined = extension.quarantineState === "quarantined";
    return <ActionSection title={t("Extension management")}>
        <div className="action-row"><Button disabled={!canManageLifecycle || feedback.busy || blocked || (extension.desiredState === "enabled" && !quarantined)} onClick={() => void run("enable")} type="button" variant="tonal">{quarantined ? t("Recover extension") : t("Enable")}</Button><Button disabled={!canManageLifecycle || feedback.busy || blocked || extension.desiredState !== "enabled"} onClick={() => void run("disable")} type="button" variant="tonal">{t("Disable")}</Button><Button disabled={feedback.busy || blocked} onClick={() => open("permissions")} type="button" variant="text">{t("Manage permissions")}</Button><Button className="material-button--danger" disabled={!canManageLifecycle || feedback.busy || blocked} onClick={() => open("uninstall")} type="button" variant="text">{t("Uninstall extension")}</Button></div>
        <MaterialDialog dismissible={!feedback.busy && !permissionsBusy} open={mode !== "none"} onClose={close} title={mode === "permissions" ? t("Extension permissions") : plan === undefined ? t("Uninstall extension") : t("Review extension uninstall")}>
            <p><strong>{extension.extensionId}</strong></p>
            {mode === "permissions" ? <ExtensionPermissions client={client} extensionId={extension.extensionId} onBusyChange={setPermissionsBusy} onChanged={onChanged} /> : feedback.operationId !== undefined ? <div className="dialog-actions"><DialogCloseButton variant="text">{t("Close")}</DialogCloseButton></div> : plan === undefined ? <>
                <p>{t("Review the removal before applying it.")}</p>
                <Checkbox disabled={feedback.busy} checked={deleteData} label={t("Delete extension-owned data during uninstall")} onChange={setDeleteData} />
                <div className="dialog-actions"><DialogCloseButton disabled={feedback.busy} variant="text">{t("Cancel")}</DialogCloseButton><Button disabled={!canManageLifecycle || feedback.busy} onClick={() => void run("plan")} type="button">{t("Create uninstall plan")}</Button></div>
            </> : <>
                <dl className="dialog-summary"><div><dt>{t("Extension")}</dt><dd>{plan.extensionId}</dd></div><div><dt>{t("Version")}</dt><dd>{plan.version}</dd></div><div><dt>{t("Data")}</dt><dd>{t(humanize(plan.dataDisposition))}</dd></div><div><dt>{t("Publisher")}</dt><dd><code>{plan.publisherFingerprint}</code></dd></div></dl>
                <div className="dialog-actions"><DialogCloseButton disabled={feedback.busy || submitted} variant="text">{t("Discard plan")}</DialogCloseButton><Button disabled={!canManageLifecycle || feedback.busy || submitted} onClick={() => void run("apply")} type="button">{t("Apply reviewed plan")}</Button></div>
            </>}
            {mode === "none" ? null : submitted && operationRequestOutcomeUnknown(feedback.error) ? <><div role="alert"><p>{t(requestErrorMessage((feedback.error!).code))}</p><code>{(feedback.error!).code}</code>{feedback.error!.diagnosticId === undefined ? null : <p><code>{feedback.error!.diagnosticId}</code></p>}{t("The result of this request is unknown. Check Tasks and refresh before reviewing it again.")}</div><div className="dialog-actions"><DialogCloseButton variant="text">{t("Close")}</DialogCloseButton></div></> : <MutationFeedback client={client} feedback={feedback} onOperationTerminal={onAccepted === undefined ? () => onChanged?.() : undefined} />}
        </MaterialDialog>
    </ActionSection>;
}

function ActionSection({ children, title }: { children: ReactNode; title: string }) {
    return <section className="action-section"><h2>{title}</h2>{children}</section>;
}

function PlanDialog({ applyDisabled = false, busy, children, onApply, onClose, open, title }: { applyDisabled?: boolean; busy: boolean; children: ReactNode; onApply(): Promise<void>; onClose(): void; open: boolean; title: string }) {
    const { t } = useTranslation();
    return <ModalDialog dismissible={!busy} open={open} title={title} onClose={onClose}>{children}<p className="risk-summary">{t("The daemon will revalidate this frozen plan. A stale plan fails instead of being silently replaced.")}</p><div className="dialog-actions"><DialogCloseButton disabled={busy} variant="tonal">{t("Discard plan")}</DialogCloseButton><Button data-dialog-initial-focus disabled={busy || applyDisabled} onClick={() => void onApply()} type="button">{t("Apply reviewed plan")}</Button></div></ModalDialog>;
}

function ModalDialog({ children, dismissible = true, onClose, open, title }: { children: ReactNode; dismissible?: boolean; onClose(): void; open: boolean; title: string }) {
    return <MaterialDialog dismissible={dismissible} onClose={onClose} open={open} title={title}>{children}</MaterialDialog>;
}

function MutationFeedback({ client, errorTitle, feedback, onOperationTerminal }: { client: GuiRpcClient; errorTitle?: string; feedback: FeedbackState; onOperationTerminal?(operation: Operation): void }) {
    const { t } = useTranslation();
    if (feedback.error !== undefined) return <div role="alert"><strong>{errorTitle}</strong><p>{t(requestErrorMessage((feedback.error).code))}</p><code>{(feedback.error).code}</code>{feedback.error.diagnosticId === undefined ? null : <p><code>{feedback.error.diagnosticId}</code></p>}</div>;
    if (feedback.operationId !== undefined) return <OperationFollow client={client} onTerminal={onOperationTerminal} operationId={feedback.operationId} />;
    return null;
}

interface AcceptedOperation {
    operationId: string;
    client: GuiRpcClient;
    onSucceeded?(operation: Operation): void | Promise<void>;
    operation?: Operation;
    error?: RpcError;
    progressError?: RpcError;
}

export type AcceptedOperations = ReturnType<typeof useAcceptedOperations>;

// These are only this page's explicitly accepted IDs. Core remains the task history owner.
export function useAcceptedOperations(client: GuiRpcClient) {
    const [accepted, setAccepted] = useState<AcceptedOperation[]>([]);
    const owner = useRef({ client, active: false, seen: new Set<string>(), finished: new Set<string>() });
    if (owner.current.client !== client) owner.current = { client, active: false, seen: new Set(), finished: new Set() };
    const scope = owner.current;
    useEffect(() => {
        scope.active = true;
        setAccepted([]);
        return () => { scope.active = false; };
    }, [scope]);
    const accept = (operationId: string, onSucceeded?: AcceptedOperation["onSucceeded"]) => {
        if (owner.current !== scope || !scope.active || scope.seen.has(operationId)) return;
        if (!canonicalUuid(operationId)) throw { code: "internal_error" };
        scope.seen.add(operationId);
        setAccepted((current) => [...current.filter((item) => item.client === client), { operationId, client, onSucceeded }]);
    };
    const snapshot = (operationId: string) => accepted.find((item) => item.client === client && item.operationId === operationId);
    const follow = (onSucceeded?: () => void) => accepted.filter((item) => item.client === client).map((item) => {
        const update = (operation: Operation) => {
            if (owner.current !== scope || !scope.active || operation.operationId !== item.operationId) return;
            setAccepted((current) => current.map((entry) => entry === item || entry.operationId === item.operationId ? { ...entry, operation } : entry));
        };
        const finish = (operation: Operation) => {
            if (owner.current !== scope || !scope.active || operation.operationId !== item.operationId || scope.finished.has(item.operationId)) return;
            scope.finished.add(item.operationId);
            setAccepted((current) => {
                const next = current.map((entry) => entry.operationId === item.operationId ? { ...entry, operation } : entry);
                const latest = [...next].reverse().find((entry) => scope.finished.has(entry.operationId));
                return next.filter((entry) => !scope.finished.has(entry.operationId) || entry === latest);
            });
            if (operation.state !== "succeeded") return;
            onSucceeded?.();
            void Promise.resolve().then(() => {
                if (owner.current === scope && scope.active) return item.onSucceeded?.(operation);
            }).catch((caught: unknown) => {
                if (owner.current === scope && scope.active) setAccepted((current) => current.map((entry) => entry.operationId === item.operationId ? { ...entry, error: safeError(caught) } : entry));
            });
        };
        return <OperationFollow background client={client} key={item.operationId} onReadError={(progressError) => {
            if (owner.current !== scope || !scope.active) return;
            setAccepted((current) => current.map((entry) => entry.operationId === item.operationId && entry.progressError?.code !== progressError?.code ? { ...entry, progressError } : entry));
        }} onSnapshot={update} onTerminal={finish} operationId={item.operationId} />;
    });
    return { accept, snapshot, follow };
}

export function operationResultProjectId(operation: Operation | undefined): string | undefined {
    if (operation?.state !== "succeeded" || typeof operation.result !== "object" || operation.result === null) return undefined;
    const projectId = (operation.result as { projectId?: unknown }).projectId;
    return canonicalUuid(projectId) ? projectId : undefined;
}

function canonicalUuid(value: unknown): value is string { return typeof value === "string" && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(value); }

export function operationRequestOutcomeUnknown(error: RpcError | undefined): boolean {
    return error?.code === "daemon_unavailable" || error?.code === "internal_error";
}

export function AcceptedOperationProgress({ operationId, operations }: { operationId: string; operations: AcceptedOperations }) {
    const { t } = useTranslation();
    const events = useCoreEvents();
    const item = operations.snapshot(operationId);
    return <><OperationProgress error={item?.progressError} operation={item?.operation} />
        {item?.progressError === undefined || events === undefined ? null : <Button type="button" variant="text" onClick={() => events.invalidate(`operation:${operationId}`, true)}>{t("Retry")}</Button>}
        {item?.error === undefined ? null : <div role="alert"><p>{t(requestErrorMessage((item.error).code))}</p><code>{(item.error).code}</code>{item.error.diagnosticId === undefined ? null : <p><code>{item.error.diagnosticId}</code></p>}</div>}</>;
}

export function OperationFollow({ background = false, children, client, initialOperation, onReadError, onSnapshot, onTerminal, operationId, title = "Operation" }: {
    background?: boolean;
    children?(operation: Operation | undefined, error: RpcError | undefined): ReactNode;
    client: Pick<GuiRpcClient, "operationGet">;
    initialOperation?: Operation;
    onReadError?(error: RpcError | undefined): void;
    onSnapshot?(operation: Operation): void;
    onTerminal?(operation: Operation): void;
    operationId: string;
    title?: string;
}) {
    const { t } = useTranslation();
    const owner = useRef({ client, operationId, initialOperation });
    if (owner.current.client !== client || owner.current.operationId !== operationId || owner.current.initialOperation !== initialOperation) owner.current = { client, operationId, initialOperation };
    const scope = owner.current;
    const terminalNotice = useRef({ scope, consumed: false });
    if (terminalNotice.current.scope !== scope) terminalNotice.current = { scope, consumed: false };
    const [snapshot, setSnapshot] = useState<{ scope: typeof scope; operation: Operation }>();
    const [error, setError] = useState<{ scope: typeof scope; error: RpcError }>();
    const callbacks = useRef({ onReadError, onSnapshot, onTerminal });
    callbacks.current = { onReadError, onSnapshot, onTerminal };
    const events = useCoreEvents();
    const [retryGeneration, setRetryGeneration] = useState(0);
    useEffect(() => {
        let active = true;
        const initial = initialOperation?.operationId === operationId ? initialOperation : undefined;
        setSnapshot(initial === undefined ? undefined : { scope, operation: initial });
        setError(undefined);
        const accept = (next: Operation | undefined, failure?: RpcError) => {
            if (!active || owner.current !== scope) return;
            if (failure) {
                setError({ scope, error: failure }); callbacks.current.onReadError?.(failure);
                return;
            }
            if (!next) return;
            setSnapshot((previous) => previous?.scope === scope && previous.operation.revision > next.revision ? previous : { scope, operation: next });
            setError(undefined); callbacks.current.onReadError?.(undefined); callbacks.current.onSnapshot?.(next);
            if (isTerminal(next.state) && callbacks.current.onTerminal && !terminalNotice.current.consumed && !(initial && isTerminal(initial.state))) {
                terminalNotice.current.consumed = true;
                if (events?.consumeTerminal(operationId) ?? true) callbacks.current.onTerminal(next);
            }
        };
        if (events) {
            const update = () => {
                const entry = events.operationSnapshot(operationId);
                accept(entry.value, entry.error);
            };
            const stop = events.observeOperation(operationId, update, initial);
            update();
            return () => { active = false; stop(); };
        }
        // A locally rendered observer has one initial read; applications supply the shared event entrance.
        if (initial === undefined) void client.operationGet(operationId).then((next) => {
            if (next.operationId !== operationId) throw { code: "internal_error" };
            accept(next);
        }).catch((caught: unknown) => accept(undefined, safeError(caught)));
        return () => { active = false; };
    }, [client, events, initialOperation, operationId, scope, retryGeneration]);
    if (background) return null;
    const visible = snapshot?.scope === scope ? snapshot.operation : initialOperation?.operationId === operationId ? initialOperation : undefined;
    const readError = error?.scope === scope ? error.error : undefined;
    return <>
        {children === undefined ? <OperationProgress error={readError} operation={visible} title={title} /> : children(visible, readError)}
        {readError === undefined ? null : <Button type="button" variant="text" onClick={() => {
            if (events) events.invalidate(`operation:${operationId}`, true); else setRetryGeneration((value) => value + 1);
        }}>{t("Retry")}</Button>}
    </>;
}

export function OperationProgress({ error, operation, title = "Operation" }: { error?: RpcError; operation?: Operation; title?: string }) {
    const { t } = useTranslation();
    return <>
        <section className="operation-follow" aria-live="polite" aria-atomic="true" role="status"><h3>{t(title)}</h3>{operation === undefined ? <p>{t("Reading progress…")}</p> : <><p><strong>{t(humanize(operation.state))}</strong>{operation.progress?.phase === undefined ? "" : ` · ${t(humanize(operation.progress.phase))}`}</p><Progress label={t("{title} progress", { title: t(title) })} value={isTerminal(operation.state) ? 1 : undefined} /></>}{error === undefined ? null : <p>{t("Progress is temporarily unavailable. ALCOMD will keep following the same task.")}</p>}</section>
        {operation?.state !== "failed" ? null : <div role="alert"><strong>{t("Operation failed")}</strong><p>{t(requestErrorMessage(({ code: operation.errorCode ?? "internal_error", message: "", diagnosticId: operation.diagnosticId }).code))}</p><code>{operation.errorCode ?? "internal_error"}</code>{operation.diagnosticId === undefined ? null : <p><code>{operation.diagnosticId}</code></p>}</div>}
    </>;
}

function safeError(caught: unknown): RpcError {
    if (typeof caught === "object" && caught !== null && "code" in caught && typeof caught.code === "string") {
        return { code: caught.code, message: "The request could not be completed.", ...("diagnosticId" in caught && typeof caught.diagnosticId === "string" ? { diagnosticId: caught.diagnosticId } : {}) };
    }
    return { code: "internal_error", message: "The request could not be completed." };
}

function humanize(value: string): string { return value.replaceAll("_", " "); }
function packageChangeLabel(kind: string): string {
    if (kind === "install") return "Install";
    if (kind === "remove") return "Remove";
    if (kind === "replace") return "Change version";
    return "Update";
}
function packageVersionChange(fromVersion: string | null | undefined, toVersion: string | null | undefined, t: Translate): string {
    if (fromVersion != null && toVersion != null) return `${fromVersion} → ${toVersion}`;
    if (toVersion != null) return t("Version {version}", { version: toVersion });
    return t("{version} → Removed", { version: fromVersion ?? "" });
}
function packageErrorMessage(error: RpcError): string {
    if (["plan_stale", "project_revision_conflict", "resource_revision_conflict", "revision_conflict"].includes(error.code)) {
        return "This project changed while the package changes were being prepared. Refresh the package list and try again.";
    }
    if (["package_not_found", "package_source_changed", "repository_revision_conflict"].includes(error.code)) {
        return "The package information changed or is no longer available. Refresh the package list and try again.";
    }
    if (error.code === "package_not_installed") {
        return "This package is not currently installed in the project.";
    }
    if (error.code === "package_source_ambiguous") {
        return "More than one source provides this package version. Choose a source and try again.";
    }
    if (error.code === "package_intent_conflict") {
        return "The selected package actions conflict. Review the selection and try again.";
    }
    return "ALCOMD could not complete the package changes. Try again.";
}
function shortValue(value: string): string { return value.length <= 28 ? value : `${value.slice(0, 20)}…${value.slice(-8)}`; }
function isTerminal(state: string): boolean { return ["succeeded", "failed", "cancelled"].includes(state); }
