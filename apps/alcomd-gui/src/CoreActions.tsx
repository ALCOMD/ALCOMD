import { requestErrorMessage } from "./request-errors";
import { useCoreEvents } from "./core-events-ui";
import { Translated, useTranslation } from "./i18n";
import type { Translate } from "./i18n-types";
import type { ExtensionRecord, RpcError } from "@alcomd/sdk";
import { queryCandidates } from "./package-candidates";
import { useUtilityDialogBusy, useUtilityDialogClose, UtilityActionDialog, UtilityDialogCancel } from "./UtilityActionDialog";
import { useCallback, useEffect, useRef, useState, type FormEvent, type ReactNode, type RefObject } from "react";

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
    TemplateRecord,
    UnityInstallation,
    UnityLaunchOptionsResult
} from "./core-models";
import type { GuiRpcClient } from "./rpc";
import { closeIcon, moreVertIcon, starIcon } from "@alcomd/ui/icons";
import { Button, Checkbox, Dialog as MaterialDialog, Icon, IconButton, Menu, MenuItem, PathField, Progress, RefreshButton, Select, TextField, type DialogOrigin } from "./Material";
import { SplitButton } from "./SplitButton";
import { LoadingState, useDelayedPending } from "./LoadingState";
import { useProjectUnityLaunch } from "./ProjectUnityLaunch";
import { capabilities, capabilityUnavailableTitle, useCapability } from "./capabilities";
import { reviewTemplateCreation, reviewTemplateDerive, reviewTemplateImport, TemplatePlanReview, type TemplateReview } from "./TemplatePlanReview";
import { displayPath } from "./display-path";
import { useProjectDirectory } from "./use-project-directory";
import { CreateUnityVersionField, useCreateUnityVersion } from "./use-create-unity-version";
import { backupCompressionOptions } from "./backup-compression";
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

export function RegisterRepositoryPanel({ client, onChanged }: ActionProps) {
    const { t } = useTranslation();

    const close = useUtilityDialogClose();
    const current = useActionScope(client);
    const available = useCapability(capabilities.repositoriesRegistry);
    const [kind, setKind] = useState<"remote" | "local">("remote");
    const [value, setValue] = useState("");
    const [preview, setPreview] = useState<RepositorySnapshot>();
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    useUtilityDialogBusy(feedback.busy);
    const pending = useRef(false);
    const source = () => kind === "remote" ? { kind, url: value } as const : { kind, path: value } as const;
    const inspect = async (event: FormEvent) => {
        event.preventDefault();
        if (pending.current || !available) return;
        pending.current = true;
         setFeedback({ busy: true });
        try {
            const result = await client.repositoriesInspect(source());
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
            await client.repositoryRegister(preview.source);
            if (!current()) return;
            setPreview(undefined); setValue("");
            setFeedback(INITIAL_FEEDBACK);  close(); onChanged?.();
        } catch (caught: unknown) { if (current()) setFeedback({ busy: false, error: safeError(caught) }); }
        finally { pending.current = false; }
    };
    const fieldError = preview === undefined && feedback.error !== undefined && ["repository_source_invalid", "repository_document_invalid", "invalid_params"].includes(feedback.error.code) ? feedback.error : undefined;
    const error = feedback.error === undefined || fieldError !== undefined ? null : <div role="alert"><p>{t(requestErrorMessage((feedback.error).code))}</p><code>{(feedback.error).code}</code>{feedback.error.diagnosticId === undefined ? null : <p><code>{feedback.error.diagnosticId}</code></p>}</div>;
    return <ActionSection title={t("Add repository")}>
        {preview === undefined ? <form onSubmit={(event) => void inspect(event)}>
            <Select disabled={feedback.busy} id="repository-kind" label={t("Source type")} onChange={(next) => { setKind(next as "remote" | "local"); setFeedback(INITIAL_FEEDBACK); }} options={[{ label: t("Remote URL"), value: "remote" }, { label: t("Local manifest"), value: "local" }]} value={kind} />
            <TextField disabled={feedback.busy} error={fieldError !== undefined} errorText={fieldError === undefined ? undefined : t("Check the repository URL or manifest path.")} id="repository-source" label={kind === "remote" ? t("Repository URL") : t("Local manifest path")} maxLength={2048} onInput={(next) => { setValue(next); setFeedback(INITIAL_FEEDBACK); }} required type={kind === "remote" ? "url" : "text"} value={value} />
            {error}
            <div className="dialog-actions"><UtilityDialogCancel disabled={feedback.busy} /><Button disabled={!available || feedback.busy || value.trim().length === 0} title={capabilityUnavailableTitle(available, capabilities.repositoriesRegistry, t)} type="submit">{t("Review repository")}</Button></div>
        </form> : <>
            <dl className="dialog-summary"><div><dt>{t("Name")}</dt><dd>{preview.name ?? t("Unnamed repository")}</dd></div><div><dt>{t("Source")}</dt><dd>{preview.source.kind === "remote" ? preview.source.url : displayPath(preview.source.path)}</dd></div></dl>
            {preview.issues.length === 0 ? null : <div>{t("This repository has {count} reported issues. Registration will validate the source again.", { count: preview.issues.length })}</div>}
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
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const pending = useRef(false);
    const ready = repository.repositoryId !== undefined && repository.revision !== undefined;
    const run = async (remove: boolean) => {
        if (!ready || !available || pending.current) return;
        pending.current = true;  setFeedback({ busy: true });
        try {
            if (remove) await client.repositoryUnregister(repository.repositoryId!, repository.revision!);
            else await client.repositoryRefresh(repository.repositoryId!, repository.revision!);
            if (!current()) return;
            setFeedback(INITIAL_FEEDBACK);
            setConfirmRemove(false); onChanged?.();
        } catch (caught: unknown) {
            if (!current()) return;
            const error = safeError(caught);
            setFeedback({ busy: false, error: remove ? error : undefined });
            if (!remove && error.code === "revision_conflict") onChanged?.();
        }
        finally { pending.current = false; }
    };
    return <div className="repository-row-actions">
        <RefreshButton disabled={!available || !ready || feedback.busy} label={t("Refresh repository")} onClick={() => void run(false)} refreshing={feedback.busy && !confirmRemove} refreshingLabel={t("Refreshing repository")} />
        <IconButton disabled={!available || !ready || feedback.busy} label={t("Remove")} onClick={() => {  setFeedback(INITIAL_FEEDBACK); setConfirmRemove(true); }} title={t("Remove repository")} type="button"><Icon asset={closeIcon} /></IconButton>
        <MaterialDialog open={confirmRemove} title={t("Remove this repository?")} dismissible={!feedback.busy} onClose={() => { setConfirmRemove(false); setFeedback(INITIAL_FEEDBACK); }}>
            <p><Translated message="Remove {name} from your sources?" values={{ name: <strong>{repository.name ?? repository.declaredId ?? t("this repository")}</strong> }} /></p><p>{t("Packages already installed in projects stay installed.")}</p>
            <MutationFeedback client={client} feedback={feedback} />
            <div className="dialog-actions"><Button disabled={feedback.busy} onClick={() => { setConfirmRemove(false); setFeedback(INITIAL_FEEDBACK); }} type="button" variant="text">{t("Cancel")}</Button><Button disabled={feedback.busy || !available} onClick={() => void run(true)} type="button">{t("Remove repository")}</Button></div>
        </MaterialDialog>
    </div>;
}

export interface PackageActionSelection {
    action: "install" | "remove" | "upgrade" | "downgrade" | "resolve" | "reinstall" | "reinstall-all" | "bulk-reinstall" | "bulk-remove" | "bulk-candidates";
    key: number;
    origin: DialogOrigin;
    packageId: string;
    packageIds?: string[];
    source?: PackageSourceSelector;
    sources?: Array<{ packageId: string; source: PackageSourceSelector }>;
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
    const prepareChanges = useCallback(async (action: PackageActionSelection["action"], selectedPackageId: string, selectedVersion = "", selectedSource?: PackageSourceSelector, packageIds?: string[], sources?: Array<{ packageId: string; source: PackageSourceSelector }>, includePrerelease = false) => {
        if (revision === undefined || projectId === undefined) return;
        const requiresV2 = ((action === "install" || action === "upgrade") && selectedSource !== undefined) || action === "reinstall" || action === "reinstall-all" || action === "bulk-reinstall" || action === "bulk-remove" || action === "bulk-candidates" || action === "downgrade";
        if ((requiresV2 && !canPlanV2) || (!requiresV2 && !canPlanV1)) {
            setFeedback({ busy: false, error: { code: "capability_required", message: t("The connected daemon did not negotiate {capability}.", { capability: requiresV2 ? capabilities.packagesPlanV2 : capabilities.packagesPlanV1 }) } });
            return;
        }
        setPlan(undefined);
        setFeedback({ busy: true });
        setOperationFinished(false);
        try {
            if (selection?.candidateSnapshot && action !== "remove") {
                const ids = selection.intents?.map((intent) => intent.packageId) ?? packageIds ?? (selectedPackageId ? [selectedPackageId] : undefined);
                const evidence = await queryCandidates(client, { projectId, expectedRevision: revision, expectedSnapshot: selection.candidateSnapshot, view: { kind: "summary", ...(ids ? { packageIds: ids } : {}) }, limit: 1 });
                if (!evidence.catalogComplete) throw { code: "package_catalog_incomplete" };
            }
            let result: PackagePlan;
            if ((action === "bulk-reinstall" || action === "bulk-remove") && (packageIds === undefined || packageIds.length === 0 || packageIds.length > 256)) throw { code: "invalid_input" };
            if (action === "bulk-candidates") {
                if (!selection?.intents?.length || selection.intents.length > 256) throw { code: "invalid_input" };
                result = await client.packagePlanBulk({ projectId, expectedRevision: revision, intents: selection.intents });
            }
            else if (action === "remove") result = await client.packagePlanRemove({ projectId, expectedRevision: revision, packageId: selectedPackageId });
            else if (action === "resolve") result = await client.packagePlanResolve({ projectId, expectedRevision: revision, includePrerelease: false });
            else if (action === "reinstall-all") result = await client.packagePlanReinstall({ projectId, expectedRevision: revision, selection: { kind: "all" } });
            else if (action === "reinstall") result = await client.packagePlanReinstall({ projectId, expectedRevision: revision, selection: { kind: "packages", packageIds: [selectedPackageId] }, ...(selectedSource === undefined ? {} : { sources: [{ packageId: selectedPackageId, source: selectedSource }] }) });
            else if (action === "bulk-reinstall") result = await client.packagePlanBulk({ projectId, expectedRevision: revision, intents: (packageIds ?? []).map((packageId) => ({ kind: "reinstall" as const, packageId, ...(sources?.find((item) => item.packageId === packageId)?.source === undefined ? {} : { source: sources.find((item) => item.packageId === packageId)?.source }) })) });
            else if (action === "bulk-remove") result = await client.packagePlanBulk({ projectId, expectedRevision: revision, intents: (packageIds ?? []).map((packageId) => ({ kind: "remove" as const, packageId })) });
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
        void prepareChanges(selection.action, selection.packageId, selection.version, selection.source, selection.packageIds, selection.sources, selection.includePrerelease);
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
    const hasChanges = (plan?.changeSet.mutations.length ?? 0) > 0;
    return (
        <>
            <MaterialDialog onClose={closeChanges} open={plan !== undefined} returnFocus={selection?.origin} returnFocusReady={returnFocusReady && feedback.operationId === undefined} title={hasChanges ? t("Apply package changes?") : t("Packages are up to date")}>
                {selection?.exclusions?.length ? <div role="status"><p>{t("Not included in this update:")}</p><ul>{selection.exclusions.map((reason) => <li key={reason}>{reason}</li>)}</ul></div> : null}
                {plan === undefined ? null : hasChanges ? (
                    <div className="package-plan-review">
                        <p>{t("Review the changes ALCOMD will make to this project.")}</p>
                        <ul className="change-list">{plan.changeSet.mutations.map((mutation) => <li key={`${mutation.kind}-${mutation.packageId}`}><strong>{t(packageChangeLabel(mutation.kind))}</strong><span>{mutation.packageId}</span>{mutation.fromVersion == null && mutation.toVersion == null ? null : <small>{packageVersionChange(mutation.fromVersion, mutation.toVersion, t)}</small>}</li>)}</ul>
                        <div className="dialog-actions">
                            <Button disabled={feedback.busy} onClick={closeChanges} type="button" variant="text">{t("Cancel")}</Button>
                            <Button disabled={!canApply || feedback.busy} onClick={() => void apply()} title={capabilityUnavailableTitle(canApply, capabilities.packagesApply, t)} type="button">{t("Apply changes")}</Button>
                        </div>
                    </div>
                ) : (
                    <div className="package-plan-review"><p>{t("No package changes are required for this project.")}</p><div className="dialog-actions"><Button onClick={closeChanges} type="button">{t("Close")}</Button></div></div>
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
    return <ActionSection title={confirmRemove ? t("Remove Unity installation") : t("Manage Unity installations")}>
        {confirmRemove && remove !== undefined ? <>
            <p><Translated message="Remove {name} from the list?" values={{ name: <strong>Unity {remove.unityVersion}</strong> }} /></p>
            <p>{t("Only the ALCOMD registry entry is removed. The editor remains installed.")}</p>
            <div className="dialog-actions"><UtilityDialogCancel disabled={feedback.busy} /><Button disabled={feedback.busy} onClick={() => { setConfirmRemove(false); setFeedback(INITIAL_FEEDBACK); }} type="button" variant="text">{t("Back")}</Button><Button className="material-button--danger" disabled={!available || feedback.busy} onClick={() => void mutate("remove")} type="button">{t("Remove installation")}</Button></div>
        </> : <>
            <form onSubmit={(event) => { event.preventDefault(); void mutate("register"); }}>
                <TextField disabled={feedback.busy} id="unity-executable" label={t("Unity executable")} maxLength={1024} onInput={(value) => { setPath(value); setFeedback(INITIAL_FEEDBACK); }} required value={path} />
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
    const close = () => { if (!pending.current) setOpen(false); };
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
                <Button disabled={feedback.busy} onClick={close} type="button" variant="text">{t("Cancel")}</Button>
                <Button disabled={!canManage || feedback.busy || conflict || (inherit && snapshot.config.revision === 0)} type="submit">{t(feedback.error === undefined ? "Save launch arguments" : "Retry")}</Button>
            </div>
        </form>}
        {snapshot === undefined ? <div className="dialog-actions"><Button onClick={close} type="button" variant="text">{t("Cancel")}</Button></div> : null}
        <MutationFeedback client={client} errorTitle={unknown ? t("Launch argument save result is unknown") : undefined} feedback={feedback} />
    </MaterialDialog>;
}

export function ProjectUnityActions({ afterMigrationOpen = false, client, installations, launchConfig, launchOptions, project, onChanged }: ActionProps & { afterMigrationOpen?: boolean; installations: UnityInstallation[]; launchConfig: ProjectUnityLaunchConfigResult; launchOptions: UnityLaunchOptionsResult; project: ProjectSnapshot }) {
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
    const migrationVersions = [...new Set(installations.map((installation) => installation.unityVersion))]
        .filter((version) => version !== project.unityVersion)
        .sort((left, right) => left.localeCompare(right));
    const targetInstallations = installations.filter((installation) => installation.unityVersion === targetVersion);
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
        const matches = installations.filter((installation) => installation.unityVersion === version);
        setTargetInstallationId(matches.length === 1 ? matches[0]?.installationId ?? "" : "");
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
    return (
        <ActionSection title={t("Unity actions")}>
            <p><Translated message="Project Unity version: {version}" values={{ version: <strong>{project.unityVersion}</strong> }} /></p>
            {effectiveLaunchOptions.exactMatchingInstallations.length > 1 ? <Select id="launch-installation" label={t("Unity installation for this launch")} onChange={setLaunchInstallationId} options={[{ label: t("Choose an installation"), value: "" }, ...effectiveLaunchOptions.exactMatchingInstallations.map((item) => ({ label: `Unity ${item.unityVersion} · ${item.architecture}`, value: item.installationId }))]} value={launchInstallationId} /> : null}
            <Button disabled={!canLaunch || feedback.busy || projectRevision === undefined || launchInstallation === undefined} onClick={() => void launch()} title={capabilityUnavailableTitle(canLaunch, capabilities.unityLaunch, t)} type="button">{t("Open Unity")}</Button>
            <Button disabled={!canManage || feedback.busy || projectId === undefined} onClick={() => setArgumentsOpen(true)} title={capabilityUnavailableTitle(canManage, capabilities.unityManage, t)} type="button" variant="tonal">{t("Edit launch arguments")}</Button>
            {!argumentsOpen || projectId === undefined ? null : <ProjectLaunchArgumentsDialog client={client} launchConfig={launchConfig} onChanged={onChanged} onClose={() => setArgumentsOpen(false)} projectId={projectId} />}
            <h3>{t("Migrate Project Unity version")}</h3>
            <Select id="project-unity-version" label={t("Target Unity version")} onChange={chooseTargetVersion} options={[{ label: t("Current · {version}", { version: project.unityVersion }), value: "" }, ...migrationVersions.map((version) => ({ label: version, value: version }))]} value={targetVersion} />
            {targetInstallations.length > 1 ? <Select id="migration-installation" label={t("Target Unity installation")} onChange={setTargetInstallationId} options={[{ label: t("Choose an installation"), value: "" }, ...targetInstallations.map((item) => ({ label: `${item.unityVersion} · ${item.architecture}`, value: item.installationId }))]} value={targetInstallationId} /> : null}
            <Button disabled={!canMigrate || feedback.busy || targetInstallationId.length === 0 || projectRevision === undefined} onClick={() => void planMigration()} title={capabilityUnavailableTitle(canMigrate, capabilities.projectsUnityMigration, t)} type="button" variant="tonal">{t("Review migration")}</Button>
            <PlanDialog applyDisabled={migrationPlan?.classification.supportedForApply === false} busy={feedback.busy} onApply={applyMigration} onClose={() => setMigrationPlan(undefined)} open={migrationPlan !== undefined} title={t("Review Unity migration")}>
                {migrationPlan === undefined ? null : <dl className="dialog-summary"><div><dt>{t("From")}</dt><dd>{migrationPlan.sourceUnityVersion}</dd></div><div><dt>{t("To")}</dt><dd>{migrationPlan.targetUnityVersion}</dd></div><div><dt>{t("Classification")}</dt><dd>{t(humanize(migrationPlan.classification.kind))}</dd></div><div><dt>{t("Supported")}</dt><dd>{migrationPlan.classification.supportedForApply ? t("Yes") : t("No")}</dd></div></dl>}
                {feedback.error === undefined ? null : <div role="alert"><p>{t(requestErrorMessage((feedback.error).code))}</p><code>{(feedback.error).code}</code>{feedback.error.diagnosticId === undefined ? null : <p><code>{feedback.error.diagnosticId}</code></p>}</div>}
            </PlanDialog>
            {feedback.operationId === undefined ? null : <OperationFollow client={client} operationId={feedback.operationId} onTerminal={migrationFinished} background />}
        </ActionSection>
    );
}

export function ProjectUnityWorkspaceActions({ client, installations, launchOptions, navigate, onChanged, project, renderLaunch }: {
    client: GuiRpcClient;
    installations: UnityInstallation[];
    launchOptions: UnityLaunchOptionsResult;
    navigate(path: string): void;
    onChanged(): void;
    project: ProjectSnapshot;
    renderLaunch(action: { disabled: boolean; onClick(): void; title?: string }): ReactNode;
}) {
    const { t } = useTranslation();
    const canMigrate = useCapability(capabilities.projectsUnityMigration);
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const unityLaunch = useProjectUnityLaunch({ client, initialOptions: launchOptions, navigate, onLaunchSettled: onChanged, project });
    const [migrationVersion, setMigrationVersion] = useState(project.unityVersion);
    const [migrationInstallationId, setMigrationInstallationId] = useState("");
    const [migrationChooserOpen, setMigrationChooserOpen] = useState(false);
    const [migrationPlan, setMigrationPlan] = useState<ProjectUnityMigrationPlan>();
    const [submitted, setSubmitted] = useState(false);
    const pending = useRef(false);
    const isCurrent = useActionScope(client);
    const projectId = project.projectId;
    const projectRevision = project.revision;
    const migrationVersions = [...new Set(installations.map((installation) => installation.unityVersion))]
        .filter((version) => version !== project.unityVersion)
        .sort((left, right) => left.localeCompare(right));
    const migrationInstallations = installations.filter((installation) => installation.unityVersion === migrationVersion);

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
                setMigrationVersion(project.unityVersion);
                setFeedback(INITIAL_FEEDBACK);
                return;
            }
            setMigrationPlan(result.plan);
            setFeedback({ busy: false });
        } catch (caught: unknown) {
            if (!isCurrent()) return;
            setMigrationVersion(project.unityVersion);
            setFeedback({ busy: false, error: safeError(caught) });
        } finally { pending.current = false; }
    };
    const chooseMigrationVersion = (version: string) => {
        setMigrationVersion(version);
        setFeedback(INITIAL_FEEDBACK);
        setMigrationPlan(undefined);
        if (version === project.unityVersion) return;
        const matches = installations.filter((installation) => installation.unityVersion === version);
        if (matches.length === 1) {
            void planMigration(matches[0]!.installationId);
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
                    { label: `Unity ${project.unityVersion}`, value: project.unityVersion },
                    ...migrationVersions.map((version) => ({ label: `Unity ${version}`, value: version }))
                ]}
                value={migrationVersion}
            />
            {renderLaunch({ ...unityLaunch.action, disabled: unityLaunch.action.disabled || feedback.busy })}
            {unityLaunch.dialogs}
            <ModalDialog dismissible={!feedback.busy} onClose={() => { setMigrationChooserOpen(false); setMigrationVersion(project.unityVersion); setFeedback(INITIAL_FEEDBACK); }} open={migrationChooserOpen} title={t("Choose Unity {version} installation", { version: migrationVersion })}>
                <Select label={t("Target Unity installation")} onChange={setMigrationInstallationId} options={[{ label: t("Choose an installation"), value: "" }, ...migrationInstallations.map((item) => ({ label: `${item.unityVersion} · ${item.architecture}`, value: item.installationId }))]} value={migrationInstallationId} />
                {feedback.error === undefined ? null : <div role="alert"><p>{t(requestErrorMessage((feedback.error).code))}</p><code>{(feedback.error).code}</code>{feedback.error.diagnosticId === undefined ? null : <p><code>{feedback.error.diagnosticId}</code></p>}</div>}
                <div className="dialog-actions">
                    <Button onClick={() => { setMigrationChooserOpen(false); setMigrationVersion(project.unityVersion); }} type="button" variant="tonal">{t("Cancel")}</Button>
                    <Button disabled={migrationInstallationId.length === 0 || feedback.busy} onClick={() => void planMigration(migrationInstallationId)} type="button">{t("Review migration")}</Button>
                </div>
            </ModalDialog>
            <PlanDialog applyDisabled={submitted || migrationPlan?.classification.supportedForApply === false} busy={feedback.busy} onApply={applyMigration} onClose={() => { setMigrationPlan(undefined); setMigrationVersion(project.unityVersion); setFeedback(INITIAL_FEEDBACK); }} open={migrationPlan !== undefined} title={t("Review Unity migration")}>
                {migrationPlan === undefined ? null : <dl className="dialog-summary"><div><dt>{t("From")}</dt><dd>{migrationPlan.sourceUnityVersion}</dd></div><div><dt>{t("To")}</dt><dd>{migrationPlan.targetUnityVersion}</dd></div><div><dt>{t("Classification")}</dt><dd>{t(humanize(migrationPlan.classification.kind))}</dd></div><div><dt>{t("Supported")}</dt><dd>{migrationPlan.classification.supportedForApply ? t("Yes") : t("No")}</dd></div></dl>}
                {feedback.error === undefined ? null : <div role="alert"><p>{t(requestErrorMessage((feedback.error).code))}</p><code>{(feedback.error).code}</code>{feedback.error.diagnosticId === undefined ? null : <p><code>{feedback.error.diagnosticId}</code></p>}{submitted && templateApplyResultUnknown(feedback.error) ? t("The result of this request is unknown. Check Tasks and refresh before reviewing it again.") : undefined}</div>}
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

export function TemplateImportPanel({ client, operations }: { client: GuiRpcClient; operations: TemplateOperations }) {
    const { t } = useTranslation();
    const available = useCapability(capabilities.templatesManage);
    const [bundlePath, setBundlePath] = useState("");
    const [overrideExisting, setOverrideExisting] = useState(false);
    const [review, setReview] = useState<TemplateReview>();
    const [submitted, setSubmitted] = useState(false);
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    useUtilityDialogBusy(feedback.busy);
    const pending = useRef(false);
    const current = useActionScope(client);
    const create = async (event: FormEvent) => {
        event.preventDefault();
        if (!available || pending.current) return;
        pending.current = true; setFeedback({ busy: true });
        try {
            const inspected = await client.templateInspectBundle(bundlePath);
            let existing: TemplateRecord | undefined;
            try { existing = (await client.templateGet(inspected.templateId)).template; }
            catch (caught: unknown) { if (safeError(caught).code !== "template_not_found") throw caught; }
            const plan = await client.templatePlanImport(bundlePath, overrideExisting, existing?.revision ?? 0);
            if (!current()) return;
            setReview(reviewTemplateImport(plan, inspected, existing, overrideExisting));
            setSubmitted(false); setFeedback({ busy: false });
        } catch (caught: unknown) { if (current()) setFeedback({ busy: false, error: safeError(caught) }); }
        finally { if (current()) pending.current = false; }
    };
    const apply = async () => {
        if (review === undefined || submitted || !available || pending.current) return;
        pending.current = true; setSubmitted(true); setFeedback({ busy: true });
        try {
            const result = await client.templateApplyImport(review.plan.planId);
            operations.accept(result.operationId);
            if (!current()) return;
            setReview(undefined); setBundlePath("");
            setFeedback({ busy: false, operationId: result.operationId });
        } catch (caught: unknown) { if (current()) setFeedback({ busy: false, error: safeError(caught) }); }
        finally { if (current()) pending.current = false; }
    };
    return <ActionSection title={review === undefined ? t("Import template") : t("Review template import")}>
        {feedback.operationId !== undefined ? <div className="dialog-actions"><UtilityDialogCancel label={t("Close")} /></div> : review === undefined ? <form onSubmit={(event) => void create(event)}>
            <TextField disabled={feedback.busy} id="template-bundle" label={t("Template bundle")} maxLength={1024} onInput={setBundlePath} required value={bundlePath} />
            <Checkbox checked={overrideExisting} disabled={feedback.busy} label={t("Replace an existing matching template")} onChange={setOverrideExisting} />
            <div className="dialog-actions"><UtilityDialogCancel disabled={feedback.busy} /><Button disabled={!available || feedback.busy || bundlePath.trim().length === 0} title={capabilityUnavailableTitle(available, capabilities.templatesManage, t)} type="submit">{t("Review import")}</Button></div>
        </form> : <>
            <TemplatePlanReview review={review} />
            <p className="risk-summary">{t("The daemon will revalidate this frozen plan before importing.")}</p>
            <div className="dialog-actions"><UtilityDialogCancel disabled={feedback.busy} /><Button disabled={feedback.busy} onClick={() => { setReview(undefined); setSubmitted(false); setFeedback(INITIAL_FEEDBACK); }} type="button" variant="text">{t("Discard plan")}</Button><Button disabled={!available || feedback.busy || submitted} onClick={() => void apply()} type="button">{t("Apply reviewed plan")}</Button></div>
        </>}
        <TemplateMutationFeedback client={client} feedback={feedback} operations={operations} submitted={submitted} />
    </ActionSection>;
}

export function TemplateCreateActions({ client, operations, primaryRef: providedPrimaryRef }: { client: GuiRpcClient; operations: TemplateOperations; primaryRef?: RefObject<HTMLElement | null> }) {
    const { t } = useTranslation();
    const canManage = useCapability(capabilities.templatesManage);
    const [creating, setCreating] = useState(false);
    const [importing, setImporting] = useState(false);
    const menuAnchorRef = useRef<HTMLElement>(null);
    const ownPrimaryRef = useRef<HTMLElement>(null);
    const primaryRef = providedPrimaryRef ?? ownPrimaryRef;
    useEffect(() => { setCreating(false); setImporting(false); }, [client]);
    return <>
        <SplitButton disabled={!canManage} label={t("Create template")} menuButtonRef={menuAnchorRef} menuLabel={t("More template creation options")} onClick={() => setCreating(true)} primaryRef={primaryRef} title={capabilityUnavailableTitle(canManage, capabilities.templatesManage, t)}>
            <MenuItem disabled={!canManage} label={t("Import template")} onClick={() => setImporting(true)} title={capabilityUnavailableTitle(canManage, capabilities.templatesManage, t)} />
        </SplitButton>
        {creating ? <TemplateDeriveDialog client={client} operations={operations} onClose={() => { setCreating(false); window.requestAnimationFrame(() => primaryRef.current?.focus()); }} /> : null}
        {importing ? <UtilityActionDialog title={t("Import template")} onClose={() => { setImporting(false); window.requestAnimationFrame(() => menuAnchorRef.current?.focus()); }}><TemplateImportPanel client={client} operations={operations} /></UtilityActionDialog> : null}
    </>;
}

function TemplateDeriveDialog({ client, operations, onClose, origin, closed = false }: { client: GuiRpcClient; operations: TemplateOperations; onClose(): void; origin?: DialogOrigin; closed?: boolean }) {
    const { t } = useTranslation();
    const canManage = useCapability(capabilities.templatesManage);
    const [open, setOpen] = useState(true);
    const [review, setReview] = useState<TemplateReview>();
    const [submitted, setSubmitted] = useState(false);
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const [projects, setProjects] = useState<ProjectSnapshot[]>([]);
    const [projectPage, setProjectPage] = useState<Awaited<ReturnType<GuiRpcClient["projectsList"]>>>();
    const [projectsLoading, setProjectsLoading] = useState(true);
    const showProjectsLoading = useDelayedPending(projectsLoading && projects.length === 0);
    const [fields, setFields] = useState({ projectId: "", templateId: "", templateVersion: "1.0.0", displayName: "", description: "" });
    const pending = useRef(false);
    const projectLoad = useRef(0);
    const current = useActionScope(client, open && !closed);
    const update = (key: keyof typeof fields, value: string) => setFields((current) => ({ ...current, [key]: value }));
    useEffect(() => {
        const generation = ++projectLoad.current;
        void client.projectsList().then((page) => {
            if (generation !== projectLoad.current) return;
            setProjectPage(page); setProjects(page.projects); setProjectsLoading(false);
        }).catch((caught: unknown) => { if (generation === projectLoad.current) { setProjectsLoading(false); setFeedback({ busy: false, error: safeError(caught) }); } });
        return () => { projectLoad.current += 1; };
    }, [client]);
    const loadMore = async () => {
        if (projectPage?.nextCursor === undefined || projectsLoading) return;
        const generation = ++projectLoad.current;
        setProjectsLoading(true);
        try {
            const page = await client.projectsList(projectPage.nextCursor);
            if (generation !== projectLoad.current) return;
            setProjectPage(page); setProjects((current) => [...current, ...page.projects]);
        } catch (caught: unknown) { if (generation === projectLoad.current) setFeedback({ busy: false, error: safeError(caught) }); }
        finally { if (generation === projectLoad.current) setProjectsLoading(false); }
    };
    const createPlan = async (event: FormEvent) => {
        event.preventDefault();
        if (pending.current || !canManage) return;
        pending.current = true; setFeedback({ busy: true });
        try {
            const source = (await client.projectGet(fields.projectId)).project;
            if (source.projectId === undefined || source.revision === undefined) throw { code: "project_revision_unavailable" };
            const request = { projectId: source.projectId, expectedProjectRevision: source.revision, templateId: fields.templateId, templateVersion: fields.templateVersion, displayName: fields.displayName, ...(fields.description.length === 0 ? {} : { description: fields.description }) };
            const plan = await client.templatePlanDerive(request);
            if (!current()) return;
            setReview(reviewTemplateDerive(plan, request)); setSubmitted(false); setFeedback({ busy: false });
        } catch (caught: unknown) { if (current()) setFeedback({ busy: false, error: safeError(caught) }); }
        finally { if (current()) pending.current = false; }
    };
    const apply = async () => {
        if (review === undefined || submitted || pending.current || !canManage) return;
        pending.current = true; setSubmitted(true); setFeedback({ busy: true });
        try {
            const result = await client.templateApplyDerive(review.plan.planId);
            operations.accept(result.operationId);
            if (!current()) return;
            setReview(undefined); setFeedback({ busy: false, operationId: result.operationId });
        } catch (caught: unknown) { if (current()) setFeedback({ busy: false, error: safeError(caught) }); }
        finally { if (current()) pending.current = false; }
    };
    const close = () => { if (!pending.current) setOpen(false); };
    return <MaterialDialog dismissible={!feedback.busy} open={open && !closed} onClose={onClose} returnFocus={origin} title={review === undefined ? t("Create template from project") : t("Review new template")}>
        {feedback.operationId !== undefined ? <div className="dialog-actions"><Button onClick={close} type="button" variant="text">{t("Close")}</Button></div> : review !== undefined ? <>
            <TemplatePlanReview review={review} />
            <p className="risk-summary">{t("The daemon will revalidate this frozen plan before applying it.")}</p>
            <div className="dialog-actions"><Button disabled={feedback.busy} onClick={close} type="button" variant="text">{t("Discard plan")}</Button><Button disabled={feedback.busy || submitted || !canManage} onClick={() => void apply()} type="button">{t("Apply reviewed plan")}</Button></div>
        </> : <form onSubmit={(event) => void createPlan(event)}>
            <Select id="derive-project" label={t("Source project")} disabled={feedback.busy || projectsLoading} onChange={(next) => update("projectId", next)} options={[{ label: t("Choose a project"), value: "" }, ...projects.filter((project) => project.projectId !== undefined).map((project) => ({ label: displayPath(project.rootPath), value: project.projectId! }))]} value={fields.projectId} />
            {showProjectsLoading ? <p role="status">{t("Loading projects…")}</p> : !projectsLoading && projects.length === 0 ? <p>{t("No registered projects available.")}</p> : null}
            {projectPage?.nextCursor === undefined ? null : <Button disabled={projectsLoading || feedback.busy} onClick={() => void loadMore()} type="button" variant="text">{t("Load more projects")}</Button>}
            <TextField disabled={feedback.busy} id="derive-name" label={t("Display name")} onInput={(next) => update("displayName", next)} required value={fields.displayName} />
            <TextField disabled={feedback.busy} id="derive-template-id" label={t("New template ID")} onInput={(next) => update("templateId", next)} required value={fields.templateId} />
            <TextField disabled={feedback.busy} id="derive-version" label={t("Template version")} onInput={(next) => update("templateVersion", next)} required value={fields.templateVersion} />
            <TextField disabled={feedback.busy} id="derive-description" label={t("Description")} onInput={(next) => update("description", next)} value={fields.description} />
            <div className="dialog-actions"><Button disabled={feedback.busy} onClick={close} type="button" variant="text">{t("Cancel")}</Button><Button disabled={feedback.busy || !canManage || fields.projectId.length === 0} type="submit">{t("Review")}</Button></div>
        </form>}
        {open && !closed ? <TemplateMutationFeedback client={client} feedback={feedback} operations={operations} submitted={submitted} /> : null}
    </MaterialDialog>;
}

export interface TemplateActionSelection {
    action: "derive" | "create" | "export" | "remove";
    key: number;
    template: TemplateRecord;
    origin: DialogOrigin;
    closed?: boolean;
}

export function TemplateActions({ client, compact = false, onAction, onChanged, onView, template }: ActionProps & { compact?: boolean; onAction(action: TemplateActionSelection["action"], origin: DialogOrigin): void; onView?(): void; template: TemplateRecord }) {
    const { t } = useTranslation();

    const canCreateProject = useCapability(capabilities.templatesCreateProject);
    const canManage = useCapability(capabilities.templatesManage);
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const [menuOpen, setMenuOpen] = useState(false);
    const menuAnchorRef = useRef<HTMLElement>(null);
    const pending = useRef(false);
    const current = useActionScope(client);
    const open = (action: TemplateActionSelection["action"]) => {
        if (pending.current) return;
         setFeedback(INITIAL_FEEDBACK);
        setMenuOpen(false);
        onAction(action, compact ? menuAnchorRef : { current: document.activeElement instanceof HTMLElement ? document.activeElement : null });
    };
    const favorite = async () => {
        if (pending.current || !canManage) return;
        pending.current = true;  setFeedback({ busy: true });
        try {
            await client.templateSetFavorite(template.templateId, !template.favorite, template.revision);
            if (!current()) return;
            setFeedback(INITIAL_FEEDBACK);  onChanged?.();
        } catch (caught: unknown) {
            if (current()) {
                setFeedback(INITIAL_FEEDBACK);
                if (safeError(caught).code === "revision_conflict") onChanged?.();
            }
        } finally { pending.current = false; }
    };
    return <section className={compact ? "template-row-actions" : "action-section"}>
        {compact ? <div className="card-actions">
            <IconButton aria-pressed={template.favorite} className="project-favorite-action" disabled={!canManage || feedback.busy} label={template.favorite ? t("Remove {name} from favorites", { name: template.displayName }) : t("Favorite {name}", { name: template.displayName })} onClick={() => void favorite()} type="button"><Icon asset={starIcon} /></IconButton>
            <IconButton aria-expanded={menuOpen} disabled={feedback.busy} label={t("More actions for {name}", { name: template.displayName })} onClick={() => setMenuOpen(true)} ref={menuAnchorRef} type="button"><Icon asset={moreVertIcon} size={24} /></IconButton>
            <Menu anchorRef={menuAnchorRef} onClose={() => setMenuOpen(false)} open={menuOpen}>
                <MenuItem disabled={!canCreateProject} label={t("Create project")} onClick={() => open("create")} />
                {onView === undefined ? null : <MenuItem label={t("View template")} onClick={() => { setMenuOpen(false); onView(); }} />}
                <MenuItem disabled={!canManage} label={t("Derive from project")} onClick={() => open("derive")} />
                <MenuItem disabled={!canManage} label={t("Export template")} onClick={() => open("export")} />
                <MenuItem disabled={!canManage || template.sourceKind === "builtin"} label={t("Remove template")} onClick={() => open("remove")} title={template.sourceKind === "builtin" ? t("Built-in templates cannot be removed.") : undefined} />
            </Menu>
        </div> : <><h2>{t("Template actions")}</h2><div className="action-row">
            <Button disabled={!canCreateProject || feedback.busy} onClick={() => open("create")} title={capabilityUnavailableTitle(canCreateProject, capabilities.templatesCreateProject, t)} type="button">{t("Create project")}</Button>
            <Button disabled={!canManage || feedback.busy} onClick={() => open("derive")} title={capabilityUnavailableTitle(canManage, capabilities.templatesManage, t)} type="button" variant="tonal">{t("Derive from project")}</Button>
            <Button widthLabels={[t("Remove favorite"), t("Favorite")]} disabled={!canManage || feedback.busy} onClick={() => void favorite()} type="button" variant="text">{template.favorite ? t("Remove favorite") : t("Favorite")}</Button>
            <Button disabled={!canManage || feedback.busy} onClick={() => open("export")} title={capabilityUnavailableTitle(canManage, capabilities.templatesManage, t)} type="button" variant="text">{t("Export template")}</Button>
            {template.sourceKind === "builtin" ? null : <Button className="material-button--danger" disabled={!canManage || feedback.busy} onClick={() => open("remove")} type="button" variant="text">{t("Remove template")}</Button>}
        </div></>}
    </section>;
}

export function TemplateActionDialog({ client, onChanged, onClose, onOpenProject, operations, selection }: ActionProps & { onClose(): void; onOpenProject(projectId: string): void; operations: TemplateOperations; selection: TemplateActionSelection }) {
    return selection.action === "derive"
        ? <TemplateDeriveDialog client={client} closed={selection.closed} onClose={onClose} operations={operations} origin={selection.origin} />
        : <TemplateObjectDialog client={client} onChanged={onChanged} onClose={onClose} onOpenProject={onOpenProject} operations={operations} selection={selection} />;
}

function TemplateObjectDialog({ client, onChanged, onClose, onOpenProject, operations, selection }: ActionProps & { onClose(): void; onOpenProject(projectId: string): void; operations: TemplateOperations; selection: TemplateActionSelection }) {
    const { t } = useTranslation();

    const canCreateProject = useCapability(capabilities.templatesCreateProject);
    const canManage = useCapability(capabilities.templatesManage);
    const { template, action } = selection;
    const [open, setOpen] = useState(true);
    const [review, setReview] = useState<TemplateReview>();
    const [submitted, setSubmitted] = useState(false);
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const [fields, setFields] = useState({ leaf: "", exportPath: "" });
    const projectDirectory = useProjectDirectory(client, action === "create" && open && !selection.closed);
    const unity = useCreateUnityVersion(client, template, action === "create" && open && !selection.closed);
    const pending = useRef(false);
    const current = useActionScope(client, open && !selection.closed);
    const update = (key: keyof typeof fields, value: string) => { setFields((current) => ({ ...current, [key]: value })); setFeedback(INITIAL_FEEDBACK); };
    const close = () => { if (!pending.current) {  setFeedback(INITIAL_FEEDBACK); setOpen(false); } };
    const createPlan = async (event: FormEvent) => {
        event.preventDefault();
        if (pending.current || !canCreateProject) return;
        pending.current = true; setFeedback({ busy: true });
        try {
            const plan = await client.templatePlanCreateProject(template.templateId, template.revision, projectDirectory.directory, fields.leaf, unity.version.trim());
            if (!current()) return;
            setReview(reviewTemplateCreation(plan, template, projectDirectory.directory, fields.leaf, unity.version.trim())); setSubmitted(false); setFeedback({ busy: false });
        } catch (caught: unknown) { if (current()) setFeedback({ busy: false, error: safeError(caught) }); }
        finally { if (current()) pending.current = false; }
    };
    const apply = async () => {
        if (review === undefined || submitted || pending.current || !canCreateProject) return;
        pending.current = true; setSubmitted(true); setFeedback({ busy: true });
        try {
            const result = await client.templateApplyCreateProject(review.plan.planId);
            operations.accept(result.operationId);
            if (!current()) return;
            setFeedback({ busy: false, operationId: result.operationId });
        } catch (caught: unknown) { if (current()) setFeedback({ busy: false, error: safeError(caught) }); }
        finally { if (current()) pending.current = false; }
    };
    const simple = async () => {
        if (pending.current || !canManage || (action === "remove" && template.sourceKind === "builtin")) return;
        pending.current = true; setFeedback({ busy: true });
        try {
            if (action === "export") await client.templateExport(template.templateId, template.revision, fields.exportPath);
            else if (action === "remove") await client.templateRemove(template.templateId, template.revision);
            else return;
            if (!current()) return;
            setFeedback(INITIAL_FEEDBACK);  setOpen(false);
            if (action === "remove") onChanged?.();
        } catch (caught: unknown) { if (current()) setFeedback({ busy: false, error: safeError(caught) }); }
        finally { if (current()) pending.current = false; }
    };
    const pickParent = async () => {
        if (pending.current) return;
        pending.current = true; setFeedback({ busy: true });
        try { const path = await client.selectDirectory(); if (current() && path !== undefined) projectDirectory.setDirectory(path); }
        catch (caught: unknown) { if (current()) setFeedback({ busy: false, error: safeError(caught) }); }
        finally { if (current()) { pending.current = false; setFeedback((value) => ({ ...value, busy: false })); } }
    };
    const createdProject = feedback.operationId === undefined ? undefined : templateOperationProjectId(operations.snapshot(feedback.operationId)?.operation);
    const title = review !== undefined ? t("Review project creation") : action === "create" ? t("Create project") : action === "export" ? t("Export template") : t("Remove template");
    return <MaterialDialog dismissible={!feedback.busy} open={open && !selection.closed} onClose={onClose} returnFocus={selection.origin} title={title}>
        {feedback.operationId !== undefined ? <div className="dialog-actions">
            <Button onClick={close} type="button" variant="text">{t("Close")}</Button>
            {createdProject === undefined ? null : <Button onClick={() => onOpenProject(createdProject)} type="button">{t("Open created project")}</Button>}
        </div> : review !== undefined ? <>
            <TemplatePlanReview review={review} />
            <p className="risk-summary">{t("The daemon will revalidate this frozen plan before applying it.")}</p>
            <div className="dialog-actions"><Button disabled={feedback.busy} onClick={close} type="button" variant="text">{t("Discard plan")}</Button><Button disabled={feedback.busy || submitted || !canCreateProject} onClick={() => void apply()} type="button">{t("Apply reviewed plan")}</Button></div>
        </> : action === "create" ? <form onSubmit={(event) => void createPlan(event)}>
            <p>{t("Template:")}<strong>{template.displayName}</strong></p>
            <CreateUnityVersionField disabled={feedback.busy} selection={unity} />
            <TextField disabled={feedback.busy} id="create-leaf" label={t("Project folder name")} onInput={(next) => update("leaf", next)} required value={fields.leaf} />
            <PathField disabled={feedback.busy} id="create-parent" label={t("Target parent")} onInput={projectDirectory.setDirectory} required value={projectDirectory.directory} />
            <Button disabled={feedback.busy} onClick={() => void pickParent()} type="button" variant="text">{t("Browse…")}</Button>
            {projectDirectory.error === undefined ? null : <div role="alert"><strong>{t("Default project location could not be loaded.")}</strong><p>{t(requestErrorMessage((projectDirectory.error).code))}</p><code>{(projectDirectory.error).code}</code>{projectDirectory.error.diagnosticId === undefined ? null : <p><code>{projectDirectory.error.diagnosticId}</code></p>}</div>}
            <div className="dialog-actions"><Button disabled={feedback.busy} onClick={close} type="button" variant="text">{t("Cancel")}</Button><Button disabled={feedback.busy || !canCreateProject || projectDirectory.directory.length === 0 || unity.version.trim().length === 0} type="submit">{t("Review")}</Button></div>
        </form> : action === "export" ? <form onSubmit={(event) => { event.preventDefault(); void simple(); }}>
            <p><Translated message="Export {name} as a template bundle." values={{ name: <strong>{template.displayName}</strong> }} /></p>
            <TextField disabled={feedback.busy} id="template-export" label={t("Export target")} onInput={(next) => update("exportPath", next)} required value={fields.exportPath} />
            <div className="dialog-actions"><Button disabled={feedback.busy} onClick={close} type="button" variant="text">{t("Cancel")}</Button><Button disabled={!canManage || feedback.busy} type="submit">{t("Export")}</Button></div>
        </form> : <>
            <p><Translated message="Remove {name} from your templates?" values={{ name: <strong>{template.displayName}</strong> }} /></p>
            <div className="dialog-actions"><Button disabled={feedback.busy} onClick={close} type="button" variant="text">{t("Cancel")}</Button><Button className="material-button--danger" disabled={!canManage || feedback.busy} onClick={() => void simple()} type="button">{t("Remove template")}</Button></div>
        </>}
        <TemplateMutationFeedback client={client} feedback={feedback} operations={operations} submitted={submitted} />
    </MaterialDialog>;
}

export function BackupCreatePanel({ client, navigate, onAccepted, project }: { client: GuiRpcClient; navigate(path: string): void; onAccepted(result: BackupCreateResult): void; project: ProjectSnapshot }) {
    const { t } = useTranslation();
    const available = useCapability(capabilities.backupsCreate);
    const [options, setOptions] = useState<OfficialSettings["backups"]>();
    const [accepted, setAccepted] = useState<BackupCreateResult>();
    const [readGeneration, setReadGeneration] = useState(0);
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const showLoading = useDelayedPending(options === undefined && feedback.error === undefined);
    const unknown = templateApplyResultUnknown(feedback.error);
    const pending = useRef(false);
    const close = useUtilityDialogClose();
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
                <Button onClick={close} type="button" variant="text">{t("Close")}</Button>
                <Button onClick={() => navigate(`/operations/${accepted.operationId}`)} type="button" variant="text">{t("View operation")}</Button>
                {operation?.state !== "succeeded" ? null : <Button onClick={() => navigate(`/backups/${accepted.backupId}`)} type="button">{t("View backup")}</Button>}
            </div>
        </>}
    </OperationFollow>;
    return <ActionSection title={t("Create backup")}>
        <p><Translated message="Back up {name}." values={{ name: <strong>{project.rootPath.split(/[\\/]/).filter(Boolean).at(-1) ?? project.projectId}</strong> }} /></p>
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
            <div className="dialog-actions"><Button disabled={feedback.busy} onClick={() => { setConfirm(false); setFeedback(INITIAL_FEEDBACK); }} type="button" variant="text">{t("Back")}</Button><Button className="material-button--danger" disabled={!available || feedback.busy || terminal || operation.state === "cancelling"} onClick={() => void cancel()} type="button">{t("Confirm")}</Button></div>
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
    const close = useUtilityDialogClose();
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
            if (!templateApplyResultUnknown(error)) setSubmitted(false);
            setFeedback({ busy: false, error });
        }
        finally { pending.current = false; }
    };
    return <section aria-label={plan === undefined ? t("Extension install input") : t("Review extension install")} aria-busy={feedback.busy}>
        {feedback.operationId !== undefined ? <div className="dialog-actions"><Button onClick={close} type="button" variant="text">{t("Close")}</Button></div> : plan === undefined ? <form onSubmit={(event) => { event.preventDefault(); void run(false); }}>
            <TextField disabled={feedback.busy} id="extension-package" label={t("Extension package")} maxLength={32768} onInput={(value) => { setPath(value); setFeedback(INITIAL_FEEDBACK); }} required value={path} />
            <Checkbox disabled={feedback.busy} checked={approvePublisher} label={t("Approve this publisher for this extension")} onChange={(value) => { setApprovePublisher(value); setFeedback(INITIAL_FEEDBACK); }} />
            <div className="dialog-actions"><UtilityDialogCancel disabled={feedback.busy} /><Button disabled={!available || feedback.busy || path.trim().length === 0} title={capabilityUnavailableTitle(available, capabilities.extensionsLifecycle, t)} type="submit">{t("Create install plan")}</Button></div>
        </form> : <>
            <h2>{t("Review extension install")}</h2>
            <dl className="dialog-summary"><div><dt>{t("Extension")}</dt><dd>{plan.extensionId}</dd></div><div><dt>{t("Version")}</dt><dd>{plan.version}</dd></div><div><dt>{t("Publisher")}</dt><dd><code>{plan.publisherFingerprint}</code></dd></div><div><dt>{t("Trust")}</dt><dd>{t(humanize(plan.trustDecision))}</dd></div></dl>
            {hasPermissionRequests(plan) ? <ExtensionPermissionRequests required={plan.requiredPermissions} optional={plan.optionalPermissions} /> : <div>{t("Permission details are unavailable from this Core.")}</div>}
            <div className="dialog-actions"><UtilityDialogCancel disabled={feedback.busy} /><Button disabled={feedback.busy || submitted} onClick={() => { setPlan(undefined); setFeedback(INITIAL_FEEDBACK); }} type="button" variant="text">{t("Discard plan")}</Button><Button disabled={!available || feedback.busy || submitted || !hasPermissionRequests(plan)} onClick={() => void run(true)} type="button">{t("Apply reviewed plan")}</Button></div>
        </>}
        {submitted && templateApplyResultUnknown(feedback.error) ? <div role="alert"><p>{t(requestErrorMessage((feedback.error!).code))}</p><code>{(feedback.error!).code}</code>{feedback.error!.diagnosticId === undefined ? null : <p><code>{feedback.error!.diagnosticId}</code></p>}{t("The result of this request is unknown. Check Tasks and refresh before reviewing it again.")}</div> : <MutationFeedback client={client} feedback={feedback} onOperationTerminal={onAccepted === undefined ? () => onChanged?.() : undefined} />}
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
        } catch (caught: unknown) { if (current()) { const error = safeError(caught); if (!templateApplyResultUnknown(error)) setSubmitted(false); setFeedback({ busy: false, error }); } }
        finally { pending.current = false; }
    };
    const blocked = extension.desiredState === "uninstalling" || operationActive;
    const quarantined = extension.quarantineState === "quarantined";
    return <ActionSection title={t("Extension management")}>
        <div className="action-row"><Button disabled={!canManageLifecycle || feedback.busy || blocked || (extension.desiredState === "enabled" && !quarantined)} onClick={() => void run("enable")} type="button" variant="tonal">{quarantined ? t("Recover extension") : t("Enable")}</Button><Button disabled={!canManageLifecycle || feedback.busy || blocked || extension.desiredState !== "enabled"} onClick={() => void run("disable")} type="button" variant="tonal">{t("Disable")}</Button><Button disabled={feedback.busy || blocked} onClick={() => open("permissions")} type="button" variant="text">{t("Manage permissions")}</Button><Button className="material-button--danger" disabled={!canManageLifecycle || feedback.busy || blocked} onClick={() => open("uninstall")} type="button" variant="text">{t("Uninstall extension")}</Button></div>
        <MaterialDialog dismissible={!feedback.busy && !permissionsBusy} open={mode !== "none"} onClose={close} title={mode === "permissions" ? t("Extension permissions") : plan === undefined ? t("Uninstall extension") : t("Review extension uninstall")}>
            <p><strong>{extension.extensionId}</strong></p>
            {mode === "permissions" ? <ExtensionPermissions client={client} extensionId={extension.extensionId} onBusyChange={setPermissionsBusy} onChanged={onChanged} onClose={close} /> : feedback.operationId !== undefined ? <div className="dialog-actions"><Button onClick={close} type="button" variant="text">{t("Close")}</Button></div> : plan === undefined ? <>
                <p>{t("Review the removal before applying it.")}</p>
                <Checkbox disabled={feedback.busy} checked={deleteData} label={t("Delete extension-owned data during uninstall")} onChange={setDeleteData} />
                <div className="dialog-actions"><Button disabled={feedback.busy} onClick={close} type="button" variant="text">{t("Cancel")}</Button><Button disabled={!canManageLifecycle || feedback.busy} onClick={() => void run("plan")} type="button">{t("Create uninstall plan")}</Button></div>
            </> : <>
                <dl className="dialog-summary"><div><dt>{t("Extension")}</dt><dd>{plan.extensionId}</dd></div><div><dt>{t("Version")}</dt><dd>{plan.version}</dd></div><div><dt>{t("Data")}</dt><dd>{t(humanize(plan.dataDisposition))}</dd></div><div><dt>{t("Publisher")}</dt><dd><code>{plan.publisherFingerprint}</code></dd></div></dl>
                <div className="dialog-actions"><Button disabled={feedback.busy || submitted} onClick={close} type="button" variant="text">{t("Discard plan")}</Button><Button disabled={!canManageLifecycle || feedback.busy || submitted} onClick={() => void run("apply")} type="button">{t("Apply reviewed plan")}</Button></div>
            </>}
            {mode === "none" ? null : submitted && templateApplyResultUnknown(feedback.error) ? <><div role="alert"><p>{t(requestErrorMessage((feedback.error!).code))}</p><code>{(feedback.error!).code}</code>{feedback.error!.diagnosticId === undefined ? null : <p><code>{feedback.error!.diagnosticId}</code></p>}{t("The result of this request is unknown. Check Tasks and refresh before reviewing it again.")}</div><div className="dialog-actions"><Button onClick={close} type="button" variant="text">{t("Close")}</Button></div></> : <MutationFeedback client={client} feedback={feedback} onOperationTerminal={onAccepted === undefined ? () => onChanged?.() : undefined} />}
        </MaterialDialog>
    </ActionSection>;
}

function ActionSection({ children, title }: { children: ReactNode; title: string }) {
    return <section className="action-section"><h2>{title}</h2>{children}</section>;
}

function PlanDialog({ applyDisabled = false, busy, children, onApply, onClose, open, title }: { applyDisabled?: boolean; busy: boolean; children: ReactNode; onApply(): Promise<void>; onClose(): void; open: boolean; title: string }) {
    const { t } = useTranslation();
    return <ModalDialog dismissible={!busy} open={open} title={title} onClose={onClose}>{children}<p className="risk-summary">{t("The daemon will revalidate this frozen plan. A stale plan fails instead of being silently replaced.")}</p><div className="dialog-actions"><Button disabled={busy} onClick={onClose} type="button" variant="tonal">{t("Discard plan")}</Button><Button data-dialog-initial-focus disabled={busy || applyDisabled} onClick={() => void onApply()} type="button">{t("Apply reviewed plan")}</Button></div></ModalDialog>;
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

interface AcceptedTemplateOperation {
    operationId: string;
    client: GuiRpcClient;
    onSucceeded?(operation: Operation): void | Promise<void>;
    operation?: Operation;
    error?: RpcError;
    progressError?: RpcError;
}

export type TemplateOperations = ReturnType<typeof useTemplateOperations>;

// These are only this page's explicitly accepted IDs. Core remains the task history owner.
export function useTemplateOperations(client: GuiRpcClient) {
    const [accepted, setAccepted] = useState<AcceptedTemplateOperation[]>([]);
    const owner = useRef({ client, active: false, seen: new Set<string>(), finished: new Set<string>() });
    if (owner.current.client !== client) owner.current = { client, active: false, seen: new Set(), finished: new Set() };
    const scope = owner.current;
    useEffect(() => {
        scope.active = true;
        setAccepted([]);
        return () => { scope.active = false; };
    }, [scope]);
    const accept = (operationId: string, onSucceeded?: AcceptedTemplateOperation["onSucceeded"]) => {
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

export function templateOperationProjectId(operation: Operation | undefined): string | undefined {
    if (operation?.state !== "succeeded" || typeof operation.result !== "object" || operation.result === null) return undefined;
    const projectId = (operation.result as { projectId?: unknown }).projectId;
    return canonicalUuid(projectId) ? projectId : undefined;
}

function canonicalUuid(value: unknown): value is string { return typeof value === "string" && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(value); }

export function templateApplyResultUnknown(error: RpcError | undefined): boolean {
    return error?.code === "daemon_unavailable" || error?.code === "internal_error";
}

function TemplateMutationFeedback({ client, feedback, operations, submitted = false }: { client: GuiRpcClient; feedback: FeedbackState; operations: TemplateOperations; submitted?: boolean }) {
    const { t } = useTranslation();
    if (feedback.operationId === undefined) return <><MutationFeedback client={client} feedback={feedback} />{submitted && templateApplyResultUnknown(feedback.error) ? <p>{t("The result of this request is unknown. Check Tasks and refresh before reviewing it again.")}</p> : null}</>;
    return <TemplateOperationProgress operationId={feedback.operationId} operations={operations} />;
}

export function TemplateOperationProgress({ operationId, operations }: { operationId: string; operations: TemplateOperations }) {
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
