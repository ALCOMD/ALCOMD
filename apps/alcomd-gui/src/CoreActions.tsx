import { Translated, useTranslation } from "./i18n";
import type { Translate } from "./i18n-types";
import type { ExtensionRecord, RpcError } from "@alcomd/sdk";
import { queryCandidates } from "./package-candidates";
import { useUtilityDialogBusy, UtilityActionDialog, UtilityDialogCancel } from "./UtilityActionDialog";
import { useCallback, useEffect, useRef, useState, type FormEvent, type ReactNode } from "react";

import type {
    BackupRecord,
    BackupRestorePlan,
    ExtensionPlan,
    Operation,
    PackagePlan,
    PackageSourceSelector,
    ProjectSnapshot,
    ProjectUnityLaunchConfig,
    ProjectUnityMigrationPlan,
    RepositorySnapshot,
    TemplatePlan,
    TemplateRecord,
    UnityInstallation,
    UnityLaunchOptionsResult
} from "./core-models";
import type { GuiRpcClient } from "./rpc";
import { closeIcon, moreVertIcon, starIcon } from "@alcomd/ui/icons";
import { Button, Checkbox, Dialog as MaterialDialog, Icon, IconButton, Menu, MenuItem, Progress, RefreshButton, Select, TextField, type DialogOrigin } from "./Material";
import { SplitButton } from "./SplitButton";
import { LoadingState, useDelayedPending } from "./LoadingState";
import { useProjectUnityLaunch } from "./ProjectUnityLaunch";
import { capabilities, capabilityUnavailableTitle, useCapability } from "./capabilities";

interface ActionProps {
    client: GuiRpcClient;
    onChanged?(): void;
}

interface FeedbackState {
    busy: boolean;
    error?: RpcError;
    message?: string;
    operationId?: string;
}

const INITIAL_FEEDBACK: FeedbackState = { busy: false };

export function RegisterProjectPanel({ client, onChanged }: ActionProps) {
    const { t } = useTranslation();
    const available = useCapability(capabilities.projectsRegistry);
    const [path, setPath] = useState("");
    const [confirm, setConfirm] = useState(false);
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const run = async () => {
        setFeedback({ busy: true });
        try {
            const result = await client.projectRegister(path);
            setFeedback({ busy: false, message: t("Project registered at revision {revision}.", { revision: result.project.revision ?? t("unknown") }) });
            setPath("");
            onChanged?.();
        } catch (caught: unknown) {
            setFeedback({ busy: false, error: safeError(caught) });
        }
    };
    return (
        <ActionSection title={t("Register project")}>
            <form onSubmit={(event) => { event.preventDefault(); setConfirm(true); }}>
                <TextField aria-describedby="project-root-hint" id="project-root" label={t("Project root")} maxLength={1024} onInput={setPath} required supportingText={t("The daemon validates and owns this path.")} value={path} />
                <Button disabled={!available || feedback.busy || path.length === 0} title={capabilityUnavailableTitle(available, capabilities.projectsRegistry, t)} type="submit">{t("Review registration")}</Button>
            </form>
            <ConfirmDialog busy={feedback.busy} open={confirm} title={t("Register this project?")} detail={t("ALCOMD will inspect the selected root and add it to the per-user registry.")} onClose={() => setConfirm(false)} onConfirm={run} />
            <MutationFeedback client={client} feedback={feedback} />
        </ActionSection>
    );
}

export function ProjectActions({ client, onChanged, project }: ActionProps & { project: ProjectSnapshot }) {
    const { t } = useTranslation();
    const available = useCapability(capabilities.projectsRegistry);
    const [confirmUnregister, setConfirmUnregister] = useState(false);
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const revision = project.revision;
    const refresh = async () => {
        if (project.projectId === undefined || revision === undefined) return;
        await runSimple(setFeedback, () => client.projectRefresh(project.projectId!, revision), t("Project refreshed."), onChanged);
    };
    const unregister = async () => {
        if (project.projectId === undefined || revision === undefined) return;
        await runSimple(setFeedback, () => client.projectUnregister(project.projectId!, revision), t("Project unregistered. Files were not deleted."), onChanged);
    };
    return (
        <ActionSection title={t("Project actions")}>
            <div className="action-row">
                <RefreshButton disabled={!available || feedback.busy || revision === undefined} onClick={() => void refresh()} title={capabilityUnavailableTitle(available, capabilities.projectsRegistry, t)} />
                <Button className="material-button--danger" disabled={!available || feedback.busy || revision === undefined} onClick={() => setConfirmUnregister(true)} title={capabilityUnavailableTitle(available, capabilities.projectsRegistry, t)} type="button" variant="text">{t("Unregister")}</Button>
            </div>
            <ConfirmDialog busy={feedback.busy} open={confirmUnregister} title={t("Unregister this project?")} detail={t("This removes only the ALCOMD registry entry. It does not delete the Unity project.")} onClose={() => setConfirmUnregister(false)} onConfirm={unregister} />
            <MutationFeedback client={client} feedback={feedback} />
        </ActionSection>
    );
}

export function RegisterRepositoryPanel({ client, onChanged }: ActionProps) {
    const { t } = useTranslation();
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
        try { setPreview((await client.repositoriesInspect(source())).repository); setFeedback({ busy: false }); }
        catch (caught: unknown) { setFeedback({ busy: false, error: safeError(caught) }); }
        finally { pending.current = false; }
    };
    const register = async () => {
        if (pending.current || !available || preview === undefined) return;
        pending.current = true;
        setFeedback({ busy: true });
        try {
            await client.repositoryRegister(preview.source);
            setPreview(undefined); setValue("");
            setFeedback({ busy: false, message: t("Repository added.") }); onChanged?.();
        } catch (caught: unknown) { setFeedback({ busy: false, error: safeError(caught) }); }
        finally { pending.current = false; }
    };
    return <ActionSection title={t("Add repository")}>
        {preview === undefined ? <form onSubmit={(event) => void inspect(event)}>
            <Select disabled={feedback.busy} id="repository-kind" label={t("Source type")} onChange={(next) => setKind(next as "remote" | "local")} options={[{ label: t("Remote URL"), value: "remote" }, { label: t("Local manifest"), value: "local" }]} value={kind} />
            <TextField disabled={feedback.busy} id="repository-source" label={kind === "remote" ? t("Repository URL") : t("Local manifest path")} maxLength={2048} onInput={setValue} required type={kind === "remote" ? "url" : "text"} value={value} />
            <div className="dialog-actions"><UtilityDialogCancel disabled={feedback.busy} /><Button disabled={!available || feedback.busy || value.trim().length === 0} title={capabilityUnavailableTitle(available, capabilities.repositoriesRegistry, t)} type="submit">{t("Review repository")}</Button></div>
        </form> : <>
            <dl className="dialog-summary"><div><dt>{t("Name")}</dt><dd>{preview.name ?? t("Unnamed repository")}</dd></div><div><dt>{t("Source")}</dt><dd>{preview.source.kind === "remote" ? preview.source.url : preview.source.path}</dd></div></dl>
            {preview.issues.length === 0 ? null : <p role="status">{t("This repository has {count} reported issues. Registration will validate the source again.", { count: preview.issues.length })}</p>}
            <div className="dialog-actions"><UtilityDialogCancel disabled={feedback.busy} /><Button disabled={feedback.busy} onClick={() => { setPreview(undefined); setFeedback(INITIAL_FEEDBACK); }} type="button" variant="text">{t("Back")}</Button><Button disabled={!available || feedback.busy} onClick={() => void register()} type="button">{t("Add repository")}</Button></div>
        </>}
        <MutationFeedback client={client} feedback={feedback} />
    </ActionSection>;
}

export function RepositoryActions({ client, onChanged, repository }: ActionProps & { repository: RepositorySnapshot }) {
    const { t } = useTranslation();
    const available = useCapability(capabilities.repositoriesRegistry);
    const [confirmRemove, setConfirmRemove] = useState(false);
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const pending = useRef(false);
    const ready = repository.repositoryId !== undefined && repository.revision !== undefined;
    const run = async (remove: boolean) => {
        if (!ready || !available || pending.current) return;
        pending.current = true; setFeedback({ busy: true });
        try {
            if (remove) await client.repositoryUnregister(repository.repositoryId!, repository.revision!);
            else await client.repositoryRefresh(repository.repositoryId!, repository.revision!);
            setFeedback({ busy: false, message: remove ? t("Repository removed.") : t("Repository refreshed.") });
            setConfirmRemove(false); onChanged?.();
        } catch (caught: unknown) { setFeedback({ busy: false, error: safeError(caught) }); }
        finally { pending.current = false; }
    };
    return <div className="repository-row-actions">
        <RefreshButton disabled={!available || !ready || feedback.busy} label={t("Refresh repository")} onClick={() => void run(false)} refreshing={feedback.busy && !confirmRemove} refreshingLabel={t("Refreshing repository")} />
        <IconButton disabled={!available || !ready || feedback.busy} label={t("Remove")} onClick={() => setConfirmRemove(true)} title={t("Remove repository")} type="button"><Icon asset={closeIcon} /></IconButton>
        <MaterialDialog open={confirmRemove} title={t("Remove this repository?")} dismissible={!feedback.busy} onClose={() => setConfirmRemove(false)}>
            <p><Translated message="Remove {name} from your sources?" values={{ name: <strong>{repository.name ?? repository.declaredId ?? t("this repository")}</strong> }} /></p><p>{t("Packages already installed in projects stay installed.")}</p>
            <MutationFeedback client={client} feedback={feedback} />
            <div className="dialog-actions"><Button disabled={feedback.busy} onClick={() => setConfirmRemove(false)} type="button" variant="text">{t("Cancel")}</Button><Button disabled={feedback.busy || !available} onClick={() => void run(true)} type="button">{t("Remove repository")}</Button></div>
        </MaterialDialog>
        {confirmRemove ? null : <MutationFeedback client={client} feedback={feedback} />}
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
            setFeedback({ busy: false, operationId: result.operationId, message: t("Package changes started.") });
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
            {feedback.error === undefined ? null : <div className="mutation-feedback mutation-feedback--error" role="alert"><strong>{t("Package changes were not applied")}</strong><span>{t(packageErrorMessage(feedback.error))}</span></div>}
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
    useUtilityDialogBusy(feedback.busy);
    const mutate = async (action: "register" | "refresh" | "remove") => {
        if (!available || pending.current || (action === "remove" && remove === undefined)) return;
        pending.current = true; setFeedback({ busy: true });
        try {
            if (action === "register") { await client.unityInstallationRegister(path); setPath(""); }
            else if (action === "refresh") await client.unityInstallationsRefresh();
            else { await client.unityInstallationRemove(remove!.installationId, remove!.revision); setRemove(undefined); setConfirmRemove(false); }
            setFeedback({ busy: false, message: action === "register" ? t("Unity installation registered.") : action === "refresh" ? t("Unity installations refreshed.") : t("Unity installation removed from the list.") });
            onChanged?.();
        } catch (caught: unknown) { setFeedback({ busy: false, error: safeError(caught) }); }
        finally { pending.current = false; }
    };
    return <ActionSection title={confirmRemove ? t("Remove Unity installation") : t("Manage Unity installations")}>
        {confirmRemove && remove !== undefined ? <>
            <p><Translated message="Remove {name} from the list?" values={{ name: <strong>Unity {remove.unityVersion}</strong> }} /></p>
            <p>{t("Only the ALCOMD registry entry is removed. The editor remains installed.")}</p>
            <div className="dialog-actions"><UtilityDialogCancel disabled={feedback.busy} /><Button disabled={feedback.busy} onClick={() => { setConfirmRemove(false); setFeedback(INITIAL_FEEDBACK); }} type="button" variant="text">{t("Back")}</Button><Button className="material-button--danger" disabled={!available || feedback.busy} onClick={() => void mutate("remove")} type="button">{t("Remove installation")}</Button></div>
        </> : <>
            <form onSubmit={(event) => { event.preventDefault(); void mutate("register"); }}>
                <TextField disabled={feedback.busy} id="unity-executable" label={t("Unity executable")} maxLength={1024} onInput={setPath} required value={path} />
                <div className="action-row"><Button disabled={!available || feedback.busy || path.trim().length === 0} title={capabilityUnavailableTitle(available, capabilities.unityManage, t)} type="submit">{t("Register")}</Button><Button disabled={!available || feedback.busy} onClick={() => void mutate("refresh")} title={capabilityUnavailableTitle(available, capabilities.unityManage, t)} type="button" variant="tonal">{t("Discover and refresh")}</Button></div>
            </form>
            {installations.length === 0 ? null : <Select aria-label={t("Installation to remove")} disabled={feedback.busy} label={t("Remove installation")} onChange={(next) => setRemove(installations.find((item) => item.installationId === next))} options={[{ label: t("Select an installation"), value: "" }, ...installations.map((item) => ({ label: "Unity " + item.unityVersion + " · " + item.architecture, value: item.installationId }))]} value={remove?.installationId ?? ""} />}
            <div className="dialog-actions"><UtilityDialogCancel disabled={feedback.busy} /><Button className="material-button--danger" disabled={!available || remove === undefined || feedback.busy} onClick={() => { setConfirmRemove(true); setFeedback(INITIAL_FEEDBACK); }} title={capabilityUnavailableTitle(available, capabilities.unityManage, t)} type="button" variant="text">{t("Review removal")}</Button></div>
        </>}
        <MutationFeedback client={client} feedback={feedback} />
    </ActionSection>;
}

export function ProjectLaunchArgumentsDialog({ client, launchConfig, onChanged, onClose, onSaved, projectId }: ActionProps & { launchConfig?: ProjectUnityLaunchConfig; onClose(): void; onSaved?(message: string): void; projectId: string }) {
    const { t } = useTranslation();
    const canManage = useCapability(capabilities.unityManage);
    const [open, setOpen] = useState(true);
    const initialConfig = useRef(launchConfig).current;
    const [config, setConfig] = useState<ProjectUnityLaunchConfig>();
    const [argumentsText, setArgumentsText] = useState("");
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const pending = useRef(false);
    useEffect(() => {
        let active = true;
        setConfig(undefined);
        setFeedback(INITIAL_FEEDBACK);
        const read = initialConfig === undefined ? client.unityProjectLaunchConfigGet(projectId).then((result) => result.config) : Promise.resolve(initialConfig);
        void read.then((value) => {
            if (!active) return;
            setConfig(value); setArgumentsText(value.arguments.join("\n"));
        }).catch((caught: unknown) => { if (active) setFeedback({ busy: false, error: safeError(caught) }); });
        return () => { active = false; };
    }, [client, initialConfig, projectId]);
    const close = () => { if (!pending.current) setOpen(false); };
    const save = async (clear: boolean) => {
        if (!canManage || config === undefined || pending.current) return;
        pending.current = true; setFeedback({ busy: true });
        try {
            if (clear) await client.unityProjectLaunchConfigClear(projectId, config.revision);
            else await client.unityProjectLaunchConfigSet(projectId, argumentsText.split(/\r?\n/).map((value) => value.trim()).filter(Boolean), config.revision);
            setFeedback(INITIAL_FEEDBACK); setOpen(false); onSaved?.(clear ? t("Unity launch arguments cleared.") : t("Unity launch arguments updated.")); onChanged?.();
        } catch (caught: unknown) { setFeedback({ busy: false, error: safeError(caught) }); }
        finally { pending.current = false; }
    };
    return <MaterialDialog dismissible={!feedback.busy} open={open} onClose={onClose} title={t("Unity launch arguments")}>
        {config === undefined ? feedback.error === undefined ? <LoadingState title={t("Loading launch arguments…")} /> : null : <form onSubmit={(event) => { event.preventDefault(); void save(false); }}>
            <TextField aria-describedby="unity-arguments-hint" disabled={feedback.busy} id="unity-arguments" label={t("Additional arguments")} maxLength={4096} onInput={setArgumentsText} rows={4} supportingText={t("One argument per line. The daemon validates forbidden arguments.")} type="textarea" value={argumentsText} />
            <div className="dialog-actions">
                <Button disabled={feedback.busy} onClick={close} type="button" variant="text">{t("Cancel")}</Button>
                <Button disabled={!canManage || feedback.busy || config.revision === 0} onClick={() => void save(true)} type="button" variant="text">{t("Clear launch arguments")}</Button>
                <Button disabled={!canManage || feedback.busy} type="submit">{t("Save launch arguments")}</Button>
            </div>
        </form>}
        {config === undefined ? <div className="dialog-actions"><Button onClick={close} type="button" variant="text">{t("Cancel")}</Button></div> : null}
        <MutationFeedback client={client} feedback={feedback} />
    </MaterialDialog>;
}

export function ProjectUnityActions({ afterMigrationOpen = false, client, installations, launchConfig, launchOptions, project, onChanged }: ActionProps & { afterMigrationOpen?: boolean; installations: UnityInstallation[]; launchConfig: ProjectUnityLaunchConfig; launchOptions: UnityLaunchOptionsResult; project: ProjectSnapshot }) {
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
    const launch = async () => {
        if (projectId === undefined) return;
        const effectiveOptions = postMigrationLaunchOptions ?? launchOptions;
        const candidates = effectiveOptions.exactMatchingInstallations;
        const installationId = candidates.length === 1 ? candidates[0]?.installationId : launchInstallationId;
        if (installationId === undefined || installationId.length === 0) {
            setFeedback({ busy: false, message: candidates.length === 0
                ? t("This project requires Unity {version}. No exact matching Unity installation was found.", { version: launchOptions.projectUnityVersion })
                : t("Choose the Unity installation to use for this launch.") });
            return;
        }
        setFeedback({ busy: true });
        try {
            const result = await client.unityLaunch(projectId, installationId, effectiveOptions.projectRevision);
            setFeedback({ busy: false, message: t("Unity launch {state}.", { state: t(result.launch.state) }) });
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
                setFeedback({ busy: false, message: t("Project already uses Unity {version}.", { version: result.currentVersion }) });
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
            setFeedback({ busy: false, message: t("Unity migration accepted."), operationId: result.operationId });
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
                setFeedback({ busy: false, message: t("Migration completed, but no exact Unity {version} installation is available.", { version: options.projectUnityVersion }) });
                return;
            }
            if (options.exactMatchingInstallations.length > 1) {
                setLaunchInstallationId("");
                setFeedback({ busy: false, message: t("Migration completed. Choose the Unity installation for this launch.") });
                return;
            }
            const result = await client.unityLaunch(projectId, options.exactMatchingInstallations[0]!.installationId, freshProject.revision);
            setFeedback({ busy: false, message: t("Migration completed. Unity launch {state}.", { state: t(result.launch.state) }) });
        } catch (caught: unknown) {
            setFeedback({ busy: false, error: safeError(caught) });
        }
    }, [afterMigrationOpen, client, onChanged, projectId, t]);
    const effectiveLaunchOptions = postMigrationLaunchOptions ?? launchOptions;
    return (
        <ActionSection title={t("Unity actions")}>
            <p><Translated message="Project Unity version: {version}" values={{ version: <strong>{project.unityVersion}</strong> }} /></p>
            {effectiveLaunchOptions.exactMatchingInstallations.length > 1 ? <Select id="launch-installation" label={t("Unity installation for this launch")} onChange={setLaunchInstallationId} options={[{ label: t("Choose an installation"), value: "" }, ...effectiveLaunchOptions.exactMatchingInstallations.map((item) => ({ label: `Unity ${item.unityVersion} · ${item.architecture}`, value: item.installationId }))]} value={launchInstallationId} /> : null}
            <Button disabled={!canLaunch || feedback.busy || projectRevision === undefined} onClick={() => void launch()} title={capabilityUnavailableTitle(canLaunch, capabilities.unityLaunch, t)} type="button">{t("Open Unity")}</Button>
            {effectiveLaunchOptions.exactMatchingInstallations.length === 0 ? <p>{t("This project requires Unity {version}. No exact matching Unity installation was found.", { version: effectiveLaunchOptions.projectUnityVersion })}</p> : null}
            <Button disabled={!canManage || feedback.busy || projectId === undefined} onClick={() => setArgumentsOpen(true)} title={capabilityUnavailableTitle(canManage, capabilities.unityManage, t)} type="button" variant="tonal">{t("Edit launch arguments")}</Button>
            {!argumentsOpen || projectId === undefined ? null : <ProjectLaunchArgumentsDialog client={client} launchConfig={launchConfig} onChanged={onChanged} onClose={() => setArgumentsOpen(false)} onSaved={(message) => setFeedback({ busy: false, message })} projectId={projectId} />}
            <h3>{t("Migrate Project Unity version")}</h3>
            <Select id="project-unity-version" label={t("Target Unity version")} onChange={chooseTargetVersion} options={[{ label: t("Current · {version}", { version: project.unityVersion }), value: "" }, ...migrationVersions.map((version) => ({ label: version, value: version }))]} value={targetVersion} />
            {targetInstallations.length > 1 ? <Select id="migration-installation" label={t("Target Unity installation")} onChange={setTargetInstallationId} options={[{ label: t("Choose an installation"), value: "" }, ...targetInstallations.map((item) => ({ label: `${item.unityVersion} · ${item.architecture}`, value: item.installationId }))]} value={targetInstallationId} /> : null}
            <Button disabled={!canMigrate || feedback.busy || targetInstallationId.length === 0 || projectRevision === undefined} onClick={() => void planMigration()} title={capabilityUnavailableTitle(canMigrate, capabilities.projectsUnityMigration, t)} type="button" variant="tonal">{t("Review migration")}</Button>
            <PlanDialog applyDisabled={migrationPlan?.classification.supportedForApply === false} busy={feedback.busy} onApply={applyMigration} onClose={() => setMigrationPlan(undefined)} open={migrationPlan !== undefined} title={t("Review Unity migration")}>
                {migrationPlan === undefined ? null : <dl className="dialog-summary"><div><dt>{t("From")}</dt><dd>{migrationPlan.sourceUnityVersion}</dd></div><div><dt>{t("To")}</dt><dd>{migrationPlan.targetUnityVersion}</dd></div><div><dt>{t("Classification")}</dt><dd>{t(humanize(migrationPlan.classification.kind))}</dd></div><div><dt>{t("Supported")}</dt><dd>{migrationPlan.classification.supportedForApply ? t("Yes") : t("No")}</dd></div></dl>}
            </PlanDialog>
            <MutationFeedback client={client} feedback={feedback} onOperationTerminal={migrationFinished} />
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
    const unityLaunch = useProjectUnityLaunch({ client, initialOptions: launchOptions, navigate, onFeedback: setFeedback, onLaunchSettled: onChanged, project });
    const [migrationVersion, setMigrationVersion] = useState(project.unityVersion);
    const [migrationInstallationId, setMigrationInstallationId] = useState("");
    const [migrationChooserOpen, setMigrationChooserOpen] = useState(false);
    const [migrationPlan, setMigrationPlan] = useState<ProjectUnityMigrationPlan>();
    const projectId = project.projectId;
    const projectRevision = project.revision;
    const migrationVersions = [...new Set(installations.map((installation) => installation.unityVersion))]
        .filter((version) => version !== project.unityVersion)
        .sort((left, right) => left.localeCompare(right));
    const migrationInstallations = installations.filter((installation) => installation.unityVersion === migrationVersion);

    const planMigration = async (installationId: string) => {
        if (projectId === undefined || projectRevision === undefined) return;
        setFeedback({ busy: true });
        try {
            const result = await client.projectPlanUnityMigration(projectId, installationId, projectRevision);
            setMigrationChooserOpen(false);
            if (result.kind === "no_change") {
                setMigrationVersion(project.unityVersion);
                setFeedback({ busy: false, message: t("Project already uses Unity {version}.", { version: result.currentVersion }) });
                return;
            }
            setMigrationPlan(result.plan);
            setFeedback({ busy: false });
        } catch (caught: unknown) {
            setMigrationVersion(project.unityVersion);
            setFeedback({ busy: false, error: safeError(caught) });
        }
    };
    const chooseMigrationVersion = (version: string) => {
        setMigrationVersion(version);
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
        if (migrationPlan === undefined || !migrationPlan.classification.supportedForApply) return;
        setFeedback({ busy: true });
        try {
            const result = await client.projectApplyUnityMigration(migrationPlan.planId);
            setMigrationPlan(undefined);
            navigate(`/operations/${result.operationId}`);
        } catch (caught: unknown) {
            setFeedback({ busy: false, error: safeError(caught) });
        }
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
            <ModalDialog onClose={() => { setMigrationChooserOpen(false); setMigrationVersion(project.unityVersion); }} open={migrationChooserOpen} title={t("Choose Unity {version} installation", { version: migrationVersion })}>
                <Select label={t("Target Unity installation")} onChange={setMigrationInstallationId} options={[{ label: t("Choose an installation"), value: "" }, ...migrationInstallations.map((item) => ({ label: `${item.unityVersion} · ${item.architecture}`, value: item.installationId }))]} value={migrationInstallationId} />
                <div className="dialog-actions">
                    <Button onClick={() => { setMigrationChooserOpen(false); setMigrationVersion(project.unityVersion); }} type="button" variant="tonal">{t("Cancel")}</Button>
                    <Button disabled={migrationInstallationId.length === 0 || feedback.busy} onClick={() => void planMigration(migrationInstallationId)} type="button">{t("Review migration")}</Button>
                </div>
            </ModalDialog>
            <PlanDialog applyDisabled={migrationPlan?.classification.supportedForApply === false} busy={feedback.busy} onApply={applyMigration} onClose={() => { setMigrationPlan(undefined); setMigrationVersion(project.unityVersion); }} open={migrationPlan !== undefined} title={t("Review Unity migration")}>
                {migrationPlan === undefined ? null : <dl className="dialog-summary"><div><dt>{t("From")}</dt><dd>{migrationPlan.sourceUnityVersion}</dd></div><div><dt>{t("To")}</dt><dd>{migrationPlan.targetUnityVersion}</dd></div><div><dt>{t("Classification")}</dt><dd>{t(humanize(migrationPlan.classification.kind))}</dd></div><div><dt>{t("Supported")}</dt><dd>{migrationPlan.classification.supportedForApply ? t("Yes") : t("No")}</dd></div></dl>}
            </PlanDialog>
            {feedback.error === undefined ? null : <span className="inline-error" role="alert">{t("Unity action failed: {code}", { code: feedback.error.code })}</span>}
            {feedback.message === undefined ? null : <span aria-live="polite" role="status">{feedback.message}</span>}
        </>
    );
}

export function TemplateImportPanel({ client, onChanged }: ActionProps) {
    const { t } = useTranslation();
    const available = useCapability(capabilities.templatesManage);
    const [bundlePath, setBundlePath] = useState("");
    const [overrideExisting, setOverrideExisting] = useState(false);
    const [plan, setPlan] = useState<TemplatePlan>();
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    useUtilityDialogBusy(feedback.busy);
    const pending = useRef(false);
    const create = async (event: FormEvent) => {
        event.preventDefault();
        if (!available || pending.current) return;
        pending.current = true; setFeedback({ busy: true });
        try {
            const inspected = await client.templateInspectBundle(bundlePath);
            let revision = 0;
            try { revision = (await client.templateGet(inspected.templateId)).template.revision; }
            catch (caught: unknown) { if (safeError(caught).code !== "template_not_found") throw caught; }
            setPlan(await client.templatePlanImport(bundlePath, overrideExisting, revision));
            setFeedback({ busy: false });
        } catch (caught: unknown) { setFeedback({ busy: false, error: safeError(caught) }); }
        finally { pending.current = false; }
    };
    const apply = async () => {
        if (plan === undefined || !available || pending.current) return;
        pending.current = true; setFeedback({ busy: true });
        try {
            const result = await client.templateApplyImport(plan.planId);
            setPlan(undefined); setBundlePath("");
            setFeedback({ busy: false, operationId: result.operationId, message: t("Template import accepted.") }); onChanged?.();
        } catch (caught: unknown) { setFeedback({ busy: false, error: safeError(caught) }); }
        finally { pending.current = false; }
    };
    return <ActionSection title={plan === undefined ? t("Import template") : t("Review template import")}>
        {plan === undefined ? <form onSubmit={(event) => void create(event)}>
            <TextField disabled={feedback.busy} id="template-bundle" label={t("Template bundle")} maxLength={1024} onInput={setBundlePath} required value={bundlePath} />
            <Checkbox checked={overrideExisting} disabled={feedback.busy} label={t("Replace an existing matching template")} onChange={setOverrideExisting} />
            <div className="dialog-actions"><UtilityDialogCancel disabled={feedback.busy} /><Button disabled={!available || feedback.busy || bundlePath.trim().length === 0} title={capabilityUnavailableTitle(available, capabilities.templatesManage, t)} type="submit">{t("Review import")}</Button></div>
        </form> : <>
            <p>{t("Action:")}<strong>{t(humanize(plan.action))}</strong></p>
            <p>{t("Plan fingerprint:")}<code>{shortValue(plan.planFingerprint)}</code></p>
            <p className="risk-summary">{t("The daemon will revalidate this frozen plan before importing.")}</p>
            <div className="dialog-actions"><UtilityDialogCancel disabled={feedback.busy} /><Button disabled={feedback.busy} onClick={() => { setPlan(undefined); setFeedback(INITIAL_FEEDBACK); }} type="button" variant="text">{t("Discard plan")}</Button><Button disabled={!available || feedback.busy} onClick={() => void apply()} type="button">{t("Apply reviewed plan")}</Button></div>
        </>}
        <MutationFeedback client={client} feedback={feedback} />
    </ActionSection>;
}

export function TemplateCreateActions({ client, onChanged }: ActionProps) {
    const { t } = useTranslation();
    const canManage = useCapability(capabilities.templatesManage);
    const [creating, setCreating] = useState(false);
    const [importing, setImporting] = useState(false);
    const menuAnchorRef = useRef<HTMLElement>(null);
    const primaryRef = useRef<HTMLElement>(null);
    return <>
        <SplitButton disabled={!canManage} label={t("Create template")} menuButtonRef={menuAnchorRef} menuLabel={t("More template creation options")} onClick={() => setCreating(true)} primaryRef={primaryRef} title={capabilityUnavailableTitle(canManage, capabilities.templatesManage, t)}>
            <MenuItem disabled={!canManage} label={t("Import template")} onClick={() => setImporting(true)} title={capabilityUnavailableTitle(canManage, capabilities.templatesManage, t)} />
        </SplitButton>
        {creating ? <TemplateDeriveDialog client={client} onChanged={onChanged} onClose={() => { setCreating(false); window.requestAnimationFrame(() => primaryRef.current?.focus()); }} /> : null}
        {importing ? <UtilityActionDialog title={t("Import template")} onClose={() => { setImporting(false); window.requestAnimationFrame(() => menuAnchorRef.current?.focus()); }}><TemplateImportPanel client={client} onChanged={onChanged} /></UtilityActionDialog> : null}
    </>;
}

function TemplateDeriveDialog({ client, onChanged, onClose }: ActionProps & { onClose(): void }) {
    const { t } = useTranslation();
    const canManage = useCapability(capabilities.templatesManage);
    const [open, setOpen] = useState(true);
    const [plan, setPlan] = useState<TemplatePlan>();
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const [projects, setProjects] = useState<ProjectSnapshot[]>([]);
    const [projectPage, setProjectPage] = useState<Awaited<ReturnType<GuiRpcClient["projectsList"]>>>();
    const [projectsLoading, setProjectsLoading] = useState(true);
    const showProjectsLoading = useDelayedPending(projectsLoading && projects.length === 0);
    const [fields, setFields] = useState({ projectId: "", templateId: "", templateVersion: "1.0.0", displayName: "", description: "" });
    const pending = useRef(false);
    const projectLoad = useRef(0);
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
            setPlan(await client.templatePlanDerive({ projectId: source.projectId, expectedProjectRevision: source.revision, templateId: fields.templateId, templateVersion: fields.templateVersion, displayName: fields.displayName, ...(fields.description.length === 0 ? {} : { description: fields.description }) }));
            setFeedback({ busy: false });
        } catch (caught: unknown) { setFeedback({ busy: false, error: safeError(caught) }); }
        finally { pending.current = false; }
    };
    const apply = async () => {
        if (plan === undefined || pending.current || !canManage) return;
        pending.current = true; setFeedback({ busy: true });
        try {
            const result = await client.templateApplyDerive(plan.planId);
            setPlan(undefined); setFeedback({ busy: false, operationId: result.operationId, message: t("Template operation accepted.") }); onChanged?.();
        } catch (caught: unknown) { setFeedback({ busy: false, error: safeError(caught) }); }
        finally { pending.current = false; }
    };
    const close = () => { if (!pending.current) setOpen(false); };
    return <MaterialDialog dismissible={!feedback.busy} open={open} onClose={onClose} title={plan === undefined ? t("Create template from project") : t("Review new template")}>
        {feedback.operationId !== undefined ? <div className="dialog-actions"><Button onClick={close} type="button" variant="text">{t("Close")}</Button></div> : plan !== undefined ? <>
            <p>{t("Action:")}<strong>{t(humanize(plan.action))}</strong></p>
            <p>{t("Plan fingerprint:")}<code>{shortValue(plan.planFingerprint)}</code></p>
            <p className="risk-summary">{t("The daemon will revalidate this frozen plan before applying it.")}</p>
            <div className="dialog-actions"><Button disabled={feedback.busy} onClick={close} type="button" variant="text">{t("Discard plan")}</Button><Button disabled={feedback.busy || !canManage} onClick={() => void apply()} type="button">{t("Apply reviewed plan")}</Button></div>
        </> : <form onSubmit={(event) => void createPlan(event)}>
            <Select id="derive-project" label={t("Source project")} disabled={feedback.busy || projectsLoading} onChange={(next) => update("projectId", next)} options={[{ label: t("Choose a project"), value: "" }, ...projects.filter((project) => project.projectId !== undefined).map((project) => ({ label: project.rootPath, value: project.projectId! }))]} value={fields.projectId} />
            {showProjectsLoading ? <p role="status">{t("Loading projects…")}</p> : !projectsLoading && projects.length === 0 ? <p>{t("No registered projects available.")}</p> : null}
            {projectPage?.nextCursor === undefined ? null : <Button disabled={projectsLoading || feedback.busy} onClick={() => void loadMore()} type="button" variant="text">{t("Load more projects")}</Button>}
            <TextField disabled={feedback.busy} id="derive-name" label={t("Display name")} onInput={(next) => update("displayName", next)} required value={fields.displayName} />
            <TextField disabled={feedback.busy} id="derive-template-id" label={t("New template ID")} onInput={(next) => update("templateId", next)} required value={fields.templateId} />
            <TextField disabled={feedback.busy} id="derive-version" label={t("Template version")} onInput={(next) => update("templateVersion", next)} required value={fields.templateVersion} />
            <TextField disabled={feedback.busy} id="derive-description" label={t("Description")} onInput={(next) => update("description", next)} value={fields.description} />
            <div className="dialog-actions"><Button disabled={feedback.busy} onClick={close} type="button" variant="text">{t("Cancel")}</Button><Button disabled={feedback.busy || !canManage || fields.projectId.length === 0} type="submit">{t("Review")}</Button></div>
        </form>}
        <MutationFeedback client={client} feedback={feedback} />
    </MaterialDialog>;
}

export function TemplateActions({ client, compact = false, onChanged, onView, template }: ActionProps & { compact?: boolean; onView?(): void; template: TemplateRecord }) {
    const { t } = useTranslation();
    const canCreateProject = useCapability(capabilities.templatesCreateProject);
    const canManage = useCapability(capabilities.templatesManage);
    const [mode, setMode] = useState<"none" | "derive" | "create" | "export" | "remove">("none");
    const [plan, setPlan] = useState<TemplatePlan>();
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const [fields, setFields] = useState({ parent: "", leaf: "", exportPath: "" });
    const [menuOpen, setMenuOpen] = useState(false);
    const menuAnchorRef = useRef<HTMLElement>(null);
    const pending = useRef(false);
    const update = (key: keyof typeof fields, value: string) => setFields((current) => ({ ...current, [key]: value }));
    const close = () => {
        if (pending.current) return;
        setMode("none"); setPlan(undefined);
    };
    const open = (next: typeof mode) => {
        if (pending.current) return;
        setMenuOpen(false); setPlan(undefined); setFeedback(INITIAL_FEEDBACK); setMode(next);
        setFields({ parent: "", leaf: "", exportPath: "" });
    };
    const createPlan = async (event: FormEvent) => {
        event.preventDefault();
        if (pending.current || !canCreateProject) return;
        pending.current = true; setFeedback({ busy: true });
        try {
            const next = await client.templatePlanCreateProject(template.templateId, template.revision, fields.parent, fields.leaf);
            setPlan(next); setFeedback({ busy: false });
        } catch (caught: unknown) { setFeedback({ busy: false, error: safeError(caught) }); }
        finally { pending.current = false; }
    };
    const apply = async () => {
        if (plan === undefined || pending.current || !canCreateProject) return;
        pending.current = true; setFeedback({ busy: true });
        try {
            const result = await client.templateApplyCreateProject(plan.planId);
            setPlan(undefined); setMode("none");
            setFeedback({ busy: false, operationId: result.operationId, message: t("Template operation accepted.") }); onChanged?.();
        } catch (caught: unknown) { setFeedback({ busy: false, error: safeError(caught) }); }
        finally { pending.current = false; }
    };
    const simple = async (action: "favorite" | "export" | "remove") => {
        if (pending.current || !canManage) return;
        pending.current = true; setFeedback({ busy: true });
        try {
            if (action === "favorite") await client.templateSetFavorite(template.templateId, !template.favorite, template.revision);
            else if (action === "export") await client.templateExport(template.templateId, template.revision, fields.exportPath);
            else await client.templateRemove(template.templateId, template.revision);
            setMode("none");
            setFeedback({ busy: false, message: action === "favorite" ? t("Favorite updated.") : action === "export" ? t("Template exported.") : t("Template removed.") });
            if (action !== "export") onChanged?.();
        } catch (caught: unknown) { setFeedback({ busy: false, error: safeError(caught) }); }
        finally { pending.current = false; }
    };
    const pickParent = async () => {
        try { const path = await client.selectDirectory(); if (path !== undefined) update("parent", path); }
        catch (caught: unknown) { setFeedback({ busy: false, error: safeError(caught) }); }
    };
    const title = plan !== undefined ? t("Review project creation") : mode === "create" ? t("Create project") : mode === "export" ? t("Export template") : t("Remove template");
    return <section className={compact ? "template-row-actions" : "action-section"}>
        {compact ? <div className="card-actions">
            <IconButton aria-pressed={template.favorite} className="project-favorite-action" disabled={!canManage || feedback.busy} label={template.favorite ? t("Remove {name} from favorites", { name: template.displayName }) : t("Favorite {name}", { name: template.displayName })} onClick={() => void simple("favorite")} type="button"><Icon asset={starIcon} /></IconButton>
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
            <Button widthLabels={[t("Remove favorite"), t("Favorite")]} disabled={!canManage || feedback.busy} onClick={() => void simple("favorite")} type="button" variant="text">{template.favorite ? t("Remove favorite") : t("Favorite")}</Button>
            <Button disabled={!canManage || feedback.busy} onClick={() => open("export")} type="button" variant="text">{t("Export")}</Button>
            {template.sourceKind === "builtin" ? null : <Button className="material-button--danger" disabled={!canManage || feedback.busy} onClick={() => open("remove")} type="button" variant="text">{t("Remove template")}</Button>}
        </div></>}
        {mode === "derive" ? <TemplateDeriveDialog client={client} onChanged={onChanged} onClose={close} /> : null}
        <MaterialDialog dismissible={!feedback.busy} open={mode !== "none" && mode !== "derive"} onClose={close} title={title}>
            {plan !== undefined ? <>
                <p>{t("Action:")}<strong>{t(humanize(plan.action))}</strong></p>
                <p>{t("Plan fingerprint:")}<code>{shortValue(plan.planFingerprint)}</code></p>
                <p className="risk-summary">{t("The daemon will revalidate this frozen plan before applying it.")}</p>
                <div className="dialog-actions"><Button disabled={feedback.busy} onClick={close} type="button" variant="text">{t("Discard plan")}</Button><Button disabled={feedback.busy || !canCreateProject} onClick={() => void apply()} type="button">{t("Apply reviewed plan")}</Button></div>
            </> : mode === "create" ? <form onSubmit={(event) => void createPlan(event)}>
                <p>{t("Template:")}<strong>{template.displayName}</strong></p>
                <TextField disabled={feedback.busy} id="create-parent" label={t("Target parent")} onInput={(next) => update("parent", next)} required value={fields.parent} />
                <Button disabled={feedback.busy} onClick={() => void pickParent()} type="button" variant="text">{t("Browse…")}</Button>
                <TextField disabled={feedback.busy} id="create-leaf" label={t("Project folder name")} onInput={(next) => update("leaf", next)} required value={fields.leaf} />
                <div className="dialog-actions"><Button disabled={feedback.busy} onClick={close} type="button" variant="text">{t("Cancel")}</Button><Button disabled={feedback.busy || !canCreateProject} type="submit">{t("Review")}</Button></div>
            </form> : mode === "export" ? <form onSubmit={(event) => { event.preventDefault(); void simple("export"); }}>
                <p><Translated message="Export {name} as a template bundle." values={{ name: <strong>{template.displayName}</strong> }} /></p>
                <TextField disabled={feedback.busy} id="template-export" label={t("Export target")} onInput={(next) => update("exportPath", next)} required value={fields.exportPath} />
                <div className="dialog-actions"><Button disabled={feedback.busy} onClick={close} type="button" variant="text">{t("Cancel")}</Button><Button disabled={!canManage || feedback.busy} type="submit">{t("Export")}</Button></div>
            </form> : mode === "remove" ? <>
                <p><Translated message="Remove {name} from your templates?" values={{ name: <strong>{template.displayName}</strong> }} /></p>
                <div className="dialog-actions"><Button disabled={feedback.busy} onClick={close} type="button" variant="text">{t("Cancel")}</Button><Button className="material-button--danger" disabled={!canManage || feedback.busy} onClick={() => void simple("remove")} type="button">{t("Remove template")}</Button></div>
            </> : null}
            <MutationFeedback client={client} feedback={feedback} />
        </MaterialDialog>
        {mode === "none" ? <MutationFeedback client={client} feedback={feedback} /> : null}
    </section>;
}

export function BackupCreatePanel({ client, onChanged, project }: ActionProps & { project: ProjectSnapshot }) {
    const { t } = useTranslation();
    const available = useCapability(capabilities.backupsCreate);
    const [compression, setCompression] = useState<"store" | "fast" | "maximum">("fast");
    const [exclude, setExclude] = useState(true);
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const pending = useRef(false);
    useUtilityDialogBusy(feedback.busy);
    const create = async () => {
        if (!available || pending.current || project.projectId === undefined || project.revision === undefined) return;
        pending.current = true; setFeedback({ busy: true });
        try { const result = await client.backupCreate(project.projectId, project.revision, compression, exclude); setFeedback({ busy: false, operationId: result.operationId }); onChanged?.(); }
        catch (caught: unknown) { setFeedback({ busy: false, error: safeError(caught) }); }
        finally { pending.current = false; }
    };
    return <ActionSection title={t("Create backup")}>
        <p><Translated message="Back up {name}." values={{ name: <strong>{project.rootPath.split(/[\\/]/).filter(Boolean).at(-1) ?? project.projectId}</strong> }} /></p>
        {feedback.operationId === undefined ? <><Select disabled={feedback.busy} id="backup-compression" label={t("Compression")} onChange={(next) => setCompression(next as typeof compression)} options={[{ label: t("Store"), value: "store" }, { label: t("Fast"), value: "fast" }, { label: t("Maximum"), value: "maximum" }]} value={compression} /><Checkbox disabled={feedback.busy} checked={exclude} label={t("Exclude VPM packages")} onChange={setExclude} /><div className="dialog-actions"><UtilityDialogCancel disabled={feedback.busy} /><Button disabled={!available || feedback.busy || project.revision === undefined} onClick={() => void create()} type="button">{t("Create backup")}</Button></div></> : <p>{t("Closing this dialog does not cancel the backup task.")}</p>}
        <MutationFeedback client={client} feedback={feedback} />
    </ActionSection>;
}

export function BackupRestorePanel({ backup, client }: ActionProps & { backup: BackupRecord }) {
    const { t } = useTranslation();
    const available = useCapability(capabilities.backupsRestore);
    const [parent, setParent] = useState("");
    const [leaf, setLeaf] = useState("");
    const [plan, setPlan] = useState<BackupRestorePlan>();
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const pending = useRef(false);
    useUtilityDialogBusy(feedback.busy);
    const run = async (apply: boolean) => {
        if (!available || pending.current || (apply && plan === undefined)) return;
        pending.current = true; setFeedback({ busy: true });
        try {
            if (apply) { const result = await client.backupApplyRestore(plan!.planId); setPlan(undefined); setFeedback({ busy: false, operationId: result.operationId }); }
            else { setPlan(await client.backupPlanRestore(backup.backupId, parent, leaf)); setFeedback({ busy: false }); }
        } catch (caught: unknown) { setFeedback({ busy: false, error: safeError(caught) }); }
        finally { pending.current = false; }
    };
    return <ActionSection title={t("Restore backup")}>
        {feedback.operationId !== undefined ? <p>{t("Closing this dialog does not cancel the restore task.")}</p> : plan === undefined ? <form onSubmit={(event) => { event.preventDefault(); void run(false); }}>
            <TextField disabled={feedback.busy} id="restore-parent" label={t("Target parent")} maxLength={1024} onInput={setParent} required value={parent} />
            <Button disabled={feedback.busy} onClick={() => void client.selectDirectory().then((path) => { if (path !== undefined) setParent(path); }).catch((caught: unknown) => setFeedback({ busy: false, error: safeError(caught) }))} type="button" variant="text">{t("Browse…")}</Button>
            <TextField disabled={feedback.busy} id="restore-leaf" label={t("New directory name")} maxLength={255} onInput={setLeaf} required value={leaf} />
            <div className="dialog-actions"><UtilityDialogCancel disabled={feedback.busy} /><Button disabled={!available || feedback.busy} type="submit">{t("Review backup restore")}</Button></div>
        </form> : <>
            <h3>{t("Review backup restore")}</h3><dl className="dialog-summary"><div><dt>{t("Target parent")}</dt><dd>{plan.target.parent}</dd></div><div><dt>{t("New directory")}</dt><dd>{plan.target.leaf}</dd></div><div><dt>{t("Target must be absent")}</dt><dd>{plan.target.mustBeAbsent ? t("Yes") : t("No")}</dd></div></dl>
            <p>{plan.packagesRequireResolve ? t("VPM packages require a separate resolve after restoration.") : t("No package resolve is required.")}</p>
            <div className="dialog-actions"><UtilityDialogCancel disabled={feedback.busy} /><Button disabled={feedback.busy} onClick={() => setPlan(undefined)} type="button" variant="text">{t("Back")}</Button><Button disabled={!available || feedback.busy} onClick={() => void run(true)} type="button">{t("Apply reviewed plan")}</Button></div>
        </>}
        <MutationFeedback client={client} feedback={feedback} />
    </ActionSection>;
}

export function OperationActions({ client, operation, onChanged }: ActionProps & { operation: Operation }) {
    const { t } = useTranslation();
    const available = useCapability(capabilities.operations);
    const [confirm, setConfirm] = useState(false);
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const terminal = isTerminal(operation.state);
    const cancel = async () => { await runSimple(setFeedback, () => client.operationCancel(operation.operationId, operation.revision), t("Cancellation requested."), onChanged); };
    return <ActionSection title={t("Operation control")}><Button className="material-button--danger" disabled={!available || terminal || feedback.busy} onClick={() => setConfirm(true)} title={capabilityUnavailableTitle(available, capabilities.operations, t)} type="button" variant="text">{t("Cancel operation")}</Button><ConfirmDialog busy={feedback.busy || !available} open={confirm} title={t("Request cancellation?")} detail={t("Cancellation is cooperative. The operation remains authoritative until the daemon reports a terminal state.")} onClose={() => setConfirm(false)} onConfirm={cancel} /><MutationFeedback client={client} feedback={feedback} /></ActionSection>;
}

export function ExtensionInstallPanel({ client, onChanged }: ActionProps) {
    const { t } = useTranslation();
    const available = useCapability(capabilities.extensionsLifecycle);
    const [path, setPath] = useState("");
    const [approvePublisher, setApprovePublisher] = useState(false);
    const [expectedRevision, setExpectedRevision] = useState("");
    const [plan, setPlan] = useState<ExtensionPlan>();
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    useUtilityDialogBusy(feedback.busy);
    const pending = useRef(false);
    const revisionValid = /^\d+$/.test(expectedRevision) && Number.isSafeInteger(Number(expectedRevision));
    const run = async (apply: boolean) => {
        if (!available || pending.current || (apply ? plan === undefined : !revisionValid || path.trim().length === 0)) return;
        pending.current = true;
        setFeedback({ busy: true });

        try {
            if (apply && plan !== undefined) {
                const result = await client.extensionApplyInstall(plan.planId);
                setPlan(undefined);
                setFeedback({ busy: false, operationId: result.operationId, message: t("Extension install accepted.") });
                onChanged?.();
            } else {
                const result = await client.extensionPlanInstall(path, Number(expectedRevision), approvePublisher ? "approve_for_extension" : "none");
                setPlan(result.plan);
                setFeedback({ busy: false });
            }
        } catch (caught: unknown) { setFeedback({ busy: false, error: safeError(caught) }); }
        finally { pending.current = false; }
    };
    return <section aria-label={plan === undefined ? t("Extension install input") : t("Review extension install")} aria-busy={feedback.busy}>
        {plan === undefined ? <form onSubmit={(event) => { event.preventDefault(); void run(false); }}>
            <TextField disabled={feedback.busy} id="extension-package" label={t("Extension package")} maxLength={1024} onInput={setPath} required value={path} />
            <p className="risk-summary">{t("Transitional input: the current registry read interface does not provide its revision. Enter the verified registry revision; it is never inferred from installed extensions.")}</p>
            <TextField disabled={feedback.busy} id="extension-registry-revision" label={t("Expected registry revision")} min={0} onInput={setExpectedRevision} required type="number" value={expectedRevision} />
            <Checkbox disabled={feedback.busy} checked={approvePublisher} label={t("Approve this publisher for this extension")} onChange={setApprovePublisher} />
            <div className="dialog-actions"><UtilityDialogCancel disabled={feedback.busy} /><Button disabled={!available || feedback.busy || !revisionValid} title={capabilityUnavailableTitle(available, capabilities.extensionsLifecycle, t)} type="submit">{t("Create install plan")}</Button></div>
        </form> : <>
            <h2>{t("Review extension install")}</h2>
            <dl className="dialog-summary"><div><dt>{t("Extension")}</dt><dd>{plan.extensionId}</dd></div><div><dt>{t("Version")}</dt><dd>{plan.version}</dd></div><div><dt>{t("Publisher")}</dt><dd><code>{plan.publisherFingerprint}</code></dd></div><div><dt>{t("Trust")}</dt><dd>{t(humanize(plan.trustDecision))}</dd></div></dl>
            <p>{t("The daemon will revalidate this install plan before applying it.")}</p>
            <div className="dialog-actions"><UtilityDialogCancel disabled={feedback.busy} /><Button disabled={feedback.busy} onClick={() => { setPlan(undefined); setFeedback(INITIAL_FEEDBACK); }} type="button" variant="text">{t("Discard plan")}</Button><Button disabled={!available || feedback.busy} onClick={() => void run(true)} type="button">{t("Apply reviewed plan")}</Button></div>
        </>}
        <MutationFeedback client={client} feedback={feedback} />
    </section>;
}

export function ExtensionActions({ client, extension, onChanged }: ActionProps & { extension: ExtensionRecord }) {
    const { t } = useTranslation();
    const canManageLifecycle = useCapability(capabilities.extensionsLifecycle);
    const canManagePermissions = useCapability(capabilities.extensionsPermissions);
    const [mode, setMode] = useState<"none" | "permissions" | "uninstall">("none");
    const [plan, setPlan] = useState<ExtensionPlan>();
    const [deleteData, setDeleteData] = useState(false);
    const [grant, setGrant] = useState({ permission: "", resourceKind: "Project", resourceId: "" });
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const pending = useRef(false);
    const close = () => { if (!pending.current) { setMode("none"); setPlan(undefined); setFeedback(INITIAL_FEEDBACK); } };
    const open = (next: "permissions" | "uninstall") => { if (!pending.current) { setMode(next); setPlan(undefined); setDeleteData(false); setGrant({ permission: "", resourceKind: "Project", resourceId: "" }); setFeedback(INITIAL_FEEDBACK); } };
    const run = async (kind: "enable" | "disable" | "plan" | "apply" | "grant" | "revoke") => {
        if (pending.current || ((kind === "grant" || kind === "revoke") ? !canManagePermissions : !canManageLifecycle)) return;
        if (kind === "apply" && plan === undefined) return;
        pending.current = true;
        setFeedback({ busy: true });
        try {
            if (kind === "plan") {
                const result = await client.extensionPlanUninstall(extension.extensionId, extension.revision, deleteData ? "delete_data" : "retain_data");
                setPlan(result.plan);
                setFeedback({ busy: false });
            } else if (kind === "apply" && plan !== undefined) {
                const result = await client.extensionApplyUninstall(plan.planId);
                setPlan(undefined);
                setMode("none");
                setFeedback({ busy: false, operationId: result.operationId, message: t("Extension uninstall accepted.") });
                onChanged?.();
            } else if (kind === "grant" || kind === "revoke") {
                await (kind === "grant" ? client.extensionSetGrant(extension.extensionId, grant.permission, grant.resourceKind, grant.resourceId, extension.grantRevision) : client.extensionRevokeGrant(extension.extensionId, grant.permission, grant.resourceKind, grant.resourceId, extension.grantRevision));
                setFeedback({ busy: false, message: kind === "grant" ? t("Permission granted.") : t("Permission revoked.") });
                onChanged?.();
            } else if (kind === "enable" || kind === "disable") {
                await (kind === "enable" ? client.extensionEnable(extension.extensionId, extension.revision) : client.extensionDisable(extension.extensionId, extension.revision));
                setFeedback({ busy: false, message: kind === "enable" ? t("Extension enabled.") : t("Extension disabled.") });
                onChanged?.();
            }
        } catch (caught: unknown) { setFeedback({ busy: false, error: safeError(caught) }); }
        finally { pending.current = false; }
    };
    const blocked = extension.desiredState === "uninstalling" || extension.quarantineState === "quarantined";
    return <ActionSection title={t("Extension management")}>
        <div className="action-row"><Button disabled={!canManageLifecycle || feedback.busy || blocked || extension.desiredState === "enabled"} onClick={() => void run("enable")} type="button" variant="tonal">{t("Enable")}</Button><Button disabled={!canManageLifecycle || feedback.busy || blocked || extension.desiredState !== "enabled"} onClick={() => void run("disable")} type="button" variant="tonal">{t("Disable")}</Button><Button disabled={feedback.busy} onClick={() => open("permissions")} type="button" variant="text">{t("Manage permissions")}</Button><Button className="material-button--danger" disabled={!canManageLifecycle || feedback.busy || extension.desiredState === "uninstalling"} onClick={() => open("uninstall")} type="button" variant="text">{t("Uninstall extension")}</Button></div>
        <MaterialDialog dismissible={!feedback.busy} open={mode !== "none"} onClose={close} title={mode === "permissions" ? t("Extension permissions") : plan === undefined ? t("Uninstall extension") : t("Review extension uninstall")}>
            <p><strong>{extension.extensionId}</strong></p>
            {mode === "permissions" ? <>
                <p>{t("Specify the exact permission and resource scope. Changes take effect only after Grant or Revoke.")}</p>
                <TextField disabled={feedback.busy} id="grant-permission" label={t("Permission")} onInput={(permission) => setGrant((current) => ({ ...current, permission }))} required value={grant.permission} />
                <Select disabled={feedback.busy} id="grant-kind" label={t("Resource kind")} onChange={(resourceKind) => setGrant((current) => ({ ...current, resourceKind }))} options={[{ label: t("Project"), value: "Project" }, { label: t("Extension"), value: "Extension" }]} value={grant.resourceKind} />
                <TextField disabled={feedback.busy} id="grant-resource" label={t("Resource ID")} onInput={(resourceId) => setGrant((current) => ({ ...current, resourceId }))} required value={grant.resourceId} />
                <div className="dialog-actions"><Button disabled={feedback.busy} onClick={close} type="button" variant="text">{t("Close")}</Button><Button disabled={!canManagePermissions || feedback.busy || !grant.permission.trim() || !grant.resourceId.trim()} onClick={() => void run("revoke")} type="button" variant="text">{t("Revoke")}</Button><Button disabled={!canManagePermissions || feedback.busy || !grant.permission.trim() || !grant.resourceId.trim()} onClick={() => void run("grant")} type="button">{t("Grant")}</Button></div>
            </> : plan === undefined ? <>
                <p>{t("Review the removal before applying it.")}</p>
                <Checkbox disabled={feedback.busy} checked={deleteData} label={t("Delete extension-owned data during uninstall")} onChange={setDeleteData} />
                <div className="dialog-actions"><Button disabled={feedback.busy} onClick={close} type="button" variant="text">{t("Cancel")}</Button><Button disabled={!canManageLifecycle || feedback.busy} onClick={() => void run("plan")} type="button">{t("Create uninstall plan")}</Button></div>
            </> : <>
                <dl className="dialog-summary"><div><dt>{t("Extension")}</dt><dd>{plan.extensionId}</dd></div><div><dt>{t("Version")}</dt><dd>{plan.version}</dd></div><div><dt>{t("Data")}</dt><dd>{t(humanize(plan.dataDisposition))}</dd></div><div><dt>{t("Publisher")}</dt><dd><code>{plan.publisherFingerprint}</code></dd></div></dl>
                <p>{t("The daemon will revalidate this uninstall plan before applying it.")}</p>
                <div className="dialog-actions"><Button disabled={feedback.busy} onClick={close} type="button" variant="text">{t("Discard plan")}</Button><Button disabled={!canManageLifecycle || feedback.busy} onClick={() => void run("apply")} type="button">{t("Apply reviewed plan")}</Button></div>
            </>}
            <MutationFeedback client={client} feedback={feedback} />
        </MaterialDialog>
        {mode === "none" ? <MutationFeedback client={client} feedback={feedback} /> : null}
    </ActionSection>;
}

function ActionSection({ children, title }: { children: ReactNode; title: string }) {
    return <section className="action-section"><h2>{title}</h2>{children}</section>;
}

function ConfirmDialog({ busy, detail, onClose, onConfirm, open, title }: { busy: boolean; detail: string; onClose(): void; onConfirm(): Promise<void>; open: boolean; title: string }) {
    const { t } = useTranslation();
    return <ModalDialog open={open} title={title} onClose={onClose}><p>{detail}</p><div className="dialog-actions"><Button disabled={busy} onClick={onClose} type="button" variant="tonal">{t("Go back")}</Button><Button data-dialog-initial-focus disabled={busy} onClick={() => void onConfirm().finally(onClose)} type="button">{t("Confirm")}</Button></div></ModalDialog>;
}

function PlanDialog({ applyDisabled = false, busy, children, onApply, onClose, open, title }: { applyDisabled?: boolean; busy: boolean; children: ReactNode; onApply(): Promise<void>; onClose(): void; open: boolean; title: string }) {
    const { t } = useTranslation();
    return <ModalDialog open={open} title={title} onClose={onClose}>{children}<p className="risk-summary">{t("The daemon will revalidate this frozen plan. A stale plan fails instead of being silently replaced.")}</p><div className="dialog-actions"><Button disabled={busy} onClick={onClose} type="button" variant="tonal">{t("Discard plan")}</Button><Button data-dialog-initial-focus disabled={busy || applyDisabled} onClick={() => void onApply()} type="button">{t("Apply reviewed plan")}</Button></div></ModalDialog>;
}

function ModalDialog({ children, onClose, open, title }: { children: ReactNode; onClose(): void; open: boolean; title: string }) {
    return <MaterialDialog onClose={onClose} open={open} title={title}>{children}</MaterialDialog>;
}

function MutationFeedback({ client, feedback, onOperationTerminal }: { client: GuiRpcClient; feedback: FeedbackState; onOperationTerminal?(operation: Operation): void }) {
    const { t } = useTranslation();
    if (feedback.error !== undefined) return <div className="mutation-feedback mutation-feedback--error" role="alert"><strong>{t("Request failed")}</strong><span><code>{feedback.error.code}</code>{feedback.error.diagnosticId === undefined ? "" : t(" · Diagnostic ID {id}", { id: feedback.error.diagnosticId })}</span></div>;
    if (feedback.operationId !== undefined) return <OperationFollow client={client} onTerminal={onOperationTerminal} operationId={feedback.operationId} />;
    if (feedback.message !== undefined) return <div className="mutation-feedback" role="status" aria-live="polite">{feedback.message}</div>;
    return null;
}

export function OperationFollow({ client, onTerminal, operationId, title = "Operation" }: { client: GuiRpcClient; onTerminal?(operation: Operation): void; operationId: string; title?: string }) {
    const { t } = useTranslation();
    const [operation, setOperation] = useState<Operation>();
    const [error, setError] = useState<RpcError>();
    const terminalNotification = useRef<string | undefined>(undefined);
    const load = useCallback(async () => {
        try { setOperation(await client.operationGet(operationId)); setError(undefined); }
        catch (caught: unknown) { setError(safeError(caught)); }
    }, [client, operationId]);
    useEffect(() => { void load(); }, [load]);
    useEffect(() => {
        if (operation === undefined || isTerminal(operation.state)) return;
        const timer = window.setTimeout(() => void load(), 750);
        return () => window.clearTimeout(timer);
    }, [load, operation]);
    useEffect(() => {
        if (operation === undefined || !isTerminal(operation.state)) return;
        const notification = `${operation.operationId}:${operation.revision}:${operation.state}`;
        if (terminalNotification.current === notification) return;
        terminalNotification.current = notification;
        onTerminal?.(operation);
    }, [onTerminal, operation]);
    return <section className="operation-follow" aria-live="polite" aria-atomic="true" role="status"><h3>{t(title)}</h3>{operation === undefined ? <p>{t("Reading progress…")}</p> : <><p><strong>{t(humanize(operation.state))}</strong>{operation.progress?.phase === undefined ? "" : ` · ${t(humanize(operation.progress.phase))}`}</p><Progress label={t("{title} progress", { title: t(title) })} value={isTerminal(operation.state) ? 1 : undefined} /></>}{error === undefined ? null : <p className="inline-error">{t("Progress is temporarily unavailable. ALCOMD will keep following the same task.")}</p>}</section>;
}

async function runSimple<T>(setFeedback: (value: FeedbackState) => void, run: () => Promise<T>, message: string, onChanged?: () => void): Promise<void> {
    setFeedback({ busy: true });
    try { await run(); setFeedback({ busy: false, message }); onChanged?.(); }
    catch (caught: unknown) { setFeedback({ busy: false, error: safeError(caught) }); }
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
    return "ALCOMD could not complete the package changes. Try again or open Logs for more information.";
}
function shortValue(value: string): string { return value.length <= 28 ? value : `${value.slice(0, 20)}…${value.slice(-8)}`; }
function isTerminal(state: string): boolean { return ["succeeded", "failed", "cancelled"].includes(state); }
