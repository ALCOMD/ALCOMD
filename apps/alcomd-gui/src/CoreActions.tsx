import type { ExtensionRecord, RpcError } from "@alcomd/sdk";
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
import { playArrowIcon } from "@alcomd/ui/icons";
import { Button, Checkbox, Dialog as MaterialDialog, Icon, Progress, Select, TextField } from "./Material";
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
    const [confirm, setConfirm] = useState(false);
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const run = async () => {
        const source = kind === "remote" ? { kind, url: value } as const : { kind, path: value } as const;
        await runSimple(setFeedback, () => client.repositoryRegister(source), "Repository registered.", onChanged);
    };
    return (
        <ActionSection title="Add repository">
            <form onSubmit={(event) => { event.preventDefault(); setConfirm(true); }}>
                <Select id="repository-kind" label="Source type" onChange={(next) => setKind(next as "remote" | "local")} options={[{ label: "Remote URL", value: "remote" }, { label: "Local manifest", value: "local" }]} value={kind} />
                <TextField id="repository-source" label={kind === "remote" ? "Repository URL" : "Local manifest path"} maxLength={2048} onInput={setValue} required type={kind === "remote" ? "url" : "text"} value={value} />
                <Button disabled={!available || feedback.busy || value.length === 0} title={capabilityUnavailableTitle(available, capabilities.repositoriesRegistry)} type="submit">Review repository</Button>
            </form>
            <ConfirmDialog busy={feedback.busy} open={confirm} title="Register this repository?" detail="The daemon will validate the source and store its normalized read model." onClose={() => setConfirm(false)} onConfirm={run} />
            <MutationFeedback client={client} feedback={feedback} />
        </ActionSection>
    );
}

export function RepositoryActions({ client, onChanged, repository }: ActionProps & { repository: RepositorySnapshot }) {
    const available = useCapability(capabilities.repositoriesRegistry);
    const [confirmRemove, setConfirmRemove] = useState(false);
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const ready = repository.repositoryId !== undefined && repository.revision !== undefined;
    const refresh = async () => {
        if (!ready) return;
        await runSimple(setFeedback, () => client.repositoryRefresh(repository.repositoryId!, repository.revision!), "Repository refreshed.", onChanged);
    };
    const remove = async () => {
        if (!ready) return;
        await runSimple(setFeedback, () => client.repositoryUnregister(repository.repositoryId!, repository.revision!), "Repository removed from the registry.", onChanged);
    };
    return (
        <ActionSection title="Repository actions">
            <div className="action-row"><Button disabled={!available || !ready || feedback.busy} onClick={() => void refresh()} title={capabilityUnavailableTitle(available, capabilities.repositoriesRegistry)} type="button" variant="tonal">Refresh</Button><Button className="material-button--danger" disabled={!available || !ready || feedback.busy} onClick={() => setConfirmRemove(true)} title={capabilityUnavailableTitle(available, capabilities.repositoriesRegistry)} type="button" variant="text">Remove</Button></div>
            <ConfirmDialog busy={feedback.busy} open={confirmRemove} title="Remove this repository?" detail="Packages already installed in projects are not silently changed." onClose={() => setConfirmRemove(false)} onConfirm={remove} />
            <MutationFeedback client={client} feedback={feedback} />
        </ActionSection>
    );
}

export interface PackageActionSelection {
    action: "install" | "remove" | "upgrade" | "downgrade" | "resolve" | "reinstall" | "reinstall-all" | "bulk-reinstall";
    key: number;
    packageId: string;
    packageIds?: string[];
    source?: PackageSourceSelector;
    sources?: Array<{ packageId: string; source: PackageSourceSelector }>;
    version?: string;
}

export function PackageActions({ client, project, onChanged, selection }: ActionProps & { project: ProjectSnapshot; selection?: PackageActionSelection }) {
    const canApply = useCapability(capabilities.packagesApply);
    const canPlanV1 = useCapability(capabilities.packagesPlanV1);
    const canPlanV2 = useCapability(capabilities.packagesPlanV2);
    const [packageId, setPackageId] = useState("");
    const [version, setVersion] = useState("");
    const [plan, setPlan] = useState<PackagePlan>();
    const [versionDialogOpen, setVersionDialogOpen] = useState(false);
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const handledSelectionKey = useRef<number | undefined>(undefined);
    const revision = project.revision;
    const projectId = project.projectId;
    const prepareChanges = useCallback(async (action: PackageActionSelection["action"], selectedPackageId: string, selectedVersion = "", selectedSource?: PackageSourceSelector, packageIds?: string[], sources?: Array<{ packageId: string; source: PackageSourceSelector }>) => {
        if (revision === undefined || projectId === undefined) return;
        const requiresV2 = action === "reinstall" || action === "reinstall-all" || action === "bulk-reinstall" || action === "downgrade";
        if ((requiresV2 && !canPlanV2) || (!requiresV2 && !canPlanV1)) {
            setFeedback({ busy: false, error: { code: "capability_required", message: `The connected daemon did not negotiate ${requiresV2 ? capabilities.packagesPlanV2 : capabilities.packagesPlanV1}.` } });
            return;
        }
        setPlan(undefined);
        setFeedback({ busy: true });
        try {
            let result: PackagePlan;
            if (action === "remove") result = await client.packagePlanRemove({ projectId, expectedRevision: revision, packageId: selectedPackageId });
            else if (action === "resolve") result = await client.packagePlanResolve({ projectId, expectedRevision: revision, includePrerelease: false });
            else if (action === "reinstall-all") result = await client.packagePlanReinstall({ projectId, expectedRevision: revision, selection: { kind: "all" } });
            else if (action === "reinstall") result = await client.packagePlanReinstall({ projectId, expectedRevision: revision, selection: { kind: "packages", packageIds: [selectedPackageId] }, ...(selectedSource === undefined ? {} : { sources: [{ packageId: selectedPackageId, source: selectedSource }] }) });
            else if (action === "bulk-reinstall") result = await client.packagePlanBulk({ projectId, expectedRevision: revision, intents: (packageIds ?? []).map((packageId) => ({ kind: "reinstall" as const, packageId, ...(sources?.find((item) => item.packageId === packageId)?.source === undefined ? {} : { source: sources.find((item) => item.packageId === packageId)?.source }) })) });
            else if (action === "downgrade") result = await client.packagePlanDowngrade({ projectId, expectedRevision: revision, packageId: selectedPackageId, version: selectedVersion, ...(selectedSource === undefined ? {} : { source: selectedSource }) });
            else {
                const params = { projectId, expectedRevision: revision, packageId: selectedPackageId, includePrerelease: false, ...(selectedVersion.length === 0 ? {} : { versionRange: selectedVersion }), ...(selectedSource === undefined ? {} : { source: selectedSource }) };
                result = action === "upgrade" ? await client.packagePlanUpgrade(params) : await client.packagePlanInstall(params);
            }
            setPlan(result);
            setVersionDialogOpen(false);
            setFeedback({ busy: false });
        } catch (caught: unknown) {
            setVersionDialogOpen(false);
            setFeedback({ busy: false, error: safeError(caught) });
        }
    }, [canPlanV1, canPlanV2, client, projectId, revision]);
    useEffect(() => {
        if (selection === undefined) return;
        if (handledSelectionKey.current === selection.key) return;
        handledSelectionKey.current = selection.key;
        setPackageId(selection.packageId);
        setVersion(selection.version ?? "");
        setPlan(undefined);
        setFeedback(INITIAL_FEEDBACK);
        if (selection.action === "downgrade" && selection.version === undefined) {
            setVersionDialogOpen(true);
            return;
        }
        setVersionDialogOpen(false);
        void prepareChanges(selection.action, selection.packageId, selection.version, selection.source, selection.packageIds, selection.sources);
    }, [prepareChanges, selection]);
    const chooseVersion = (event: FormEvent) => {
        event.preventDefault();
        void prepareChanges("downgrade", packageId, version);
    };
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
    const closeVersionDialog = () => {
        if (feedback.busy) return;
        setVersionDialogOpen(false);
    };
    const closeChanges = () => { if (!feedback.busy) setPlan(undefined); };
    const hasChanges = (plan?.changeSet.mutations.length ?? 0) > 0;
    return (
        <>
            <MaterialDialog onClose={closeVersionDialog} open={versionDialogOpen} title="Choose package version">
                <form className="package-action-form" onSubmit={chooseVersion}>
                    <p>Enter the version you want to use for <strong>{packageId}</strong>.</p>
                    <TextField className="package-action-version" label="Version" maxLength={128} onInput={setVersion} required value={version} />
                    <div className="dialog-actions">
                        <Button disabled={feedback.busy} onClick={closeVersionDialog} type="button" variant="text">Cancel</Button>
                        <Button disabled={feedback.busy || version.length === 0} type="submit">{feedback.busy ? "Checking…" : "Continue"}</Button>
                    </div>
                </form>
            </MaterialDialog>
            <MaterialDialog onClose={closeChanges} open={plan !== undefined} title={hasChanges ? "Apply package changes?" : "Packages are up to date"}>
                {plan === undefined ? null : hasChanges ? (
                    <div className="package-plan-review">
                        <p>Review the changes ALCOMD will make to this project.</p>
                        <ul className="change-list">{plan.changeSet.mutations.map((mutation) => <li key={`${mutation.kind}-${mutation.packageId}`}><strong>{packageChangeLabel(mutation.kind)}</strong><span>{mutation.packageId}</span>{mutation.fromVersion === undefined && mutation.toVersion === undefined ? null : <small>{packageVersionChange(mutation.fromVersion, mutation.toVersion)}</small>}</li>)}</ul>
                        <div className="dialog-actions">
                            <Button disabled={feedback.busy} onClick={closeChanges} type="button" variant="text">Cancel</Button>
                            <Button disabled={!canApply || feedback.busy} onClick={() => void apply()} title={capabilityUnavailableTitle(canApply, capabilities.packagesApply)} type="button">{feedback.busy ? "Applying…" : "Apply changes"}</Button>
                        </div>
                    </div>
                ) : (
                    <div className="package-plan-review"><p>No package changes are required for this project.</p><div className="dialog-actions"><Button onClick={closeChanges} type="button">Close</Button></div></div>
                )}
            </MaterialDialog>
            {feedback.busy && plan === undefined && !versionDialogOpen ? <div className="mutation-feedback" role="status" aria-live="polite">Checking package changes…</div> : null}
            {feedback.error === undefined ? null : <div className="mutation-feedback mutation-feedback--error" role="alert"><strong>Package changes were not applied</strong><span>{packageErrorMessage(feedback.error)}</span></div>}
            {feedback.operationId === undefined ? null : <OperationFollow client={client} operationId={feedback.operationId} title="Package changes" />}
        </>
    );
}

export function UnityRegistryActions({ client, installations, onChanged }: ActionProps & { installations: UnityInstallation[] }) {
    const available = useCapability(capabilities.unityManage);
    const [path, setPath] = useState("");
    const [remove, setRemove] = useState<UnityInstallation>();
    const [confirmRemove, setConfirmRemove] = useState(false);
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const register = async (event: FormEvent) => {
        event.preventDefault();
        await runSimple(setFeedback, () => client.unityInstallationRegister(path), "Unity installation registered.", onChanged);
    };
    const removeInstallation = async () => {
        if (remove === undefined) return;
        await runSimple(setFeedback, () => client.unityInstallationRemove(remove.installationId, remove.revision), "Unity installation removed.", onChanged);
        setRemove(undefined);
        setConfirmRemove(false);
    };
    return (
        <ActionSection title="Installation registry">
            <form onSubmit={(event) => void register(event)}>
                <TextField id="unity-executable" label="Unity executable" maxLength={1024} onInput={setPath} required value={path} />
                <div className="action-row">
                    <Button disabled={!available || feedback.busy} title={capabilityUnavailableTitle(available, capabilities.unityManage)} type="submit">Register</Button>
                    <Button disabled={!available || feedback.busy} onClick={() => void runSimple(setFeedback, () => client.unityInstallationsRefresh(), "Unity registry refreshed.", onChanged)} title={capabilityUnavailableTitle(available, capabilities.unityManage)} type="button" variant="tonal">Discover and refresh</Button>
                </div>
            </form>
            {installations.length === 0 ? null : <Select aria-label="Installation to remove" label="Remove installation" onChange={(next) => setRemove(installations.find((item) => item.installationId === next))} options={[{ label: "Select an installation", value: "" }, ...installations.map((item) => ({ label: `Unity ${item.unityVersion}`, value: item.installationId }))]} value={remove?.installationId ?? ""} />}
            <Button className="material-button--danger" disabled={!available || remove === undefined || feedback.busy} onClick={() => setConfirmRemove(true)} title={capabilityUnavailableTitle(available, capabilities.unityManage)} type="button" variant="text">Review removal</Button>
            <ConfirmDialog busy={feedback.busy} open={confirmRemove} title="Remove this Unity installation?" detail="Only the ALCOMD registry entry is removed. The editor remains installed." onClose={() => setConfirmRemove(false)} onConfirm={removeInstallation} />
            <MutationFeedback client={client} feedback={feedback} />
        </ActionSection>
    );
}

export function ProjectUnityActions({ afterMigrationOpen = false, client, installations, launchConfig, launchOptions, project, onChanged }: ActionProps & { afterMigrationOpen?: boolean; installations: UnityInstallation[]; launchConfig: ProjectUnityLaunchConfig; launchOptions: UnityLaunchOptionsResult; project: ProjectSnapshot }) {
    const canLaunch = useCapability(capabilities.unityLaunch);
    const canManage = useCapability(capabilities.unityManage);
    const canMigrate = useCapability(capabilities.projectsUnityMigration);
    const [argumentsText, setArgumentsText] = useState(launchConfig.arguments.join("\n"));
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
        setArgumentsText(launchConfig.arguments.join("\n"));
    }, [launchConfig]);
    const migrationVersions = [...new Set(installations.map((installation) => installation.unityVersion))]
        .filter((version) => version !== project.unityVersion)
        .sort((left, right) => left.localeCompare(right));
    const targetInstallations = installations.filter((installation) => installation.unityVersion === targetVersion);
    const setLaunchConfig = async (event: FormEvent) => {
        event.preventDefault();
        if (projectId === undefined) return;
        const arguments_ = argumentsText.split(/\r?\n/).map((value) => value.trim()).filter(Boolean);
        setFeedback({ busy: true });
        try {
            await client.unityProjectLaunchConfigSet(projectId, arguments_, launchConfig.revision);
            setFeedback({ busy: false, message: "Unity launch arguments updated." });
            onChanged?.();
        } catch (caught: unknown) {
            setFeedback({ busy: false, error: safeError(caught) });
        }
    };
    const clearLaunchConfig = async () => {
        if (projectId === undefined) return;
        await runSimple(setFeedback, () => client.unityProjectLaunchConfigClear(projectId, launchConfig.revision), "Unity launch arguments cleared.", onChanged);
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
            <form onSubmit={(event) => void setLaunchConfig(event)}>
                <TextField aria-describedby="unity-arguments-hint" id="unity-arguments" label="Additional arguments" maxLength={4096} onInput={setArgumentsText} rows={4} supportingText="One argument per line. The daemon validates forbidden arguments." type="textarea" value={argumentsText} />
                <div className="action-row">
                    <Button disabled={!canManage || feedback.busy || projectId === undefined} title={capabilityUnavailableTitle(canManage, capabilities.unityManage)} type="submit" variant="tonal">Save launch arguments</Button>
                    <Button disabled={!canManage || feedback.busy || projectId === undefined || launchConfig.revision === 0} onClick={() => void clearLaunchConfig()} title={capabilityUnavailableTitle(canManage, capabilities.unityManage)} type="button" variant="text">Clear launch arguments</Button>
                </div>
            </form>
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
    const [expectedRevision, setExpectedRevision] = useState(0);
    const [overrideExisting, setOverrideExisting] = useState(false);
    const [plan, setPlan] = useState<TemplatePlan>();
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const create = async (event: FormEvent) => {
        event.preventDefault(); setFeedback({ busy: true });
        try { setPlan(await client.templatePlanImport(bundlePath, overrideExisting, expectedRevision)); setFeedback({ busy: false }); }
        catch (caught: unknown) { setFeedback({ busy: false, error: safeError(caught) }); }
    };
    const apply = async () => {
        if (plan === undefined) return;
        setFeedback({ busy: true });
        try { const result = await client.templateApplyImport(plan.planId); setPlan(undefined); setFeedback({ busy: false, operationId: result.operationId, message: "Template import accepted." }); onChanged?.(); }
        catch (caught: unknown) { setPlan(undefined); setFeedback({ busy: false, error: safeError(caught) }); }
    };
    return <ActionSection title="Import template"><form onSubmit={(event) => void create(event)}><TextField id="template-bundle" label="Template bundle" maxLength={1024} onInput={setBundlePath} required value={bundlePath} /><TextField id="template-registry-revision" label="Expected registry revision" min={0} onInput={(next) => setExpectedRevision(Number(next))} required type="number" value={expectedRevision} /><Checkbox checked={overrideExisting} label="Replace an existing matching template" onChange={setOverrideExisting} /><Button disabled={!available || feedback.busy} title={capabilityUnavailableTitle(available, capabilities.templatesManage)} type="submit">Create import plan</Button></form><TemplatePlanDialog busy={feedback.busy || !available} plan={plan} title="Review template import" onApply={apply} onClose={() => setPlan(undefined)} /><MutationFeedback client={client} feedback={feedback} /></ActionSection>;
}

export function TemplateActions({ client, onChanged, template }: ActionProps & { template: TemplateRecord }) {
    const canCreateProject = useCapability(capabilities.templatesCreateProject);
    const canManage = useCapability(capabilities.templatesManage);
    const [mode, setMode] = useState<"none" | "derive" | "create">("none");
    const [plan, setPlan] = useState<TemplatePlan>();
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const [fields, setFields] = useState({ projectId: "", projectRevision: 0, templateId: "", templateVersion: "1.0.0", displayName: "", description: "", parent: "", leaf: "", exportPath: "" });
    const update = (key: keyof typeof fields, value: string | number) => setFields((current) => ({ ...current, [key]: value }));
    const createPlan = async (event: FormEvent) => {
        event.preventDefault(); setFeedback({ busy: true });
        try {
            const next = mode === "derive"
                ? await client.templatePlanDerive({ projectId: fields.projectId, expectedProjectRevision: fields.projectRevision, templateId: fields.templateId, templateVersion: fields.templateVersion, displayName: fields.displayName, ...(fields.description.length === 0 ? {} : { description: fields.description }) })
                : await client.templatePlanCreateProject(template.templateId, template.revision, fields.parent, fields.leaf);
            setPlan(next); setFeedback({ busy: false });
        } catch (caught: unknown) { setFeedback({ busy: false, error: safeError(caught) }); }
    };
    const apply = async () => {
        if (plan === undefined) return; setFeedback({ busy: true });
        try { const result = mode === "derive" ? await client.templateApplyDerive(plan.planId) : await client.templateApplyCreateProject(plan.planId); setPlan(undefined); setFeedback({ busy: false, operationId: result.operationId, message: `Template ${mode} operation accepted.` }); onChanged?.(); }
        catch (caught: unknown) { setPlan(undefined); setFeedback({ busy: false, error: safeError(caught) }); }
    };
    const simple = async (kind: "favorite" | "export" | "remove") => {
        if (kind === "favorite") await runSimple(setFeedback, () => client.templateSetFavorite(template.templateId, !template.favorite, template.revision), "Favorite state updated.", onChanged);
        if (kind === "export") await runSimple(setFeedback, () => client.templateExport(template.templateId, template.revision, fields.exportPath), "Template exported.");
        if (kind === "remove") await runSimple(setFeedback, () => client.templateRemove(template.templateId, template.revision), "Template removed.", onChanged);
    };
    return <ActionSection title="Template workflows"><div className="action-row"><Button disabled={!canManage} onClick={() => setMode("derive")} title={capabilityUnavailableTitle(canManage, capabilities.templatesManage)} type="button" variant="tonal">Derive from project</Button><Button disabled={!canCreateProject} onClick={() => setMode("create")} title={capabilityUnavailableTitle(canCreateProject, capabilities.templatesCreateProject)} type="button" variant="tonal">Create project</Button><Button disabled={!canManage} onClick={() => void simple("favorite")} title={capabilityUnavailableTitle(canManage, capabilities.templatesManage)} type="button" variant="tonal">{template.favorite ? "Remove favorite" : "Favorite"}</Button></div>{mode === "none" ? null : <form onSubmit={(event) => void createPlan(event)}>{mode === "derive" ? <><TextField id="derive-project" label="Source project ID" onInput={(next) => update("projectId", next)} required value={fields.projectId} /><TextField id="derive-project-revision" label="Expected project revision" min={0} onInput={(next) => update("projectRevision", Number(next))} required type="number" value={fields.projectRevision} /><TextField id="derive-template-id" label="New template ID" onInput={(next) => update("templateId", next)} required value={fields.templateId} /><TextField id="derive-version" label="Template version" onInput={(next) => update("templateVersion", next)} required value={fields.templateVersion} /><TextField id="derive-name" label="Display name" onInput={(next) => update("displayName", next)} required value={fields.displayName} /></> : <><TextField id="create-parent" label="Target parent" onInput={(next) => update("parent", next)} required value={fields.parent} /><TextField id="create-leaf" label="Target directory name" onInput={(next) => update("leaf", next)} required value={fields.leaf} /></>}<Button disabled={(mode === "derive" ? !canManage : !canCreateProject) || feedback.busy} type="submit">Create {mode} plan</Button></form>}<form onSubmit={(event) => { event.preventDefault(); void simple("export"); }}><TextField id="template-export" label="Export target" onInput={(next) => update("exportPath", next)} required value={fields.exportPath} /><Button disabled={!canManage || feedback.busy} type="submit" variant="tonal">Export</Button></form>{template.sourceKind === "builtin" ? null : <Button className="material-button--danger" disabled={!canManage || feedback.busy} onClick={() => void simple("remove")} type="button" variant="text">Remove template</Button>}<TemplatePlanDialog busy={feedback.busy || (mode === "derive" ? !canManage : !canCreateProject)} plan={plan} title={`Review template ${mode}`} onApply={apply} onClose={() => setPlan(undefined)} /><MutationFeedback client={client} feedback={feedback} /></ActionSection>;
}

export function BackupCreatePanel({ client, onChanged, project }: ActionProps & { project: ProjectSnapshot }) {
    const available = useCapability(capabilities.backupsCreate);
    const [compression, setCompression] = useState<"store" | "fast" | "maximum">("fast");
    const [exclude, setExclude] = useState(true);
    const [confirm, setConfirm] = useState(false);
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const create = async () => {
        if (project.projectId === undefined || project.revision === undefined) return;
        setFeedback({ busy: true });
        try { const result = await client.backupCreate(project.projectId, project.revision, compression, exclude); setFeedback({ busy: false, operationId: result.operationId, message: "Backup operation accepted." }); onChanged?.(); }
        catch (caught: unknown) { setFeedback({ busy: false, error: safeError(caught) }); }
    };
    return <ActionSection title="Create backup"><Select id="backup-compression" label="Compression" onChange={(next) => setCompression(next as typeof compression)} options={[{ label: "Store", value: "store" }, { label: "Fast", value: "fast" }, { label: "Maximum", value: "maximum" }]} value={compression} /><Checkbox checked={exclude} label="Exclude VPM packages" onChange={setExclude} /><Button disabled={!available || feedback.busy || project.revision === undefined} onClick={() => setConfirm(true)} title={capabilityUnavailableTitle(available, capabilities.backupsCreate)} type="button">Review backup</Button><ConfirmDialog busy={feedback.busy || !available} open={confirm} title="Create this backup?" detail={`Compression: ${compression}. ${exclude ? "VPM packages will be excluded." : "VPM packages will be included."}`} onClose={() => setConfirm(false)} onConfirm={create} /><MutationFeedback client={client} feedback={feedback} /></ActionSection>;
}

export function BackupRestorePanel({ backup, client }: ActionProps & { backup: BackupRecord }) {
    const available = useCapability(capabilities.backupsRestore);
    const [parent, setParent] = useState("");
    const [leaf, setLeaf] = useState("");
    const [plan, setPlan] = useState<BackupRestorePlan>();
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const makePlan = async (event: FormEvent) => { event.preventDefault(); setFeedback({ busy: true }); try { setPlan(await client.backupPlanRestore(backup.backupId, parent, leaf)); setFeedback({ busy: false }); } catch (caught: unknown) { setFeedback({ busy: false, error: safeError(caught) }); } };
    const apply = async () => { if (plan === undefined) return; setFeedback({ busy: true }); try { const result = await client.backupApplyRestore(plan.planId); setPlan(undefined); setFeedback({ busy: false, operationId: result.operationId, message: "Restore operation accepted." }); } catch (caught: unknown) { setPlan(undefined); setFeedback({ busy: false, error: safeError(caught) }); } };
    return <ActionSection title="Restore backup"><form onSubmit={(event) => void makePlan(event)}><TextField id="restore-parent" label="Target parent" maxLength={1024} onInput={setParent} required value={parent} /><TextField id="restore-leaf" label="New directory name" maxLength={255} onInput={setLeaf} required value={leaf} /><Button disabled={!available || feedback.busy} title={capabilityUnavailableTitle(available, capabilities.backupsRestore)} type="submit">Create restore plan</Button></form><PlanDialog busy={feedback.busy || !available} open={plan !== undefined} title="Review backup restore" onClose={() => setPlan(undefined)} onApply={apply}>{plan === undefined ? null : <><p>Restore to <strong>{plan.target.leaf}</strong> in the selected parent directory.</p><p>The target must be absent: {plan.target.mustBeAbsent ? "yes" : "no"}</p><p>{plan.packagesRequireResolve ? "VPM packages require a separate resolve after restoration." : "No package resolve is required."}</p><p>Archive: <code>{shortValue(plan.archiveSha256)}</code></p></>}</PlanDialog><MutationFeedback client={client} feedback={feedback} /></ActionSection>;
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
    const [expectedRevision, setExpectedRevision] = useState(0);
    const [plan, setPlan] = useState<ExtensionPlan>();
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const makePlan = async (event: FormEvent) => { event.preventDefault(); setFeedback({ busy: true }); try { const result = await client.extensionPlanInstall(path, expectedRevision, approvePublisher ? "approve_for_extension" : "none"); setPlan(result.plan); setFeedback({ busy: false }); } catch (caught: unknown) { setFeedback({ busy: false, error: safeError(caught) }); } };
    const apply = async () => { if (plan === undefined) return; setFeedback({ busy: true }); try { const result = await client.extensionApplyInstall(plan.planId); setPlan(undefined); setFeedback({ busy: false, operationId: result.operationId, message: "Extension install accepted." }); onChanged?.(); } catch (caught: unknown) { setPlan(undefined); setFeedback({ busy: false, error: safeError(caught) }); } };
    return <ActionSection title="Install extension"><form onSubmit={(event) => void makePlan(event)}><TextField id="extension-package" label="Extension package" maxLength={1024} onInput={setPath} required value={path} /><TextField id="extension-registry-revision" label="Expected registry revision" min={0} onInput={(next) => setExpectedRevision(Number(next))} required type="number" value={expectedRevision} /><Checkbox checked={approvePublisher} label="Approve this publisher for this extension" onChange={setApprovePublisher} /><Button disabled={!available || feedback.busy} title={capabilityUnavailableTitle(available, capabilities.extensionsLifecycle)} type="submit">Create install plan</Button></form><ExtensionPlanDialog busy={feedback.busy || !available} plan={plan} title="Review extension install" onApply={apply} onClose={() => setPlan(undefined)} /><MutationFeedback client={client} feedback={feedback} /></ActionSection>;
}

export function ExtensionActions({ client, extension, onChanged }: ActionProps & { extension: ExtensionRecord }) {
    const canManageLifecycle = useCapability(capabilities.extensionsLifecycle);
    const canManagePermissions = useCapability(capabilities.extensionsPermissions);
    const [plan, setPlan] = useState<ExtensionPlan>();
    const [deleteData, setDeleteData] = useState(false);
    const [grant, setGrant] = useState({ permission: "", resourceKind: "Project", resourceId: "" });
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const lifecycle = async (enable: boolean) => runSimple(setFeedback, () => enable ? client.extensionEnable(extension.extensionId, extension.revision) : client.extensionDisable(extension.extensionId, extension.revision), `Extension ${enable ? "enabled" : "disabled"}.`, onChanged);
    const planUninstall = async () => { setFeedback({ busy: true }); try { const result = await client.extensionPlanUninstall(extension.extensionId, extension.revision, deleteData ? "delete_data" : "retain_data"); setPlan(result.plan); setFeedback({ busy: false }); } catch (caught: unknown) { setFeedback({ busy: false, error: safeError(caught) }); } };
    const applyUninstall = async () => { if (plan === undefined) return; setFeedback({ busy: true }); try { const result = await client.extensionApplyUninstall(plan.planId); setPlan(undefined); setFeedback({ busy: false, operationId: result.operationId, message: "Extension uninstall accepted." }); onChanged?.(); } catch (caught: unknown) { setPlan(undefined); setFeedback({ busy: false, error: safeError(caught) }); } };
    const changeGrant = async (revoke: boolean) => runSimple(setFeedback, () => revoke ? client.extensionRevokeGrant(extension.extensionId, grant.permission, grant.resourceKind, grant.resourceId, extension.grantRevision) : client.extensionSetGrant(extension.extensionId, grant.permission, grant.resourceKind, grant.resourceId, extension.grantRevision), `Permission ${revoke ? "revoked" : "granted"}.`, onChanged);
    const updateGrant = (key: keyof typeof grant, value: string) => setGrant((current) => ({ ...current, [key]: value }));
    return <ActionSection title="Extension management"><div className="action-row"><Button disabled={!canManageLifecycle || feedback.busy || extension.desiredState === "enabled"} onClick={() => void lifecycle(true)} title={capabilityUnavailableTitle(canManageLifecycle, capabilities.extensionsLifecycle)} type="button" variant="tonal">Enable</Button><Button disabled={!canManageLifecycle || feedback.busy || extension.desiredState !== "enabled"} onClick={() => void lifecycle(false)} title={capabilityUnavailableTitle(canManageLifecycle, capabilities.extensionsLifecycle)} type="button" variant="tonal">Disable</Button></div><fieldset><legend>Permission scope</legend><TextField id="grant-permission" label="Permission" onInput={(next) => updateGrant("permission", next)} required value={grant.permission} /><Select id="grant-kind" label="Resource kind" onChange={(next) => updateGrant("resourceKind", next)} options={[{ label: "Project", value: "Project" }, { label: "Extension", value: "Extension" }]} value={grant.resourceKind} /><TextField id="grant-resource" label="Resource ID" onInput={(next) => updateGrant("resourceId", next)} required value={grant.resourceId} /><div className="action-row"><Button disabled={!canManagePermissions || feedback.busy || grant.permission.length === 0 || grant.resourceId.length === 0} onClick={() => void changeGrant(false)} title={capabilityUnavailableTitle(canManagePermissions, capabilities.extensionsPermissions)} type="button" variant="tonal">Grant</Button><Button className="material-button--danger" disabled={!canManagePermissions || feedback.busy || grant.permission.length === 0 || grant.resourceId.length === 0} onClick={() => void changeGrant(true)} title={capabilityUnavailableTitle(canManagePermissions, capabilities.extensionsPermissions)} type="button" variant="text">Revoke</Button></div></fieldset><Checkbox checked={deleteData} label="Delete extension-owned data during uninstall" onChange={setDeleteData} /><Button className="material-button--danger" disabled={!canManageLifecycle || feedback.busy} onClick={() => void planUninstall()} title={capabilityUnavailableTitle(canManageLifecycle, capabilities.extensionsLifecycle)} type="button" variant="text">Create uninstall plan</Button><ExtensionPlanDialog busy={feedback.busy || !canManageLifecycle} plan={plan} title="Review extension uninstall" onApply={applyUninstall} onClose={() => setPlan(undefined)} /><MutationFeedback client={client} feedback={feedback} /></ActionSection>;
}

function ActionSection({ children, title }: { children: ReactNode; title: string }) {
    return <section className="action-section"><h2>{title}</h2>{children}</section>;
}

function ConfirmDialog({ busy, detail, onClose, onConfirm, open, title }: { busy: boolean; detail: string; onClose(): void; onConfirm(): Promise<void>; open: boolean; title: string }) {
    return <ModalDialog open={open} title={title} onClose={onClose}><p>{detail}</p><div className="dialog-actions"><Button disabled={busy} onClick={onClose} type="button" variant="tonal">Go back</Button><Button data-dialog-initial-focus disabled={busy} onClick={() => void onConfirm().finally(onClose)} type="button">{busy ? "Working…" : "Confirm"}</Button></div></ModalDialog>;
}

function PlanDialog({ applyDisabled = false, busy, children, onApply, onClose, open, title }: { applyDisabled?: boolean; busy: boolean; children: ReactNode; onApply(): Promise<void>; onClose(): void; open: boolean; title: string }) {
    return <ModalDialog open={open} title={title} onClose={onClose}>{children}<p className="risk-summary">The daemon will revalidate this frozen plan. A stale plan fails instead of being silently replaced.</p><div className="dialog-actions"><Button disabled={busy} onClick={onClose} type="button" variant="tonal">Discard plan</Button><Button data-dialog-initial-focus disabled={busy || applyDisabled} onClick={() => void onApply()} type="button">{busy ? "Applying…" : "Apply reviewed plan"}</Button></div></ModalDialog>;
}

function TemplatePlanDialog(props: { busy: boolean; onApply(): Promise<void>; onClose(): void; plan?: TemplatePlan; title: string }) {
    return <PlanDialog busy={props.busy} open={props.plan !== undefined} title={props.title} onApply={props.onApply} onClose={props.onClose}>{props.plan === undefined ? null : <><p>Action: <strong>{humanize(props.plan.action)}</strong></p><p>Plan fingerprint: <code>{shortValue(props.plan.planFingerprint)}</code></p></>}</PlanDialog>;
}

function ExtensionPlanDialog(props: { busy: boolean; onApply(): Promise<void>; onClose(): void; plan?: ExtensionPlan; title: string }) {
    return <PlanDialog busy={props.busy} open={props.plan !== undefined} title={props.title} onApply={props.onApply} onClose={props.onClose}>{props.plan === undefined ? null : <dl className="dialog-summary"><div><dt>Extension</dt><dd>{props.plan.extensionId}</dd></div><div><dt>Version</dt><dd>{props.plan.version}</dd></div><div><dt>Publisher</dt><dd><code>{shortValue(props.plan.publisherFingerprint)}</code></dd></div><div><dt>Trust</dt><dd>{humanize(props.plan.trustDecision)}</dd></div><div><dt>Data</dt><dd>{humanize(props.plan.dataDisposition)}</dd></div></dl>}</PlanDialog>;
}

function ModalDialog({ children, onClose, open, title }: { children: ReactNode; onClose(): void; open: boolean; title: string }) {
    return <MaterialDialog onClose={onClose} open={open} title={title}><div className="modal-dialog-content">{children}</div></MaterialDialog>;
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
function packageVersionChange(fromVersion: string | undefined, toVersion: string | undefined): string {
    if (fromVersion !== undefined && toVersion !== undefined) return `${fromVersion} → ${toVersion}`;
    if (toVersion !== undefined) return `Version ${toVersion}`;
    return `Version ${fromVersion}`;
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
