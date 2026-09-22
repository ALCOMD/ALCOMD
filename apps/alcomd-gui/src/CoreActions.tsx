import type { ExtensionRecord, RpcError } from "@alcomd/sdk";
import { queryCandidates } from "./package-candidates";
import { useUtilityDialogBusy, UtilityDialogCancel } from "./UtilityActionDialog";
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
import { moreVertIcon, playArrowIcon, starIcon } from "@alcomd/ui/icons";
import { Button, Checkbox, Dialog as MaterialDialog, Icon, IconButton, Menu, MenuItem, Progress, Select, TextField } from "./Material";
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
    const available = useCapability(capabilities.projectsRegistry);
    const [path, setPath] = useState("");
    const [confirm, setConfirm] = useState(false);
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const run = async () => {
        setFeedback({ busy: true });
        try {
            const result = await client.projectRegister(path);
            setFeedback({ busy: false, message: `Project registered at revision ${result.project.revision ?? "unknown"}.` });
            setPath("");
            onChanged?.();
        } catch (caught: unknown) {
            setFeedback({ busy: false, error: safeError(caught) });
        }
    };
    return (
        <ActionSection title="Register project">
            <form onSubmit={(event) => { event.preventDefault(); setConfirm(true); }}>
                <TextField aria-describedby="project-root-hint" id="project-root" label="Project root" maxLength={1024} onInput={setPath} required supportingText="The daemon validates and owns this path." value={path} />
                <Button disabled={!available || feedback.busy || path.length === 0} title={capabilityUnavailableTitle(available, capabilities.projectsRegistry)} type="submit">Review registration</Button>
            </form>
            <ConfirmDialog busy={feedback.busy} open={confirm} title="Register this project?" detail="ALCOMD will inspect the selected root and add it to the per-user registry." onClose={() => setConfirm(false)} onConfirm={run} />
            <MutationFeedback client={client} feedback={feedback} />
        </ActionSection>
    );
}

export function ProjectActions({ client, onChanged, project }: ActionProps & { project: ProjectSnapshot }) {
    const available = useCapability(capabilities.projectsRegistry);
    const [confirmUnregister, setConfirmUnregister] = useState(false);
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const revision = project.revision;
    const refresh = async () => {
        if (project.projectId === undefined || revision === undefined) return;
        await runSimple(setFeedback, () => client.projectRefresh(project.projectId!, revision), "Project refreshed.", onChanged);
    };
    const unregister = async () => {
        if (project.projectId === undefined || revision === undefined) return;
        await runSimple(setFeedback, () => client.projectUnregister(project.projectId!, revision), "Project unregistered. Files were not deleted.", onChanged);
    };
    return (
        <ActionSection title="Project actions">
            <div className="action-row">
                <Button disabled={!available || feedback.busy || revision === undefined} onClick={() => void refresh()} title={capabilityUnavailableTitle(available, capabilities.projectsRegistry)} type="button" variant="tonal">Refresh</Button>
                <Button className="material-button--danger" disabled={!available || feedback.busy || revision === undefined} onClick={() => setConfirmUnregister(true)} title={capabilityUnavailableTitle(available, capabilities.projectsRegistry)} type="button" variant="text">Unregister</Button>
            </div>
            <ConfirmDialog busy={feedback.busy} open={confirmUnregister} title="Unregister this project?" detail="This removes only the ALCOMD registry entry. It does not delete the Unity project." onClose={() => setConfirmUnregister(false)} onConfirm={unregister} />
            <MutationFeedback client={client} feedback={feedback} />
        </ActionSection>
    );
}

export function RegisterRepositoryPanel({ client, onChanged }: ActionProps) {
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
            setFeedback({ busy: false, message: "Repository added." }); onChanged?.();
        } catch (caught: unknown) { setFeedback({ busy: false, error: safeError(caught) }); }
        finally { pending.current = false; }
    };
    return <ActionSection title="Add repository">
        {preview === undefined ? <form onSubmit={(event) => void inspect(event)}>
            <Select disabled={feedback.busy} id="repository-kind" label="Source type" onChange={(next) => setKind(next as "remote" | "local")} options={[{ label: "Remote URL", value: "remote" }, { label: "Local manifest", value: "local" }]} value={kind} />
            <TextField disabled={feedback.busy} id="repository-source" label={kind === "remote" ? "Repository URL" : "Local manifest path"} maxLength={2048} onInput={setValue} required type={kind === "remote" ? "url" : "text"} value={value} />
            <div className="dialog-actions"><UtilityDialogCancel disabled={feedback.busy} /><Button widthLabels={["Loading repository…", "Review repository"]} disabled={!available || feedback.busy || value.trim().length === 0} title={capabilityUnavailableTitle(available, capabilities.repositoriesRegistry)} type="submit">{feedback.busy ? "Loading repository…" : "Review repository"}</Button></div>
        </form> : <>
            <dl className="dialog-summary"><div><dt>Name</dt><dd>{preview.name ?? "Unnamed repository"}</dd></div><div><dt>Source</dt><dd>{preview.source.kind === "remote" ? preview.source.url : preview.source.path}</dd></div></dl>
            {preview.issues.length === 0 ? null : <p role="status">This repository has {preview.issues.length} reported issues. Registration will validate the source again.</p>}
            <div className="dialog-actions"><UtilityDialogCancel disabled={feedback.busy} /><Button disabled={feedback.busy} onClick={() => { setPreview(undefined); setFeedback(INITIAL_FEEDBACK); }} type="button" variant="text">Back</Button><Button widthLabels={["Adding…", "Add repository"]} disabled={!available || feedback.busy} onClick={() => void register()} type="button">{feedback.busy ? "Adding…" : "Add repository"}</Button></div>
        </>}
        <MutationFeedback client={client} feedback={feedback} />
    </ActionSection>;
}

export function RepositoryActions({ client, onChanged, repository }: ActionProps & { repository: RepositorySnapshot }) {
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
            setFeedback({ busy: false, message: remove ? "Repository removed." : "Repository refreshed." });
            setConfirmRemove(false); onChanged?.();
        } catch (caught: unknown) { setFeedback({ busy: false, error: safeError(caught) }); }
        finally { pending.current = false; }
    };
    return <div className="repository-row-actions">
        <Button disabled={!available || !ready || feedback.busy} onClick={() => void run(false)} type="button" variant="text">Refresh repository</Button>
        <Button disabled={!available || !ready || feedback.busy} onClick={() => setConfirmRemove(true)} type="button" variant="text">Remove</Button>
        <MaterialDialog open={confirmRemove} title="Remove this repository?" dismissible={!feedback.busy} onClose={() => setConfirmRemove(false)}>
            <p>Remove <strong>{repository.name ?? repository.declaredId ?? "this repository"}</strong> from your sources?</p><p>Packages already installed in projects stay installed.</p>
            <MutationFeedback client={client} feedback={feedback} />
            <div className="dialog-actions"><Button disabled={feedback.busy} onClick={() => setConfirmRemove(false)} type="button" variant="text">Cancel</Button><Button disabled={feedback.busy || !available} onClick={() => void run(true)} type="button">Remove repository</Button></div>
        </MaterialDialog>
        {confirmRemove ? null : <MutationFeedback client={client} feedback={feedback} />}
    </div>;
}

export interface PackageActionSelection {
    action: "install" | "remove" | "upgrade" | "downgrade" | "resolve" | "reinstall" | "reinstall-all" | "bulk-reinstall" | "bulk-remove" | "bulk-candidates";
    key: number;
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

export function PackageActions({ client, project, onChanged, selection, onEvidenceStale, onBusyChanged }: ActionProps & { project: ProjectSnapshot; selection?: PackageActionSelection; onEvidenceStale?(): void; onBusyChanged?(busy: boolean): void }) {
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
            setFeedback({ busy: false, error: { code: "capability_required", message: `The connected daemon did not negotiate ${requiresV2 ? capabilities.packagesPlanV2 : capabilities.packagesPlanV1}.` } });
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
    }, [canPlanV1, canPlanV2, client, projectId, revision, selection]);
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
            setFeedback({ busy: false, operationId: result.operationId, message: "Package changes started." });
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
            <MaterialDialog onClose={closeChanges} open={plan !== undefined} title={hasChanges ? "Apply package changes?" : "Packages are up to date"}>
                {selection?.exclusions?.length ? <div role="status"><p>Not included in this update:</p><ul>{selection.exclusions.map((reason) => <li key={reason}>{reason}</li>)}</ul></div> : null}
                {plan === undefined ? null : hasChanges ? (
                    <div className="package-plan-review">
                        <p>Review the changes ALCOMD will make to this project.</p>
                        <ul className="change-list">{plan.changeSet.mutations.map((mutation) => <li key={`${mutation.kind}-${mutation.packageId}`}><strong>{packageChangeLabel(mutation.kind)}</strong><span>{mutation.packageId}</span>{mutation.fromVersion == null && mutation.toVersion == null ? null : <small>{packageVersionChange(mutation.fromVersion, mutation.toVersion)}</small>}</li>)}</ul>
                        <div className="dialog-actions">
                            <Button disabled={feedback.busy} onClick={closeChanges} type="button" variant="text">Cancel</Button>
                            <Button widthLabels={["Applying…", "Apply changes"]} disabled={!canApply || feedback.busy} onClick={() => void apply()} title={capabilityUnavailableTitle(canApply, capabilities.packagesApply)} type="button">{feedback.busy ? "Applying…" : "Apply changes"}</Button>
                        </div>
                    </div>
                ) : (
                    <div className="package-plan-review"><p>No package changes are required for this project.</p><div className="dialog-actions"><Button onClick={closeChanges} type="button">Close</Button></div></div>
                )}
            </MaterialDialog>
            {feedback.busy && plan === undefined ? <div className="mutation-feedback" role="status" aria-live="polite">Checking package changes…</div> : null}
            {feedback.error === undefined ? null : <div className="mutation-feedback mutation-feedback--error" role="alert"><strong>Package changes were not applied</strong><span>{packageErrorMessage(feedback.error)}</span></div>}
            {feedback.operationId === undefined ? null : <OperationFollow client={client} operationId={feedback.operationId} onTerminal={() => { setOperationFinished(true); onChanged?.(); }} title="Package changes" />}
        </>
    );
}

export function UnityRegistryActions({ client, installations, onChanged }: ActionProps & { installations: UnityInstallation[] }) {
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
            setFeedback({ busy: false, message: action === "register" ? "Unity installation registered." : action === "refresh" ? "Unity installations refreshed." : "Unity installation removed from the list." });
            onChanged?.();
        } catch (caught: unknown) { setFeedback({ busy: false, error: safeError(caught) }); }
        finally { pending.current = false; }
    };
    return <ActionSection title={confirmRemove ? "Remove Unity installation" : "Manage Unity installations"}>
        {confirmRemove && remove !== undefined ? <>
            <p>Remove <strong>Unity {remove.unityVersion}</strong> from the list?</p>
            <p>Only the ALCOMD registry entry is removed. The editor remains installed.</p>
            <div className="dialog-actions"><UtilityDialogCancel disabled={feedback.busy} /><Button disabled={feedback.busy} onClick={() => { setConfirmRemove(false); setFeedback(INITIAL_FEEDBACK); }} type="button" variant="text">Back</Button><Button className="material-button--danger" disabled={!available || feedback.busy} onClick={() => void mutate("remove")} type="button">Remove installation</Button></div>
        </> : <>
            <form onSubmit={(event) => { event.preventDefault(); void mutate("register"); }}>
                <TextField disabled={feedback.busy} id="unity-executable" label="Unity executable" maxLength={1024} onInput={setPath} required value={path} />
                <div className="action-row"><Button disabled={!available || feedback.busy || path.trim().length === 0} title={capabilityUnavailableTitle(available, capabilities.unityManage)} type="submit">Register</Button><Button disabled={!available || feedback.busy} onClick={() => void mutate("refresh")} title={capabilityUnavailableTitle(available, capabilities.unityManage)} type="button" variant="tonal">Discover and refresh</Button></div>
            </form>
            {installations.length === 0 ? null : <Select aria-label="Installation to remove" disabled={feedback.busy} label="Remove installation" onChange={(next) => setRemove(installations.find((item) => item.installationId === next))} options={[{ label: "Select an installation", value: "" }, ...installations.map((item) => ({ label: "Unity " + item.unityVersion + " · " + item.architecture, value: item.installationId }))]} value={remove?.installationId ?? ""} />}
            <div className="dialog-actions"><UtilityDialogCancel disabled={feedback.busy} /><Button className="material-button--danger" disabled={!available || remove === undefined || feedback.busy} onClick={() => { setConfirmRemove(true); setFeedback(INITIAL_FEEDBACK); }} title={capabilityUnavailableTitle(available, capabilities.unityManage)} type="button" variant="text">Review removal</Button></div>
        </>}
        <MutationFeedback client={client} feedback={feedback} />
    </ActionSection>;
}

export function ProjectUnityActions({ afterMigrationOpen = false, client, installations, launchConfig, launchOptions, project, onChanged }: ActionProps & { afterMigrationOpen?: boolean; installations: UnityInstallation[]; launchConfig: ProjectUnityLaunchConfig; launchOptions: UnityLaunchOptionsResult; project: ProjectSnapshot }) {
    const canLaunch = useCapability(capabilities.unityLaunch);
    const canManage = useCapability(capabilities.unityManage);
    const canMigrate = useCapability(capabilities.projectsUnityMigration);
    const [argumentsText, setArgumentsText] = useState(launchConfig.arguments.join("\n"));
    const [argumentsOpen, setArgumentsOpen] = useState(false);
    const [argumentsRevision, setArgumentsRevision] = useState(launchConfig.revision);
    const [argumentsFeedback, setArgumentsFeedback] = useState(INITIAL_FEEDBACK);
    const argumentsPending = useRef(false);
    const [launchInstallationId, setLaunchInstallationId] = useState("");
    const [postMigrationLaunchOptions, setPostMigrationLaunchOptions] = useState<UnityLaunchOptionsResult>();
    const [targetVersion, setTargetVersion] = useState("");
    const [targetInstallationId, setTargetInstallationId] = useState("");
    const [migrationPlan, setMigrationPlan] = useState<ProjectUnityMigrationPlan>();
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const projectId = project.projectId;
    const projectRevision = project.revision;
    const handledMigrationOperation = useRef<string | undefined>(undefined);
    useEffect(() => {
        if (!argumentsOpen) setArgumentsText(launchConfig.arguments.join("\n"));
    }, [argumentsOpen, launchConfig]);
    const migrationVersions = [...new Set(installations.map((installation) => installation.unityVersion))]
        .filter((version) => version !== project.unityVersion)
        .sort((left, right) => left.localeCompare(right));
    const targetInstallations = installations.filter((installation) => installation.unityVersion === targetVersion);
    const closeArguments = () => {
        if (argumentsPending.current) return;
        setArgumentsOpen(false); setArgumentsText(launchConfig.arguments.join("\n")); setArgumentsFeedback(INITIAL_FEEDBACK);
    };
    const saveArguments = async (clear: boolean) => {
        if (projectId === undefined || !canManage || argumentsPending.current) return;
        argumentsPending.current = true; setArgumentsFeedback({ busy: true });
        try {
            if (clear) await client.unityProjectLaunchConfigClear(projectId, argumentsRevision);
            else await client.unityProjectLaunchConfigSet(projectId, argumentsText.split(/\r?\n/).map((value) => value.trim()).filter(Boolean), argumentsRevision);
            setArgumentsOpen(false); setArgumentsFeedback(INITIAL_FEEDBACK);
            setFeedback({ busy: false, message: clear ? "Unity launch arguments cleared." : "Unity launch arguments updated." });
            onChanged?.();
        } catch (caught: unknown) { setArgumentsFeedback({ busy: false, error: safeError(caught) }); }
        finally { argumentsPending.current = false; }
    };
    const launch = async () => {
        if (projectId === undefined) return;
        const effectiveOptions = postMigrationLaunchOptions ?? launchOptions;
        const candidates = effectiveOptions.exactMatchingInstallations;
        const installationId = candidates.length === 1 ? candidates[0]?.installationId : launchInstallationId;
        if (installationId === undefined || installationId.length === 0) {
            setFeedback({ busy: false, message: candidates.length === 0
                ? `This project requires Unity ${launchOptions.projectUnityVersion}. No exact matching Unity installation was found.`
                : "Choose the Unity installation to use for this launch." });
            return;
        }
        setFeedback({ busy: true });
        try {
            const result = await client.unityLaunch(projectId, installationId, effectiveOptions.projectRevision);
            setFeedback({ busy: false, message: `Unity launch ${result.launch.state}.` });
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
                setFeedback({ busy: false, message: `Project already uses Unity ${result.currentVersion}.` });
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
            setFeedback({ busy: false, message: "Unity migration accepted.", operationId: result.operationId });
        } catch (caught: unknown) {
            setFeedback({ busy: false, error: safeError(caught) });
        }
    };
    const migrationFinished = useCallback(async (operation: Operation) => {
        if (!afterMigrationOpen || handledMigrationOperation.current === operation.operationId) return;
        handledMigrationOperation.current = operation.operationId;
        if (operation.state !== "succeeded") {
            setFeedback({ busy: false, error: { code: operation.errorCode ?? "internal_error", message: "The Unity migration did not complete." } });
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
                setFeedback({ busy: false, message: `Migration completed, but no exact Unity ${options.projectUnityVersion} installation is available.` });
                return;
            }
            if (options.exactMatchingInstallations.length > 1) {
                setLaunchInstallationId("");
                setFeedback({ busy: false, message: "Migration completed. Choose the Unity installation for this launch." });
                return;
            }
            const result = await client.unityLaunch(projectId, options.exactMatchingInstallations[0]!.installationId, freshProject.revision);
            setFeedback({ busy: false, message: `Migration completed. Unity launch ${result.launch.state}.` });
        } catch (caught: unknown) {
            setFeedback({ busy: false, error: safeError(caught) });
        }
    }, [afterMigrationOpen, client, onChanged, projectId]);
    const effectiveLaunchOptions = postMigrationLaunchOptions ?? launchOptions;
    return (
        <ActionSection title="Unity actions">
            <p>Project Unity version: <strong>{project.unityVersion}</strong></p>
            {effectiveLaunchOptions.exactMatchingInstallations.length > 1 ? <Select id="launch-installation" label="Unity installation for this launch" onChange={setLaunchInstallationId} options={[{ label: "Choose an installation", value: "" }, ...effectiveLaunchOptions.exactMatchingInstallations.map((item) => ({ label: `Unity ${item.unityVersion} · ${item.architecture}`, value: item.installationId }))]} value={launchInstallationId} /> : null}
            <Button disabled={!canLaunch || feedback.busy || projectRevision === undefined} onClick={() => void launch()} title={capabilityUnavailableTitle(canLaunch, capabilities.unityLaunch)} type="button">Open Unity</Button>
            {effectiveLaunchOptions.exactMatchingInstallations.length === 0 ? <p>This project requires Unity {effectiveLaunchOptions.projectUnityVersion}. No exact matching Unity installation was found.</p> : null}
            <Button disabled={!canManage || feedback.busy || projectId === undefined} onClick={() => { setArgumentsText(launchConfig.arguments.join("\n")); setArgumentsFeedback(INITIAL_FEEDBACK); setArgumentsRevision(launchConfig.revision); setArgumentsOpen(true); }} title={capabilityUnavailableTitle(canManage, capabilities.unityManage)} type="button" variant="tonal">Edit launch arguments</Button>
            <MaterialDialog dismissible={!argumentsFeedback.busy} open={argumentsOpen} onClose={closeArguments} title="Unity launch arguments">
                <form onSubmit={(event) => { event.preventDefault(); void saveArguments(false); }}>
                    <TextField aria-describedby="unity-arguments-hint" disabled={argumentsFeedback.busy} id="unity-arguments" label="Additional arguments" maxLength={4096} onInput={setArgumentsText} rows={4} supportingText="One argument per line. The daemon validates forbidden arguments." type="textarea" value={argumentsText} />
                    <div className="dialog-actions">
                        <Button disabled={argumentsFeedback.busy} onClick={closeArguments} type="button" variant="text">Cancel</Button>
                        <Button disabled={!canManage || argumentsFeedback.busy || projectId === undefined || launchConfig.revision === 0} onClick={() => void saveArguments(true)} type="button" variant="text">Clear launch arguments</Button>
                        <Button disabled={!canManage || argumentsFeedback.busy || projectId === undefined} type="submit">Save launch arguments</Button>
                    </div>
                </form>
                <MutationFeedback client={client} feedback={argumentsFeedback} />
            </MaterialDialog>
            <h3>Migrate Project Unity version</h3>
            <Select id="project-unity-version" label="Target Unity version" onChange={chooseTargetVersion} options={[{ label: `Current · ${project.unityVersion}`, value: "" }, ...migrationVersions.map((version) => ({ label: version, value: version }))]} value={targetVersion} />
            {targetInstallations.length > 1 ? <Select id="migration-installation" label="Target Unity installation" onChange={setTargetInstallationId} options={[{ label: "Choose an installation", value: "" }, ...targetInstallations.map((item) => ({ label: `${item.unityVersion} · ${item.architecture}`, value: item.installationId }))]} value={targetInstallationId} /> : null}
            <Button disabled={!canMigrate || feedback.busy || targetInstallationId.length === 0 || projectRevision === undefined} onClick={() => void planMigration()} title={capabilityUnavailableTitle(canMigrate, capabilities.projectsUnityMigration)} type="button" variant="tonal">Review migration</Button>
            <PlanDialog applyDisabled={migrationPlan?.classification.supportedForApply === false} busy={feedback.busy} onApply={applyMigration} onClose={() => setMigrationPlan(undefined)} open={migrationPlan !== undefined} title="Review Unity migration">
                {migrationPlan === undefined ? null : <dl className="dialog-summary"><div><dt>From</dt><dd>{migrationPlan.sourceUnityVersion}</dd></div><div><dt>To</dt><dd>{migrationPlan.targetUnityVersion}</dd></div><div><dt>Classification</dt><dd>{humanize(migrationPlan.classification.kind)}</dd></div><div><dt>Supported</dt><dd>{migrationPlan.classification.supportedForApply ? "Yes" : "No"}</dd></div></dl>}
            </PlanDialog>
            <MutationFeedback client={client} feedback={feedback} onOperationTerminal={migrationFinished} />
        </ActionSection>
    );
}

export function ProjectUnityWorkspaceActions({ client, installations, launchOptions, navigate, project }: {
    client: GuiRpcClient;
    installations: UnityInstallation[];
    launchOptions: UnityLaunchOptionsResult;
    navigate(path: string): void;
    project: ProjectSnapshot;
}) {
    const canLaunch = useCapability(capabilities.unityLaunch);
    const canMigrate = useCapability(capabilities.projectsUnityMigration);
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const [launchChooserOpen, setLaunchChooserOpen] = useState(false);
    const [launchInstallationId, setLaunchInstallationId] = useState("");
    const [missingExactOpen, setMissingExactOpen] = useState(false);
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

    const launch = async (installationId: string) => {
        if (projectId === undefined || projectRevision === undefined) return;
        setFeedback({ busy: true });
        try {
            const result = await client.unityLaunch(projectId, installationId, projectRevision);
            setLaunchChooserOpen(false);
            setLaunchInstallationId("");
            setFeedback({ busy: false, message: `Unity launch ${result.launch.state}.` });
        } catch (caught: unknown) {
            setFeedback({ busy: false, error: safeError(caught) });
        }
    };
    const openUnity = async () => {
        const candidates = launchOptions.exactMatchingInstallations;
        if (candidates.length === 0) {
            setMissingExactOpen(true);
            return;
        }
        if (candidates.length === 1) {
            await launch(candidates[0]!.installationId);
            return;
        }
        setLaunchInstallationId("");
        setLaunchChooserOpen(true);
    };
    const planMigration = async (installationId: string) => {
        if (projectId === undefined || projectRevision === undefined) return;
        setFeedback({ busy: true });
        try {
            const result = await client.projectPlanUnityMigration(projectId, installationId, projectRevision);
            setMigrationChooserOpen(false);
            if (result.kind === "no_change") {
                setMigrationVersion(project.unityVersion);
                setFeedback({ busy: false, message: `Project already uses Unity ${result.currentVersion}.` });
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
                label="Unity version"
                onChange={chooseMigrationVersion}
                options={[
                    { label: `Unity ${project.unityVersion}`, value: project.unityVersion },
                    ...migrationVersions.map((version) => ({ label: `Unity ${version}`, value: version }))
                ]}
                value={migrationVersion}
            />
            <Button disabled={!canLaunch || feedback.busy || projectRevision === undefined} onClick={() => void openUnity()} title={capabilityUnavailableTitle(canLaunch, capabilities.unityLaunch)} type="button"><Icon asset={playArrowIcon} slot="icon" />Open Unity</Button>
            <ModalDialog onClose={() => setMissingExactOpen(false)} open={missingExactOpen} title="Unity installation required">
                <p>This project requires Unity {launchOptions.projectUnityVersion}. No exact matching Unity installation was found.</p>
                <div className="dialog-actions">
                    <Button onClick={() => setMissingExactOpen(false)} type="button" variant="tonal">Cancel</Button>
                    <Button onClick={() => { setMissingExactOpen(false); navigate(`/projects/${projectId}/unity?afterMigration=open`); }} type="button">Migrate Project…</Button>
                </div>
            </ModalDialog>
            <ModalDialog onClose={() => setLaunchChooserOpen(false)} open={launchChooserOpen} title="Choose Unity for this launch">
                <Select label="Unity installation for this launch" onChange={setLaunchInstallationId} options={[{ label: "Choose an installation", value: "" }, ...launchOptions.exactMatchingInstallations.map((item) => ({ label: `Unity ${item.unityVersion} · ${item.architecture}`, value: item.installationId }))]} value={launchInstallationId} />
                <div className="dialog-actions">
                    <Button onClick={() => setLaunchChooserOpen(false)} type="button" variant="tonal">Cancel</Button>
                    <Button disabled={launchInstallationId.length === 0 || feedback.busy} onClick={() => void launch(launchInstallationId)} type="button">Open Unity</Button>
                </div>
            </ModalDialog>
            <ModalDialog onClose={() => { setMigrationChooserOpen(false); setMigrationVersion(project.unityVersion); }} open={migrationChooserOpen} title={`Choose Unity ${migrationVersion} installation`}>
                <Select label="Target Unity installation" onChange={setMigrationInstallationId} options={[{ label: "Choose an installation", value: "" }, ...migrationInstallations.map((item) => ({ label: `${item.unityVersion} · ${item.architecture}`, value: item.installationId }))]} value={migrationInstallationId} />
                <div className="dialog-actions">
                    <Button onClick={() => { setMigrationChooserOpen(false); setMigrationVersion(project.unityVersion); }} type="button" variant="tonal">Cancel</Button>
                    <Button disabled={migrationInstallationId.length === 0 || feedback.busy} onClick={() => void planMigration(migrationInstallationId)} type="button">Review migration</Button>
                </div>
            </ModalDialog>
            <PlanDialog applyDisabled={migrationPlan?.classification.supportedForApply === false} busy={feedback.busy} onApply={applyMigration} onClose={() => { setMigrationPlan(undefined); setMigrationVersion(project.unityVersion); }} open={migrationPlan !== undefined} title="Review Unity migration">
                {migrationPlan === undefined ? null : <dl className="dialog-summary"><div><dt>From</dt><dd>{migrationPlan.sourceUnityVersion}</dd></div><div><dt>To</dt><dd>{migrationPlan.targetUnityVersion}</dd></div><div><dt>Classification</dt><dd>{humanize(migrationPlan.classification.kind)}</dd></div><div><dt>Supported</dt><dd>{migrationPlan.classification.supportedForApply ? "Yes" : "No"}</dd></div></dl>}
            </PlanDialog>
            {feedback.error === undefined ? null : <span className="inline-error" role="alert">Unity action failed: {feedback.error.code}</span>}
            {feedback.message === undefined ? null : <span aria-live="polite" role="status">{feedback.message}</span>}
        </>
    );
}

export function TemplateImportPanel({ client, onChanged }: ActionProps) {
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
            setFeedback({ busy: false, operationId: result.operationId, message: "Template import accepted." }); onChanged?.();
        } catch (caught: unknown) { setFeedback({ busy: false, error: safeError(caught) }); }
        finally { pending.current = false; }
    };
    return <ActionSection title={plan === undefined ? "Import template" : "Review template import"}>
        {plan === undefined ? <form onSubmit={(event) => void create(event)}>
            <TextField disabled={feedback.busy} id="template-bundle" label="Template bundle" maxLength={1024} onInput={setBundlePath} required value={bundlePath} />
            <Checkbox checked={overrideExisting} disabled={feedback.busy} label="Replace an existing matching template" onChange={setOverrideExisting} />
            <div className="dialog-actions"><UtilityDialogCancel disabled={feedback.busy} /><Button widthLabels={["Preparing review…", "Review import"]} disabled={!available || feedback.busy || bundlePath.trim().length === 0} title={capabilityUnavailableTitle(available, capabilities.templatesManage)} type="submit">{feedback.busy ? "Preparing review…" : "Review import"}</Button></div>
        </form> : <>
            <p>Action: <strong>{humanize(plan.action)}</strong></p>
            <p>Plan fingerprint: <code>{shortValue(plan.planFingerprint)}</code></p>
            <p className="risk-summary">The daemon will revalidate this frozen plan before importing.</p>
            <div className="dialog-actions"><UtilityDialogCancel disabled={feedback.busy} /><Button disabled={feedback.busy} onClick={() => { setPlan(undefined); setFeedback(INITIAL_FEEDBACK); }} type="button" variant="text">Discard plan</Button><Button widthLabels={["Applying…", "Apply reviewed plan"]} disabled={!available || feedback.busy} onClick={() => void apply()} type="button">{feedback.busy ? "Applying…" : "Apply reviewed plan"}</Button></div>
        </>}
        <MutationFeedback client={client} feedback={feedback} />
    </ActionSection>;
}

export function TemplateActions({ client, compact = false, onChanged, onView, template }: ActionProps & { compact?: boolean; onView?(): void; template: TemplateRecord }) {
    const canCreateProject = useCapability(capabilities.templatesCreateProject);
    const canManage = useCapability(capabilities.templatesManage);
    const [mode, setMode] = useState<"none" | "derive" | "create" | "export" | "remove">("none");
    const [plan, setPlan] = useState<TemplatePlan>();
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const [projects, setProjects] = useState<ProjectSnapshot[]>([]);
    const [projectPage, setProjectPage] = useState<Awaited<ReturnType<GuiRpcClient["projectsList"]>>>();
    const [projectsLoading, setProjectsLoading] = useState(false);
    const [fields, setFields] = useState({ projectId: "", templateId: "", templateVersion: "1.0.0", displayName: "", description: "", parent: "", leaf: "", exportPath: "" });
    const [menuOpen, setMenuOpen] = useState(false);
    const menuAnchorRef = useRef<HTMLElement>(null);
    const pending = useRef(false);
    const projectLoad = useRef(0);
    const update = (key: keyof typeof fields, value: string) => setFields((current) => ({ ...current, [key]: value }));
    const close = () => {
        if (pending.current) return;
        projectLoad.current += 1;
        setMode("none"); setPlan(undefined); setProjectsLoading(false);
    };
    const loadProjects = async (append: boolean) => {
        const generation = ++projectLoad.current;
        setProjectsLoading(true);
        try {
            const page = await client.projectsList(append ? projectPage?.nextCursor : undefined);
            if (generation !== projectLoad.current) return;
            setProjectPage(page);
            setProjects((current) => append ? [...current, ...page.projects] : page.projects);
        } catch (caught: unknown) {
            if (generation === projectLoad.current) setFeedback({ busy: false, error: safeError(caught) });
        } finally { if (generation === projectLoad.current) setProjectsLoading(false); }
    };
    const open = (next: typeof mode) => {
        if (pending.current) return;
        setMenuOpen(false); setPlan(undefined); setFeedback(INITIAL_FEEDBACK); setMode(next);
        setFields({ projectId: "", templateId: "", templateVersion: "1.0.0", displayName: "", description: "", parent: "", leaf: "", exportPath: "" });
        if (next === "derive") { setProjects([]); setProjectPage(undefined); void loadProjects(false); }
    };
    const createPlan = async (event: FormEvent) => {
        event.preventDefault();
        if (pending.current || (mode === "derive" ? !canManage : !canCreateProject)) return;
        pending.current = true; setFeedback({ busy: true });
        try {
            let next: TemplatePlan;
            if (mode === "derive") {
                const source = (await client.projectGet(fields.projectId)).project;
                if (source.projectId === undefined || source.revision === undefined) throw { code: "project_revision_unavailable" };
                next = await client.templatePlanDerive({ projectId: source.projectId, expectedProjectRevision: source.revision, templateId: fields.templateId, templateVersion: fields.templateVersion, displayName: fields.displayName, ...(fields.description.length === 0 ? {} : { description: fields.description }) });
            } else {
                next = await client.templatePlanCreateProject(template.templateId, template.revision, fields.parent, fields.leaf);
            }
            setPlan(next); setFeedback({ busy: false });
        } catch (caught: unknown) { setFeedback({ busy: false, error: safeError(caught) }); }
        finally { pending.current = false; }
    };
    const apply = async () => {
        if (plan === undefined || pending.current || (mode === "derive" ? !canManage : !canCreateProject)) return;
        pending.current = true; setFeedback({ busy: true });
        try {
            const result = mode === "derive" ? await client.templateApplyDerive(plan.planId) : await client.templateApplyCreateProject(plan.planId);
            setPlan(undefined); setMode("none");
            setFeedback({ busy: false, operationId: result.operationId, message: "Template operation accepted." }); onChanged?.();
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
            setFeedback({ busy: false, message: action === "favorite" ? "Favorite updated." : action === "export" ? "Template exported." : "Template removed." });
            if (action !== "export") onChanged?.();
        } catch (caught: unknown) { setFeedback({ busy: false, error: safeError(caught) }); }
        finally { pending.current = false; }
    };
    const pickParent = async () => {
        try { const path = await client.selectDirectory(); if (path !== undefined) update("parent", path); }
        catch (caught: unknown) { setFeedback({ busy: false, error: safeError(caught) }); }
    };
    const title = plan !== undefined ? mode === "derive" ? "Review new template" : "Review project creation" : mode === "derive" ? "Create template from project" : mode === "create" ? "Create project" : mode === "export" ? "Export template" : "Remove template";
    return <section className={compact ? "template-row-actions" : "action-section"}>
        {compact ? <div className="card-actions">
            <IconButton aria-pressed={template.favorite} className="project-favorite-action" disabled={!canManage || feedback.busy} label={template.favorite ? "Remove " + template.displayName + " from favorites" : "Favorite " + template.displayName} onClick={() => void simple("favorite")} type="button"><Icon asset={starIcon} /></IconButton>
            <IconButton aria-expanded={menuOpen} disabled={feedback.busy} label={"More actions for " + template.displayName} onClick={() => setMenuOpen(true)} ref={menuAnchorRef} type="button"><Icon asset={moreVertIcon} size={24} /></IconButton>
            <Menu anchorRef={menuAnchorRef} onClose={() => setMenuOpen(false)} open={menuOpen}>
                <MenuItem disabled={!canCreateProject} label="Create project" onClick={() => open("create")} />
                {onView === undefined ? null : <MenuItem label="View template" onClick={() => { setMenuOpen(false); onView(); }} />}
                <MenuItem disabled={!canManage} label="Derive from project" onClick={() => open("derive")} />
                <MenuItem disabled={!canManage} label="Export template" onClick={() => open("export")} />
                <MenuItem disabled={!canManage || template.sourceKind === "builtin"} label="Remove template" onClick={() => open("remove")} title={template.sourceKind === "builtin" ? "Built-in templates cannot be removed." : undefined} />
            </Menu>
        </div> : <><h2>Template actions</h2><div className="action-row">
            <Button disabled={!canCreateProject || feedback.busy} onClick={() => open("create")} title={capabilityUnavailableTitle(canCreateProject, capabilities.templatesCreateProject)} type="button">Create project</Button>
            <Button disabled={!canManage || feedback.busy} onClick={() => open("derive")} title={capabilityUnavailableTitle(canManage, capabilities.templatesManage)} type="button" variant="tonal">Derive from project</Button>
            <Button widthLabels={["Remove favorite", "Favorite"]} disabled={!canManage || feedback.busy} onClick={() => void simple("favorite")} type="button" variant="text">{template.favorite ? "Remove favorite" : "Favorite"}</Button>
            <Button disabled={!canManage || feedback.busy} onClick={() => open("export")} type="button" variant="text">Export</Button>
            {template.sourceKind === "builtin" ? null : <Button className="material-button--danger" disabled={!canManage || feedback.busy} onClick={() => open("remove")} type="button" variant="text">Remove template</Button>}
        </div></>}
        <MaterialDialog dismissible={!feedback.busy} open={mode !== "none"} onClose={close} title={title}>
            {plan !== undefined ? <>
                <p>Action: <strong>{humanize(plan.action)}</strong></p>
                <p>Plan fingerprint: <code>{shortValue(plan.planFingerprint)}</code></p>
                <p className="risk-summary">The daemon will revalidate this frozen plan before applying it.</p>
                <div className="dialog-actions"><Button disabled={feedback.busy} onClick={close} type="button" variant="text">Discard plan</Button><Button disabled={feedback.busy || (mode === "derive" ? !canManage : !canCreateProject)} onClick={() => void apply()} type="button">Apply reviewed plan</Button></div>
            </> : mode === "derive" || mode === "create" ? <form onSubmit={(event) => void createPlan(event)}>
                {mode === "derive" ? <>
                    <Select id="derive-project" label="Source project" disabled={feedback.busy || projectsLoading} onChange={(next) => update("projectId", next)} options={[{ label: "Choose a project", value: "" }, ...projects.filter((project) => project.projectId !== undefined).map((project) => ({ label: project.rootPath, value: project.projectId! }))]} value={fields.projectId} />
                    {projectsLoading ? <p role="status">Loading projects…</p> : projects.length === 0 ? <p>No registered projects available.</p> : null}
                    {projectPage?.nextCursor === undefined ? null : <Button disabled={projectsLoading || feedback.busy} onClick={() => void loadProjects(true)} type="button" variant="text">Load more projects</Button>}
                    <TextField disabled={feedback.busy} id="derive-name" label="Display name" onInput={(next) => update("displayName", next)} required value={fields.displayName} />
                    <TextField disabled={feedback.busy} id="derive-template-id" label="New template ID" onInput={(next) => update("templateId", next)} required value={fields.templateId} />
                    <TextField disabled={feedback.busy} id="derive-version" label="Template version" onInput={(next) => update("templateVersion", next)} required value={fields.templateVersion} />
                    <TextField disabled={feedback.busy} id="derive-description" label="Description" onInput={(next) => update("description", next)} value={fields.description} />
                </> : <>
                    <p>Template: <strong>{template.displayName}</strong></p>
                    <TextField disabled={feedback.busy} id="create-parent" label="Target parent" onInput={(next) => update("parent", next)} required value={fields.parent} />
                    <Button disabled={feedback.busy} onClick={() => void pickParent()} type="button" variant="text">Browse…</Button>
                    <TextField disabled={feedback.busy} id="create-leaf" label="Project folder name" onInput={(next) => update("leaf", next)} required value={fields.leaf} />
                </>}
                <div className="dialog-actions"><Button disabled={feedback.busy} onClick={close} type="button" variant="text">Cancel</Button><Button widthLabels={["Preparing review…", "Review"]} disabled={feedback.busy || (mode === "derive" ? !canManage || fields.projectId.length === 0 : !canCreateProject)} type="submit">{feedback.busy ? "Preparing review…" : "Review"}</Button></div>
            </form> : mode === "export" ? <form onSubmit={(event) => { event.preventDefault(); void simple("export"); }}>
                <p>Export <strong>{template.displayName}</strong> as a template bundle.</p>
                <TextField disabled={feedback.busy} id="template-export" label="Export target" onInput={(next) => update("exportPath", next)} required value={fields.exportPath} />
                <div className="dialog-actions"><Button disabled={feedback.busy} onClick={close} type="button" variant="text">Cancel</Button><Button disabled={!canManage || feedback.busy} type="submit">Export</Button></div>
            </form> : mode === "remove" ? <>
                <p>Remove <strong>{template.displayName}</strong> from your templates?</p>
                <div className="dialog-actions"><Button disabled={feedback.busy} onClick={close} type="button" variant="text">Cancel</Button><Button className="material-button--danger" disabled={!canManage || feedback.busy} onClick={() => void simple("remove")} type="button">Remove template</Button></div>
            </> : null}
            <MutationFeedback client={client} feedback={feedback} />
        </MaterialDialog>
        {mode === "none" ? <MutationFeedback client={client} feedback={feedback} /> : null}
    </section>;
}

export function BackupCreatePanel({ client, onChanged, project }: ActionProps & { project: ProjectSnapshot }) {
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
    return <ActionSection title="Create backup">
        <p>Back up <strong>{project.rootPath.split(/[\\/]/).filter(Boolean).at(-1) ?? project.projectId}</strong>.</p>
        {feedback.operationId === undefined ? <><Select disabled={feedback.busy} id="backup-compression" label="Compression" onChange={(next) => setCompression(next as typeof compression)} options={[{ label: "Store", value: "store" }, { label: "Fast", value: "fast" }, { label: "Maximum", value: "maximum" }]} value={compression} /><Checkbox disabled={feedback.busy} checked={exclude} label="Exclude VPM packages" onChange={setExclude} /><div className="dialog-actions"><UtilityDialogCancel disabled={feedback.busy} /><Button disabled={!available || feedback.busy || project.revision === undefined} onClick={() => void create()} type="button">Create backup</Button></div></> : <p>Closing this dialog does not cancel the backup task.</p>}
        <MutationFeedback client={client} feedback={feedback} />
    </ActionSection>;
}

export function BackupRestorePanel({ backup, client }: ActionProps & { backup: BackupRecord }) {
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
    return <ActionSection title="Restore backup">
        {feedback.operationId !== undefined ? <p>Closing this dialog does not cancel the restore task.</p> : plan === undefined ? <form onSubmit={(event) => { event.preventDefault(); void run(false); }}>
            <TextField disabled={feedback.busy} id="restore-parent" label="Target parent" maxLength={1024} onInput={setParent} required value={parent} />
            <Button disabled={feedback.busy} onClick={() => void client.selectDirectory().then((path) => { if (path !== undefined) setParent(path); }).catch((caught: unknown) => setFeedback({ busy: false, error: safeError(caught) }))} type="button" variant="text">Browse…</Button>
            <TextField disabled={feedback.busy} id="restore-leaf" label="New directory name" maxLength={255} onInput={setLeaf} required value={leaf} />
            <div className="dialog-actions"><UtilityDialogCancel disabled={feedback.busy} /><Button disabled={!available || feedback.busy} type="submit">Review backup restore</Button></div>
        </form> : <>
            <h3>Review backup restore</h3><dl className="dialog-summary"><div><dt>Target parent</dt><dd>{plan.target.parent}</dd></div><div><dt>New directory</dt><dd>{plan.target.leaf}</dd></div><div><dt>Target must be absent</dt><dd>{plan.target.mustBeAbsent ? "Yes" : "No"}</dd></div></dl>
            <p>{plan.packagesRequireResolve ? "VPM packages require a separate resolve after restoration." : "No package resolve is required."}</p>
            <div className="dialog-actions"><UtilityDialogCancel disabled={feedback.busy} /><Button disabled={feedback.busy} onClick={() => setPlan(undefined)} type="button" variant="text">Back</Button><Button disabled={!available || feedback.busy} onClick={() => void run(true)} type="button">Apply reviewed plan</Button></div>
        </>}
        <MutationFeedback client={client} feedback={feedback} />
    </ActionSection>;
}

export function OperationActions({ client, operation, onChanged }: ActionProps & { operation: Operation }) {
    const available = useCapability(capabilities.operations);
    const [confirm, setConfirm] = useState(false);
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const terminal = isTerminal(operation.state);
    const cancel = async () => { await runSimple(setFeedback, () => client.operationCancel(operation.operationId, operation.revision), "Cancellation requested.", onChanged); };
    return <ActionSection title="Operation control"><Button className="material-button--danger" disabled={!available || terminal || feedback.busy} onClick={() => setConfirm(true)} title={capabilityUnavailableTitle(available, capabilities.operations)} type="button" variant="text">Cancel operation</Button><ConfirmDialog busy={feedback.busy || !available} open={confirm} title="Request cancellation?" detail="Cancellation is cooperative. The operation remains authoritative until the daemon reports a terminal state." onClose={() => setConfirm(false)} onConfirm={cancel} /><MutationFeedback client={client} feedback={feedback} /></ActionSection>;
}

export function ExtensionInstallPanel({ client, onChanged }: ActionProps) {
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
                setFeedback({ busy: false, operationId: result.operationId, message: "Extension install accepted." });
                onChanged?.();
            } else {
                const result = await client.extensionPlanInstall(path, Number(expectedRevision), approvePublisher ? "approve_for_extension" : "none");
                setPlan(result.plan);
                setFeedback({ busy: false });
            }
        } catch (caught: unknown) { setFeedback({ busy: false, error: safeError(caught) }); }
        finally { pending.current = false; }
    };
    return <section aria-label={plan === undefined ? "Extension install input" : "Review extension install"} aria-busy={feedback.busy}>
        {plan === undefined ? <form onSubmit={(event) => { event.preventDefault(); void run(false); }}>
            <TextField disabled={feedback.busy} id="extension-package" label="Extension package" maxLength={1024} onInput={setPath} required value={path} />
            <p className="risk-summary">Transitional input: the current registry read interface does not provide its revision. Enter the verified registry revision; it is never inferred from installed extensions.</p>
            <TextField disabled={feedback.busy} id="extension-registry-revision" label="Expected registry revision" min={0} onInput={setExpectedRevision} required type="number" value={expectedRevision} />
            <Checkbox disabled={feedback.busy} checked={approvePublisher} label="Approve this publisher for this extension" onChange={setApprovePublisher} />
            <div className="dialog-actions"><UtilityDialogCancel disabled={feedback.busy} /><Button disabled={!available || feedback.busy || !revisionValid} title={capabilityUnavailableTitle(available, capabilities.extensionsLifecycle)} type="submit">Create install plan</Button></div>
        </form> : <>
            <h2>Review extension install</h2>
            <dl className="dialog-summary"><div><dt>Extension</dt><dd>{plan.extensionId}</dd></div><div><dt>Version</dt><dd>{plan.version}</dd></div><div><dt>Publisher</dt><dd><code>{plan.publisherFingerprint}</code></dd></div><div><dt>Trust</dt><dd>{humanize(plan.trustDecision)}</dd></div></dl>
            <p>The daemon will revalidate this install plan before applying it.</p>
            <div className="dialog-actions"><UtilityDialogCancel disabled={feedback.busy} /><Button disabled={feedback.busy} onClick={() => { setPlan(undefined); setFeedback(INITIAL_FEEDBACK); }} type="button" variant="text">Discard plan</Button><Button disabled={!available || feedback.busy} onClick={() => void run(true)} type="button">Apply reviewed plan</Button></div>
        </>}
        <MutationFeedback client={client} feedback={feedback} />
    </section>;
}

export function ExtensionActions({ client, extension, onChanged }: ActionProps & { extension: ExtensionRecord }) {
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
                setFeedback({ busy: false, operationId: result.operationId, message: "Extension uninstall accepted." });
                onChanged?.();
            } else if (kind === "grant" || kind === "revoke") {
                await (kind === "grant" ? client.extensionSetGrant(extension.extensionId, grant.permission, grant.resourceKind, grant.resourceId, extension.grantRevision) : client.extensionRevokeGrant(extension.extensionId, grant.permission, grant.resourceKind, grant.resourceId, extension.grantRevision));
                setFeedback({ busy: false, message: kind === "grant" ? "Permission granted." : "Permission revoked." });
                onChanged?.();
            } else if (kind === "enable" || kind === "disable") {
                await (kind === "enable" ? client.extensionEnable(extension.extensionId, extension.revision) : client.extensionDisable(extension.extensionId, extension.revision));
                setFeedback({ busy: false, message: kind === "enable" ? "Extension enabled." : "Extension disabled." });
                onChanged?.();
            }
        } catch (caught: unknown) { setFeedback({ busy: false, error: safeError(caught) }); }
        finally { pending.current = false; }
    };
    const blocked = extension.desiredState === "uninstalling" || extension.quarantineState === "quarantined";
    return <ActionSection title="Extension management">
        <div className="action-row"><Button disabled={!canManageLifecycle || feedback.busy || blocked || extension.desiredState === "enabled"} onClick={() => void run("enable")} type="button" variant="tonal">Enable</Button><Button disabled={!canManageLifecycle || feedback.busy || blocked || extension.desiredState !== "enabled"} onClick={() => void run("disable")} type="button" variant="tonal">Disable</Button><Button disabled={feedback.busy} onClick={() => open("permissions")} type="button" variant="text">Manage permissions</Button><Button className="material-button--danger" disabled={!canManageLifecycle || feedback.busy || extension.desiredState === "uninstalling"} onClick={() => open("uninstall")} type="button" variant="text">Uninstall extension</Button></div>
        <MaterialDialog dismissible={!feedback.busy} open={mode !== "none"} onClose={close} title={mode === "permissions" ? "Extension permissions" : plan === undefined ? "Uninstall extension" : "Review extension uninstall"}>
            <p><strong>{extension.extensionId}</strong></p>
            {mode === "permissions" ? <>
                <p>Specify the exact permission and resource scope. Changes take effect only after Grant or Revoke.</p>
                <TextField disabled={feedback.busy} id="grant-permission" label="Permission" onInput={(permission) => setGrant((current) => ({ ...current, permission }))} required value={grant.permission} />
                <Select disabled={feedback.busy} id="grant-kind" label="Resource kind" onChange={(resourceKind) => setGrant((current) => ({ ...current, resourceKind }))} options={[{ label: "Project", value: "Project" }, { label: "Extension", value: "Extension" }]} value={grant.resourceKind} />
                <TextField disabled={feedback.busy} id="grant-resource" label="Resource ID" onInput={(resourceId) => setGrant((current) => ({ ...current, resourceId }))} required value={grant.resourceId} />
                <div className="dialog-actions"><Button disabled={feedback.busy} onClick={close} type="button" variant="text">Close</Button><Button disabled={!canManagePermissions || feedback.busy || !grant.permission.trim() || !grant.resourceId.trim()} onClick={() => void run("revoke")} type="button" variant="text">Revoke</Button><Button disabled={!canManagePermissions || feedback.busy || !grant.permission.trim() || !grant.resourceId.trim()} onClick={() => void run("grant")} type="button">Grant</Button></div>
            </> : plan === undefined ? <>
                <p>Review the removal before applying it.</p>
                <Checkbox disabled={feedback.busy} checked={deleteData} label="Delete extension-owned data during uninstall" onChange={setDeleteData} />
                <div className="dialog-actions"><Button disabled={feedback.busy} onClick={close} type="button" variant="text">Cancel</Button><Button disabled={!canManageLifecycle || feedback.busy} onClick={() => void run("plan")} type="button">Create uninstall plan</Button></div>
            </> : <>
                <dl className="dialog-summary"><div><dt>Extension</dt><dd>{plan.extensionId}</dd></div><div><dt>Version</dt><dd>{plan.version}</dd></div><div><dt>Data</dt><dd>{humanize(plan.dataDisposition)}</dd></div><div><dt>Publisher</dt><dd><code>{plan.publisherFingerprint}</code></dd></div></dl>
                <p>The daemon will revalidate this uninstall plan before applying it.</p>
                <div className="dialog-actions"><Button disabled={feedback.busy} onClick={close} type="button" variant="text">Discard plan</Button><Button disabled={!canManageLifecycle || feedback.busy} onClick={() => void run("apply")} type="button">Apply reviewed plan</Button></div>
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
    return <ModalDialog open={open} title={title} onClose={onClose}><p>{detail}</p><div className="dialog-actions"><Button disabled={busy} onClick={onClose} type="button" variant="tonal">Go back</Button><Button widthLabels={["Working…", "Confirm"]} data-dialog-initial-focus disabled={busy} onClick={() => void onConfirm().finally(onClose)} type="button">{busy ? "Working…" : "Confirm"}</Button></div></ModalDialog>;
}

function PlanDialog({ applyDisabled = false, busy, children, onApply, onClose, open, title }: { applyDisabled?: boolean; busy: boolean; children: ReactNode; onApply(): Promise<void>; onClose(): void; open: boolean; title: string }) {
    return <ModalDialog open={open} title={title} onClose={onClose}>{children}<p className="risk-summary">The daemon will revalidate this frozen plan. A stale plan fails instead of being silently replaced.</p><div className="dialog-actions"><Button disabled={busy} onClick={onClose} type="button" variant="tonal">Discard plan</Button><Button widthLabels={["Applying…", "Apply reviewed plan"]} data-dialog-initial-focus disabled={busy || applyDisabled} onClick={() => void onApply()} type="button">{busy ? "Applying…" : "Apply reviewed plan"}</Button></div></ModalDialog>;
}

function ModalDialog({ children, onClose, open, title }: { children: ReactNode; onClose(): void; open: boolean; title: string }) {
    return <MaterialDialog onClose={onClose} open={open} title={title}>{children}</MaterialDialog>;
}

function MutationFeedback({ client, feedback, onOperationTerminal }: { client: GuiRpcClient; feedback: FeedbackState; onOperationTerminal?(operation: Operation): void }) {
    if (feedback.error !== undefined) return <div className="mutation-feedback mutation-feedback--error" role="alert"><strong>Request failed</strong><span><code>{feedback.error.code}</code>{feedback.error.diagnosticId === undefined ? "" : ` · Diagnostic ID ${feedback.error.diagnosticId}`}</span></div>;
    if (feedback.operationId !== undefined) return <OperationFollow client={client} onTerminal={onOperationTerminal} operationId={feedback.operationId} />;
    if (feedback.message !== undefined) return <div className="mutation-feedback" role="status" aria-live="polite">{feedback.message}</div>;
    return null;
}

export function OperationFollow({ client, onTerminal, operationId, title = "Operation" }: { client: GuiRpcClient; onTerminal?(operation: Operation): void; operationId: string; title?: string }) {
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
    return <section className="operation-follow" aria-live="polite" aria-atomic="true" role="status"><h3>{title}</h3>{operation === undefined ? <p>Reading progress…</p> : <><p><strong>{humanize(operation.state)}</strong>{operation.progress?.phase === undefined ? "" : ` · ${humanize(operation.progress.phase)}`}</p><Progress label={`${title} progress`} value={isTerminal(operation.state) ? 1 : undefined} /></>}{error === undefined ? null : <p className="inline-error">Progress is temporarily unavailable. ALCOMD will keep following the same task.</p>}</section>;
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
function packageVersionChange(fromVersion: string | null | undefined, toVersion: string | null | undefined): string {
    if (fromVersion != null && toVersion != null) return `${fromVersion} → ${toVersion}`;
    if (toVersion != null) return `Version ${toVersion}`;
    return `${fromVersion} → Removed`;
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
