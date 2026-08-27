import type { ExtensionRecord, RpcError } from "@alcomd/sdk";
import { useCallback, useEffect, useRef, useState, type FormEvent, type ReactNode } from "react";

import type {
    BackupRecord,
    BackupRestorePlan,
    ExtensionPlan,
    Operation,
    PackagePlan,
    ProjectSnapshot,
    RepositorySnapshot,
    TemplatePlan,
    TemplateRecord,
    UnityInstallation
} from "./core-models";
import type { GuiRpcClient } from "./rpc";
import { Button, Checkbox, Dialog as MaterialDialog, Progress, Select, TextField } from "./Material";

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
                <Button disabled={feedback.busy || path.length === 0} type="submit">Review registration</Button>
            </form>
            <ConfirmDialog busy={feedback.busy} open={confirm} title="Register this project?" detail="ALCOMD will inspect the selected root and add it to the per-user registry." onClose={() => setConfirm(false)} onConfirm={run} />
            <MutationFeedback client={client} feedback={feedback} />
        </ActionSection>
    );
}

export function ProjectActions({ client, onChanged, project }: ActionProps & { project: ProjectSnapshot }) {
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
                <Button disabled={feedback.busy || revision === undefined} onClick={() => void refresh()} type="button" variant="tonal">Refresh</Button>
                <Button className="material-button--danger" disabled={feedback.busy || revision === undefined} onClick={() => setConfirmUnregister(true)} type="button" variant="text">Unregister</Button>
            </div>
            <ConfirmDialog busy={feedback.busy} open={confirmUnregister} title="Unregister this project?" detail="This removes only the ALCOMD registry entry. It does not delete the Unity project." onClose={() => setConfirmUnregister(false)} onConfirm={unregister} />
            <MutationFeedback client={client} feedback={feedback} />
        </ActionSection>
    );
}

export function RegisterRepositoryPanel({ client, onChanged }: ActionProps) {
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
                <Button disabled={feedback.busy || value.length === 0} type="submit">Review repository</Button>
            </form>
            <ConfirmDialog busy={feedback.busy} open={confirm} title="Register this repository?" detail="The daemon will validate the source and store its normalized read model." onClose={() => setConfirm(false)} onConfirm={run} />
            <MutationFeedback client={client} feedback={feedback} />
        </ActionSection>
    );
}

export function RepositoryActions({ client, onChanged, repository }: ActionProps & { repository: RepositorySnapshot }) {
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
            <div className="action-row"><Button disabled={!ready || feedback.busy} onClick={() => void refresh()} type="button" variant="tonal">Refresh</Button><Button className="material-button--danger" disabled={!ready || feedback.busy} onClick={() => setConfirmRemove(true)} type="button" variant="text">Remove</Button></div>
            <ConfirmDialog busy={feedback.busy} open={confirmRemove} title="Remove this repository?" detail="Packages already installed in projects are not silently changed." onClose={() => setConfirmRemove(false)} onConfirm={remove} />
            <MutationFeedback client={client} feedback={feedback} />
        </ActionSection>
    );
}

export interface PackageActionSelection {
    action: "install" | "remove" | "upgrade" | "downgrade" | "resolve";
    key: number;
    packageId: string;
    version?: string;
}

export function PackageActions({ client, project, onChanged, selection }: ActionProps & { project: ProjectSnapshot; selection?: PackageActionSelection }) {
    const [packageId, setPackageId] = useState("");
    const [version, setVersion] = useState("");
    const [plan, setPlan] = useState<PackagePlan>();
    const [versionDialogOpen, setVersionDialogOpen] = useState(false);
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const handledSelectionKey = useRef<number | undefined>(undefined);
    const revision = project.revision;
    const projectId = project.projectId;
    const prepareChanges = useCallback(async (action: PackageActionSelection["action"], selectedPackageId: string, selectedVersion = "") => {
        if (revision === undefined || projectId === undefined) return;
        setPlan(undefined);
        setFeedback({ busy: true });
        try {
            let result: PackagePlan;
            if (action === "remove") result = await client.packagePlanRemove({ projectId, expectedRevision: revision, packageId: selectedPackageId });
            else if (action === "resolve") result = await client.packagePlanResolve({ projectId, expectedRevision: revision, includePrerelease: false });
            else if (action === "downgrade") result = await client.packagePlanDowngrade({ projectId, expectedRevision: revision, packageId: selectedPackageId, version: selectedVersion });
            else {
                const params = { projectId, expectedRevision: revision, packageId: selectedPackageId, includePrerelease: false, ...(selectedVersion.length === 0 ? {} : { versionRange: selectedVersion }) };
                result = action === "upgrade" ? await client.packagePlanUpgrade(params) : await client.packagePlanInstall(params);
            }
            setPlan(result);
            setVersionDialogOpen(false);
            setFeedback({ busy: false });
        } catch (caught: unknown) {
            setVersionDialogOpen(false);
            setFeedback({ busy: false, error: safeError(caught) });
        }
    }, [client, projectId, revision]);
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
        void prepareChanges(selection.action, selection.packageId, selection.version);
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
                            <Button disabled={feedback.busy} onClick={() => void apply()} type="button">{feedback.busy ? "Applying…" : "Apply changes"}</Button>
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
                    <Button disabled={feedback.busy} type="submit">Register</Button>
                    <Button disabled={feedback.busy} onClick={() => void runSimple(setFeedback, () => client.unityInstallationsRefresh(), "Unity registry refreshed.", onChanged)} type="button" variant="tonal">Discover and refresh</Button>
                </div>
            </form>
            {installations.length === 0 ? null : <Select aria-label="Installation to remove" label="Remove installation" onChange={(next) => setRemove(installations.find((item) => item.installationId === next))} options={[{ label: "Select an installation", value: "" }, ...installations.map((item) => ({ label: `Unity ${item.unityVersion}`, value: item.installationId }))]} value={remove?.installationId ?? ""} />}
            <Button className="material-button--danger" disabled={remove === undefined || feedback.busy} onClick={() => setConfirmRemove(true)} type="button" variant="text">Review removal</Button>
            <ConfirmDialog busy={feedback.busy} open={confirmRemove} title="Remove this Unity installation?" detail="Only the ALCOMD registry entry is removed. The editor remains installed." onClose={() => setConfirmRemove(false)} onConfirm={removeInstallation} />
            <MutationFeedback client={client} feedback={feedback} />
        </ActionSection>
    );
}

export function ProjectUnityActions({ client, installations, project, preferenceRevision, onChanged }: ActionProps & { installations: UnityInstallation[]; project: ProjectSnapshot; preferenceRevision: number }) {
    const [installationId, setInstallationId] = useState("");
    const [argumentsText, setArgumentsText] = useState("");
    const [confirmLaunch, setConfirmLaunch] = useState(false);
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const projectId = project.projectId;
    const projectRevision = project.revision;
    const setEditor = async (event: FormEvent) => {
        event.preventDefault();
        if (projectId === undefined) return;
        const arguments_ = argumentsText.split(/\r?\n/).map((value) => value.trim()).filter(Boolean);
        await runSimple(setFeedback, () => client.unityProjectEditorSet(projectId, installationId, arguments_, preferenceRevision), "Project editor preference updated.", onChanged);
    };
    const launch = async () => {
        if (projectId === undefined || projectRevision === undefined) return;
        setFeedback({ busy: true });
        try {
            const result = await client.unityLaunch(projectId, projectRevision);
            setFeedback({ busy: false, message: `Unity launch ${result.launch.state}.` });
        } catch (caught: unknown) { setFeedback({ busy: false, error: safeError(caught) }); }
    };
    return (
        <ActionSection title="Unity actions">
            <form onSubmit={(event) => void setEditor(event)}>
                <Select id="project-editor" label="Selected editor" onChange={setInstallationId} options={[{ label: "Choose an editor", value: "" }, ...installations.map((item) => ({ label: `Unity ${item.unityVersion} (${item.architecture})`, value: item.installationId }))]} required value={installationId} />
                <TextField aria-describedby="unity-arguments-hint" id="unity-arguments" label="Additional arguments" maxLength={4096} onInput={setArgumentsText} rows={4} supportingText="One argument per line. The daemon validates forbidden arguments." type="textarea" value={argumentsText} />
                <Button disabled={feedback.busy || projectId === undefined} type="submit" variant="tonal">Save editor preference</Button>
            </form>
            <Button disabled={feedback.busy || projectRevision === undefined} onClick={() => setConfirmLaunch(true)} type="button">Launch Unity</Button>
            <ConfirmDialog busy={feedback.busy} open={confirmLaunch} title="Launch this project in Unity?" detail="ALCOMD will revalidate the selected editor, project revision, and writer observation." onClose={() => setConfirmLaunch(false)} onConfirm={launch} />
            <MutationFeedback client={client} feedback={feedback} />
        </ActionSection>
    );
}

export function TemplateImportPanel({ client, onChanged }: ActionProps) {
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
    return <ActionSection title="Import template"><form onSubmit={(event) => void create(event)}><TextField id="template-bundle" label="Template bundle" maxLength={1024} onInput={setBundlePath} required value={bundlePath} /><TextField id="template-registry-revision" label="Expected registry revision" min={0} onInput={(next) => setExpectedRevision(Number(next))} required type="number" value={expectedRevision} /><Checkbox checked={overrideExisting} label="Replace an existing matching template" onChange={setOverrideExisting} /><Button disabled={feedback.busy} type="submit">Create import plan</Button></form><TemplatePlanDialog busy={feedback.busy} plan={plan} title="Review template import" onApply={apply} onClose={() => setPlan(undefined)} /><MutationFeedback client={client} feedback={feedback} /></ActionSection>;
}

export function TemplateActions({ client, onChanged, template }: ActionProps & { template: TemplateRecord }) {
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
    return <ActionSection title="Template workflows"><div className="action-row"><Button onClick={() => setMode("derive")} type="button" variant="tonal">Derive from project</Button><Button onClick={() => setMode("create")} type="button" variant="tonal">Create project</Button><Button onClick={() => void simple("favorite")} type="button" variant="tonal">{template.favorite ? "Remove favorite" : "Favorite"}</Button></div>{mode === "none" ? null : <form onSubmit={(event) => void createPlan(event)}>{mode === "derive" ? <><TextField id="derive-project" label="Source project ID" onInput={(next) => update("projectId", next)} required value={fields.projectId} /><TextField id="derive-project-revision" label="Expected project revision" min={0} onInput={(next) => update("projectRevision", Number(next))} required type="number" value={fields.projectRevision} /><TextField id="derive-template-id" label="New template ID" onInput={(next) => update("templateId", next)} required value={fields.templateId} /><TextField id="derive-version" label="Template version" onInput={(next) => update("templateVersion", next)} required value={fields.templateVersion} /><TextField id="derive-name" label="Display name" onInput={(next) => update("displayName", next)} required value={fields.displayName} /></> : <><TextField id="create-parent" label="Target parent" onInput={(next) => update("parent", next)} required value={fields.parent} /><TextField id="create-leaf" label="Target directory name" onInput={(next) => update("leaf", next)} required value={fields.leaf} /></>}<Button disabled={feedback.busy} type="submit">Create {mode} plan</Button></form>}<form onSubmit={(event) => { event.preventDefault(); void simple("export"); }}><TextField id="template-export" label="Export target" onInput={(next) => update("exportPath", next)} required value={fields.exportPath} /><Button disabled={feedback.busy} type="submit" variant="tonal">Export</Button></form>{template.sourceKind === "builtin" ? null : <Button className="material-button--danger" disabled={feedback.busy} onClick={() => void simple("remove")} type="button" variant="text">Remove template</Button>}<TemplatePlanDialog busy={feedback.busy} plan={plan} title={`Review template ${mode}`} onApply={apply} onClose={() => setPlan(undefined)} /><MutationFeedback client={client} feedback={feedback} /></ActionSection>;
}

export function BackupCreatePanel({ client, onChanged, project }: ActionProps & { project: ProjectSnapshot }) {
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
    return <ActionSection title="Create backup"><Select id="backup-compression" label="Compression" onChange={(next) => setCompression(next as typeof compression)} options={[{ label: "Store", value: "store" }, { label: "Fast", value: "fast" }, { label: "Maximum", value: "maximum" }]} value={compression} /><Checkbox checked={exclude} label="Exclude VPM packages" onChange={setExclude} /><Button disabled={feedback.busy || project.revision === undefined} onClick={() => setConfirm(true)} type="button">Review backup</Button><ConfirmDialog busy={feedback.busy} open={confirm} title="Create this backup?" detail={`Compression: ${compression}. ${exclude ? "VPM packages will be excluded." : "VPM packages will be included."}`} onClose={() => setConfirm(false)} onConfirm={create} /><MutationFeedback client={client} feedback={feedback} /></ActionSection>;
}

export function BackupRestorePanel({ backup, client }: ActionProps & { backup: BackupRecord }) {
    const [parent, setParent] = useState("");
    const [leaf, setLeaf] = useState("");
    const [plan, setPlan] = useState<BackupRestorePlan>();
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const makePlan = async (event: FormEvent) => { event.preventDefault(); setFeedback({ busy: true }); try { setPlan(await client.backupPlanRestore(backup.backupId, parent, leaf)); setFeedback({ busy: false }); } catch (caught: unknown) { setFeedback({ busy: false, error: safeError(caught) }); } };
    const apply = async () => { if (plan === undefined) return; setFeedback({ busy: true }); try { const result = await client.backupApplyRestore(plan.planId); setPlan(undefined); setFeedback({ busy: false, operationId: result.operationId, message: "Restore operation accepted." }); } catch (caught: unknown) { setPlan(undefined); setFeedback({ busy: false, error: safeError(caught) }); } };
    return <ActionSection title="Restore backup"><form onSubmit={(event) => void makePlan(event)}><TextField id="restore-parent" label="Target parent" maxLength={1024} onInput={setParent} required value={parent} /><TextField id="restore-leaf" label="New directory name" maxLength={255} onInput={setLeaf} required value={leaf} /><Button disabled={feedback.busy} type="submit">Create restore plan</Button></form><PlanDialog busy={feedback.busy} open={plan !== undefined} title="Review backup restore" onClose={() => setPlan(undefined)} onApply={apply}>{plan === undefined ? null : <><p>Restore to <strong>{plan.target.leaf}</strong> in the selected parent directory.</p><p>The target must be absent: {plan.target.mustBeAbsent ? "yes" : "no"}</p><p>{plan.packagesRequireResolve ? "VPM packages require a separate resolve after restoration." : "No package resolve is required."}</p><p>Archive: <code>{shortValue(plan.archiveSha256)}</code></p></>}</PlanDialog><MutationFeedback client={client} feedback={feedback} /></ActionSection>;
}

export function OperationActions({ client, operation, onChanged }: ActionProps & { operation: Operation }) {
    const [confirm, setConfirm] = useState(false);
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const terminal = isTerminal(operation.state);
    const cancel = async () => { await runSimple(setFeedback, () => client.operationCancel(operation.operationId, operation.revision), "Cancellation requested.", onChanged); };
    return <ActionSection title="Operation control"><Button className="material-button--danger" disabled={terminal || feedback.busy} onClick={() => setConfirm(true)} type="button" variant="text">Cancel operation</Button><ConfirmDialog busy={feedback.busy} open={confirm} title="Request cancellation?" detail="Cancellation is cooperative. The operation remains authoritative until the daemon reports a terminal state." onClose={() => setConfirm(false)} onConfirm={cancel} /><MutationFeedback client={client} feedback={feedback} /></ActionSection>;
}

export function ExtensionInstallPanel({ client, onChanged }: ActionProps) {
    const [path, setPath] = useState("");
    const [approvePublisher, setApprovePublisher] = useState(false);
    const [expectedRevision, setExpectedRevision] = useState(0);
    const [plan, setPlan] = useState<ExtensionPlan>();
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const makePlan = async (event: FormEvent) => { event.preventDefault(); setFeedback({ busy: true }); try { const result = await client.extensionPlanInstall(path, expectedRevision, approvePublisher ? "approve_for_extension" : "none"); setPlan(result.plan); setFeedback({ busy: false }); } catch (caught: unknown) { setFeedback({ busy: false, error: safeError(caught) }); } };
    const apply = async () => { if (plan === undefined) return; setFeedback({ busy: true }); try { const result = await client.extensionApplyInstall(plan.planId); setPlan(undefined); setFeedback({ busy: false, operationId: result.operationId, message: "Extension install accepted." }); onChanged?.(); } catch (caught: unknown) { setPlan(undefined); setFeedback({ busy: false, error: safeError(caught) }); } };
    return <ActionSection title="Install extension"><form onSubmit={(event) => void makePlan(event)}><TextField id="extension-package" label="Extension package" maxLength={1024} onInput={setPath} required value={path} /><TextField id="extension-registry-revision" label="Expected registry revision" min={0} onInput={(next) => setExpectedRevision(Number(next))} required type="number" value={expectedRevision} /><Checkbox checked={approvePublisher} label="Approve this publisher for this extension" onChange={setApprovePublisher} /><Button disabled={feedback.busy} type="submit">Create install plan</Button></form><ExtensionPlanDialog busy={feedback.busy} plan={plan} title="Review extension install" onApply={apply} onClose={() => setPlan(undefined)} /><MutationFeedback client={client} feedback={feedback} /></ActionSection>;
}

export function ExtensionActions({ client, extension, onChanged }: ActionProps & { extension: ExtensionRecord }) {
    const [plan, setPlan] = useState<ExtensionPlan>();
    const [deleteData, setDeleteData] = useState(false);
    const [grant, setGrant] = useState({ permission: "", resourceKind: "Project", resourceId: "" });
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const lifecycle = async (enable: boolean) => runSimple(setFeedback, () => enable ? client.extensionEnable(extension.extensionId, extension.revision) : client.extensionDisable(extension.extensionId, extension.revision), `Extension ${enable ? "enabled" : "disabled"}.`, onChanged);
    const planUninstall = async () => { setFeedback({ busy: true }); try { const result = await client.extensionPlanUninstall(extension.extensionId, extension.revision, deleteData ? "delete_data" : "retain_data"); setPlan(result.plan); setFeedback({ busy: false }); } catch (caught: unknown) { setFeedback({ busy: false, error: safeError(caught) }); } };
    const applyUninstall = async () => { if (plan === undefined) return; setFeedback({ busy: true }); try { const result = await client.extensionApplyUninstall(plan.planId); setPlan(undefined); setFeedback({ busy: false, operationId: result.operationId, message: "Extension uninstall accepted." }); onChanged?.(); } catch (caught: unknown) { setPlan(undefined); setFeedback({ busy: false, error: safeError(caught) }); } };
    const changeGrant = async (revoke: boolean) => runSimple(setFeedback, () => revoke ? client.extensionRevokeGrant(extension.extensionId, grant.permission, grant.resourceKind, grant.resourceId, extension.grantRevision) : client.extensionSetGrant(extension.extensionId, grant.permission, grant.resourceKind, grant.resourceId, extension.grantRevision), `Permission ${revoke ? "revoked" : "granted"}.`, onChanged);
    const updateGrant = (key: keyof typeof grant, value: string) => setGrant((current) => ({ ...current, [key]: value }));
    return <ActionSection title="Extension management"><div className="action-row"><Button disabled={feedback.busy || extension.desiredState === "enabled"} onClick={() => void lifecycle(true)} type="button" variant="tonal">Enable</Button><Button disabled={feedback.busy || extension.desiredState !== "enabled"} onClick={() => void lifecycle(false)} type="button" variant="tonal">Disable</Button></div><fieldset><legend>Permission scope</legend><TextField id="grant-permission" label="Permission" onInput={(next) => updateGrant("permission", next)} required value={grant.permission} /><Select id="grant-kind" label="Resource kind" onChange={(next) => updateGrant("resourceKind", next)} options={[{ label: "Project", value: "Project" }, { label: "Extension", value: "Extension" }]} value={grant.resourceKind} /><TextField id="grant-resource" label="Resource ID" onInput={(next) => updateGrant("resourceId", next)} required value={grant.resourceId} /><div className="action-row"><Button disabled={feedback.busy || grant.permission.length === 0 || grant.resourceId.length === 0} onClick={() => void changeGrant(false)} type="button" variant="tonal">Grant</Button><Button className="material-button--danger" disabled={feedback.busy || grant.permission.length === 0 || grant.resourceId.length === 0} onClick={() => void changeGrant(true)} type="button" variant="text">Revoke</Button></div></fieldset><Checkbox checked={deleteData} label="Delete extension-owned data during uninstall" onChange={setDeleteData} /><Button className="material-button--danger" disabled={feedback.busy} onClick={() => void planUninstall()} type="button" variant="text">Create uninstall plan</Button><ExtensionPlanDialog busy={feedback.busy} plan={plan} title="Review extension uninstall" onApply={applyUninstall} onClose={() => setPlan(undefined)} /><MutationFeedback client={client} feedback={feedback} /></ActionSection>;
}

function ActionSection({ children, title }: { children: ReactNode; title: string }) {
    return <section className="action-section"><h2>{title}</h2>{children}</section>;
}

function ConfirmDialog({ busy, detail, onClose, onConfirm, open, title }: { busy: boolean; detail: string; onClose(): void; onConfirm(): Promise<void>; open: boolean; title: string }) {
    return <ModalDialog open={open} title={title} onClose={onClose}><p>{detail}</p><div className="dialog-actions"><Button disabled={busy} onClick={onClose} type="button" variant="tonal">Go back</Button><Button data-dialog-initial-focus disabled={busy} onClick={() => void onConfirm().finally(onClose)} type="button">{busy ? "Working…" : "Confirm"}</Button></div></ModalDialog>;
}

function PlanDialog({ busy, children, onApply, onClose, open, title }: { busy: boolean; children: ReactNode; onApply(): Promise<void>; onClose(): void; open: boolean; title: string }) {
    return <ModalDialog open={open} title={title} onClose={onClose}>{children}<p className="risk-summary">The daemon will revalidate this frozen plan. A stale plan fails instead of being silently replaced.</p><div className="dialog-actions"><Button disabled={busy} onClick={onClose} type="button" variant="tonal">Discard plan</Button><Button data-dialog-initial-focus disabled={busy} onClick={() => void onApply()} type="button">{busy ? "Applying…" : "Apply reviewed plan"}</Button></div></ModalDialog>;
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

function MutationFeedback({ client, feedback }: { client: GuiRpcClient; feedback: FeedbackState }) {
    if (feedback.error !== undefined) return <div className="mutation-feedback mutation-feedback--error" role="alert"><strong>Request failed</strong><span><code>{feedback.error.code}</code>{feedback.error.diagnosticId === undefined ? "" : ` · Diagnostic ID ${feedback.error.diagnosticId}`}</span></div>;
    if (feedback.operationId !== undefined) return <OperationFollow client={client} operationId={feedback.operationId} />;
    if (feedback.message !== undefined) return <div className="mutation-feedback" role="status" aria-live="polite">{feedback.message}</div>;
    return null;
}

export function OperationFollow({ client, operationId, title = "Operation" }: { client: GuiRpcClient; operationId: string; title?: string }) {
    const [operation, setOperation] = useState<Operation>();
    const [error, setError] = useState<RpcError>();
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
    return "ALCOMD could not complete the package changes. Try again or open Logs for more information.";
}
function shortValue(value: string): string { return value.length <= 28 ? value : `${value.slice(0, 20)}…${value.slice(-8)}`; }
function isTerminal(state: string): boolean { return ["succeeded", "failed", "cancelled"].includes(state); }
