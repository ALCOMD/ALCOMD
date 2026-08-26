import type { ExtensionRecord, RpcError } from "@alcomd/sdk";
import { useCallback, useEffect, useId, useRef, useState, type FormEvent, type ReactNode } from "react";

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
import { Button, Dialog as MaterialDialog, Select, TextField } from "./Material";

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
                <Field label="Project root" hint="The daemon validates and owns this path." id="project-root">
                    <input aria-describedby="project-root-hint" id="project-root" maxLength={1024} onChange={(event) => setPath(event.currentTarget.value)} required value={path} />
                </Field>
                <button className="button button--filled" disabled={feedback.busy || path.length === 0} type="submit">Review registration</button>
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
                <button className="button button--tonal" disabled={feedback.busy || revision === undefined} onClick={() => void refresh()} type="button">Refresh</button>
                <button className="button button--danger" disabled={feedback.busy || revision === undefined} onClick={() => setConfirmUnregister(true)} type="button">Unregister</button>
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
                <Field label="Source type" id="repository-kind"><select id="repository-kind" value={kind} onChange={(event) => setKind(event.currentTarget.value as "remote" | "local")}><option value="remote">Remote URL</option><option value="local">Local manifest</option></select></Field>
                <Field label={kind === "remote" ? "Repository URL" : "Local manifest path"} id="repository-source"><input id="repository-source" maxLength={2048} onChange={(event) => setValue(event.currentTarget.value)} required value={value} /></Field>
                <button className="button button--filled" disabled={feedback.busy || value.length === 0} type="submit">Review repository</button>
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
            <div className="action-row"><button className="button button--tonal" disabled={!ready || feedback.busy} onClick={() => void refresh()} type="button">Refresh</button><button className="button button--danger" disabled={!ready || feedback.busy} onClick={() => setConfirmRemove(true)} type="button">Remove</button></div>
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
            <form onSubmit={(event) => void register(event)}><Field label="Unity executable" id="unity-executable"><input id="unity-executable" maxLength={1024} onChange={(event) => setPath(event.currentTarget.value)} required value={path} /></Field><div className="action-row"><button className="button button--filled" disabled={feedback.busy} type="submit">Register</button><button className="button button--tonal" disabled={feedback.busy} onClick={() => void runSimple(setFeedback, () => client.unityInstallationsRefresh(), "Unity registry refreshed.", onChanged)} type="button">Discover and refresh</button></div></form>
            {installations.length === 0 ? null : <label>Remove installation<select aria-label="Installation to remove" defaultValue="" onChange={(event) => setRemove(installations.find((item) => item.installationId === event.currentTarget.value))}><option value="">Select an installation</option>{installations.map((item) => <option key={item.installationId} value={item.installationId}>Unity {item.unityVersion}</option>)}</select></label>}
            <button className="button button--danger" disabled={remove === undefined || feedback.busy} onClick={() => setConfirmRemove(true)} type="button">Review removal</button>
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
            <form onSubmit={(event) => void setEditor(event)}><Field label="Selected editor" id="project-editor"><select id="project-editor" required value={installationId} onChange={(event) => setInstallationId(event.currentTarget.value)}><option value="">Choose an editor</option>{installations.map((item) => <option key={item.installationId} value={item.installationId}>Unity {item.unityVersion} ({item.architecture})</option>)}</select></Field><Field label="Additional arguments" hint="One argument per line. The daemon validates forbidden arguments." id="unity-arguments"><textarea aria-describedby="unity-arguments-hint" id="unity-arguments" maxLength={4096} onChange={(event) => setArgumentsText(event.currentTarget.value)} value={argumentsText} /></Field><button className="button button--tonal" disabled={feedback.busy || projectId === undefined} type="submit">Save editor preference</button></form>
            <button className="button button--filled" disabled={feedback.busy || projectRevision === undefined} onClick={() => setConfirmLaunch(true)} type="button">Launch Unity</button>
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
    return <ActionSection title="Import template"><form onSubmit={(event) => void create(event)}><Field label="Template bundle" id="template-bundle"><input id="template-bundle" maxLength={1024} required value={bundlePath} onChange={(event) => setBundlePath(event.currentTarget.value)} /></Field><Field label="Expected registry revision" id="template-registry-revision"><input id="template-registry-revision" min={0} required type="number" value={expectedRevision} onChange={(event) => setExpectedRevision(event.currentTarget.valueAsNumber)} /></Field><label className="checkbox-field"><input checked={overrideExisting} onChange={(event) => setOverrideExisting(event.currentTarget.checked)} type="checkbox" />Replace an existing matching template</label><button className="button button--filled" disabled={feedback.busy} type="submit">Create import plan</button></form><TemplatePlanDialog busy={feedback.busy} plan={plan} title="Review template import" onApply={apply} onClose={() => setPlan(undefined)} /><MutationFeedback client={client} feedback={feedback} /></ActionSection>;
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
    return <ActionSection title="Template workflows"><div className="action-row"><button className="button button--tonal" onClick={() => setMode("derive")} type="button">Derive from project</button><button className="button button--tonal" onClick={() => setMode("create")} type="button">Create project</button><button className="button button--tonal" onClick={() => void simple("favorite")} type="button">{template.favorite ? "Remove favorite" : "Favorite"}</button></div>{mode === "none" ? null : <form onSubmit={(event) => void createPlan(event)}>{mode === "derive" ? <><Field label="Source project ID" id="derive-project"><input id="derive-project" required value={fields.projectId} onChange={(event) => update("projectId", event.currentTarget.value)} /></Field><Field label="Expected project revision" id="derive-project-revision"><input id="derive-project-revision" min={0} required type="number" value={fields.projectRevision} onChange={(event) => update("projectRevision", event.currentTarget.valueAsNumber)} /></Field><Field label="New template ID" id="derive-template-id"><input id="derive-template-id" required value={fields.templateId} onChange={(event) => update("templateId", event.currentTarget.value)} /></Field><Field label="Template version" id="derive-version"><input id="derive-version" required value={fields.templateVersion} onChange={(event) => update("templateVersion", event.currentTarget.value)} /></Field><Field label="Display name" id="derive-name"><input id="derive-name" required value={fields.displayName} onChange={(event) => update("displayName", event.currentTarget.value)} /></Field></> : <><Field label="Target parent" id="create-parent"><input id="create-parent" required value={fields.parent} onChange={(event) => update("parent", event.currentTarget.value)} /></Field><Field label="Target directory name" id="create-leaf"><input id="create-leaf" required value={fields.leaf} onChange={(event) => update("leaf", event.currentTarget.value)} /></Field></>}<button className="button button--filled" disabled={feedback.busy} type="submit">Create {mode} plan</button></form>}<form onSubmit={(event) => { event.preventDefault(); void simple("export"); }}><Field label="Export target" id="template-export"><input id="template-export" required value={fields.exportPath} onChange={(event) => update("exportPath", event.currentTarget.value)} /></Field><button className="button button--tonal" disabled={feedback.busy} type="submit">Export</button></form>{template.sourceKind === "builtin" ? null : <button className="button button--danger" disabled={feedback.busy} onClick={() => void simple("remove")} type="button">Remove template</button>}<TemplatePlanDialog busy={feedback.busy} plan={plan} title={`Review template ${mode}`} onApply={apply} onClose={() => setPlan(undefined)} /><MutationFeedback client={client} feedback={feedback} /></ActionSection>;
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
    return <ActionSection title="Create backup"><Field label="Compression" id="backup-compression"><select id="backup-compression" value={compression} onChange={(event) => setCompression(event.currentTarget.value as typeof compression)}><option value="store">Store</option><option value="fast">Fast</option><option value="maximum">Maximum</option></select></Field><label className="checkbox-field"><input checked={exclude} onChange={(event) => setExclude(event.currentTarget.checked)} type="checkbox" />Exclude VPM packages</label><button className="button button--filled" disabled={feedback.busy || project.revision === undefined} onClick={() => setConfirm(true)} type="button">Review backup</button><ConfirmDialog busy={feedback.busy} open={confirm} title="Create this backup?" detail={`Compression: ${compression}. ${exclude ? "VPM packages will be excluded." : "VPM packages will be included."}`} onClose={() => setConfirm(false)} onConfirm={create} /><MutationFeedback client={client} feedback={feedback} /></ActionSection>;
}

export function BackupRestorePanel({ backup, client }: ActionProps & { backup: BackupRecord }) {
    const [parent, setParent] = useState("");
    const [leaf, setLeaf] = useState("");
    const [plan, setPlan] = useState<BackupRestorePlan>();
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const makePlan = async (event: FormEvent) => { event.preventDefault(); setFeedback({ busy: true }); try { setPlan(await client.backupPlanRestore(backup.backupId, parent, leaf)); setFeedback({ busy: false }); } catch (caught: unknown) { setFeedback({ busy: false, error: safeError(caught) }); } };
    const apply = async () => { if (plan === undefined) return; setFeedback({ busy: true }); try { const result = await client.backupApplyRestore(plan.planId); setPlan(undefined); setFeedback({ busy: false, operationId: result.operationId, message: "Restore operation accepted." }); } catch (caught: unknown) { setPlan(undefined); setFeedback({ busy: false, error: safeError(caught) }); } };
    return <ActionSection title="Restore backup"><form onSubmit={(event) => void makePlan(event)}><Field label="Target parent" id="restore-parent"><input id="restore-parent" maxLength={1024} required value={parent} onChange={(event) => setParent(event.currentTarget.value)} /></Field><Field label="New directory name" id="restore-leaf"><input id="restore-leaf" maxLength={255} required value={leaf} onChange={(event) => setLeaf(event.currentTarget.value)} /></Field><button className="button button--filled" disabled={feedback.busy} type="submit">Create restore plan</button></form><PlanDialog busy={feedback.busy} open={plan !== undefined} title="Review backup restore" onClose={() => setPlan(undefined)} onApply={apply}>{plan === undefined ? null : <><p>Restore to <strong>{plan.target.leaf}</strong> in the selected parent directory.</p><p>The target must be absent: {plan.target.mustBeAbsent ? "yes" : "no"}</p><p>{plan.packagesRequireResolve ? "VPM packages require a separate resolve after restoration." : "No package resolve is required."}</p><p>Archive: <code>{shortValue(plan.archiveSha256)}</code></p></>}</PlanDialog><MutationFeedback client={client} feedback={feedback} /></ActionSection>;
}

export function OperationActions({ client, operation, onChanged }: ActionProps & { operation: Operation }) {
    const [confirm, setConfirm] = useState(false);
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const terminal = isTerminal(operation.state);
    const cancel = async () => { await runSimple(setFeedback, () => client.operationCancel(operation.operationId, operation.revision), "Cancellation requested.", onChanged); };
    return <ActionSection title="Operation control"><button className="button button--danger" disabled={terminal || feedback.busy} onClick={() => setConfirm(true)} type="button">Cancel operation</button><ConfirmDialog busy={feedback.busy} open={confirm} title="Request cancellation?" detail="Cancellation is cooperative. The operation remains authoritative until the daemon reports a terminal state." onClose={() => setConfirm(false)} onConfirm={cancel} /><MutationFeedback client={client} feedback={feedback} /></ActionSection>;
}

export function ExtensionInstallPanel({ client, onChanged }: ActionProps) {
    const [path, setPath] = useState("");
    const [approvePublisher, setApprovePublisher] = useState(false);
    const [expectedRevision, setExpectedRevision] = useState(0);
    const [plan, setPlan] = useState<ExtensionPlan>();
    const [feedback, setFeedback] = useState(INITIAL_FEEDBACK);
    const makePlan = async (event: FormEvent) => { event.preventDefault(); setFeedback({ busy: true }); try { const result = await client.extensionPlanInstall(path, expectedRevision, approvePublisher ? "approve_for_extension" : "none"); setPlan(result.plan); setFeedback({ busy: false }); } catch (caught: unknown) { setFeedback({ busy: false, error: safeError(caught) }); } };
    const apply = async () => { if (plan === undefined) return; setFeedback({ busy: true }); try { const result = await client.extensionApplyInstall(plan.planId); setPlan(undefined); setFeedback({ busy: false, operationId: result.operationId, message: "Extension install accepted." }); onChanged?.(); } catch (caught: unknown) { setPlan(undefined); setFeedback({ busy: false, error: safeError(caught) }); } };
    return <ActionSection title="Install extension"><form onSubmit={(event) => void makePlan(event)}><Field label="Extension package" id="extension-package"><input id="extension-package" maxLength={1024} required value={path} onChange={(event) => setPath(event.currentTarget.value)} /></Field><Field label="Expected registry revision" id="extension-registry-revision"><input id="extension-registry-revision" min={0} required type="number" value={expectedRevision} onChange={(event) => setExpectedRevision(event.currentTarget.valueAsNumber)} /></Field><label className="checkbox-field"><input checked={approvePublisher} onChange={(event) => setApprovePublisher(event.currentTarget.checked)} type="checkbox" />Approve this publisher for this extension</label><button className="button button--filled" disabled={feedback.busy} type="submit">Create install plan</button></form><ExtensionPlanDialog busy={feedback.busy} plan={plan} title="Review extension install" onApply={apply} onClose={() => setPlan(undefined)} /><MutationFeedback client={client} feedback={feedback} /></ActionSection>;
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
    return <ActionSection title="Extension management"><div className="action-row"><button className="button button--tonal" disabled={feedback.busy || extension.desiredState === "enabled"} onClick={() => void lifecycle(true)} type="button">Enable</button><button className="button button--tonal" disabled={feedback.busy || extension.desiredState !== "enabled"} onClick={() => void lifecycle(false)} type="button">Disable</button></div><fieldset><legend>Permission scope</legend><Field label="Permission" id="grant-permission"><input id="grant-permission" required value={grant.permission} onChange={(event) => updateGrant("permission", event.currentTarget.value)} /></Field><Field label="Resource kind" id="grant-kind"><select id="grant-kind" value={grant.resourceKind} onChange={(event) => updateGrant("resourceKind", event.currentTarget.value)}><option value="Project">Project</option><option value="Extension">Extension</option></select></Field><Field label="Resource ID" id="grant-resource"><input id="grant-resource" required value={grant.resourceId} onChange={(event) => updateGrant("resourceId", event.currentTarget.value)} /></Field><div className="action-row"><button className="button button--tonal" disabled={feedback.busy || grant.permission.length === 0 || grant.resourceId.length === 0} onClick={() => void changeGrant(false)} type="button">Grant</button><button className="button button--danger" disabled={feedback.busy || grant.permission.length === 0 || grant.resourceId.length === 0} onClick={() => void changeGrant(true)} type="button">Revoke</button></div></fieldset><label className="checkbox-field"><input checked={deleteData} onChange={(event) => setDeleteData(event.currentTarget.checked)} type="checkbox" />Delete extension-owned data during uninstall</label><button className="button button--danger" disabled={feedback.busy} onClick={() => void planUninstall()} type="button">Create uninstall plan</button><ExtensionPlanDialog busy={feedback.busy} plan={plan} title="Review extension uninstall" onApply={applyUninstall} onClose={() => setPlan(undefined)} /><MutationFeedback client={client} feedback={feedback} /></ActionSection>;
}

function ActionSection({ children, title }: { children: ReactNode; title: string }) {
    return <section className="action-section"><h2>{title}</h2>{children}</section>;
}

function Field({ children, hint, id, label }: { children: ReactNode; hint?: string; id: string; label: string }) {
    return <label className="form-field" htmlFor={id}><span>{label}</span>{children}{hint === undefined ? null : <span className="field-hint" id={`${id}-hint`}>{hint}</span>}</label>;
}

function ConfirmDialog({ busy, detail, onClose, onConfirm, open, title }: { busy: boolean; detail: string; onClose(): void; onConfirm(): Promise<void>; open: boolean; title: string }) {
    return <ModalDialog open={open} title={title} onClose={onClose}><p>{detail}</p><div className="dialog-actions"><button className="button button--tonal" disabled={busy} onClick={onClose} type="button">Go back</button><button className="button button--filled" data-dialog-initial-focus disabled={busy} onClick={() => void onConfirm().finally(onClose)} type="button">{busy ? "Working…" : "Confirm"}</button></div></ModalDialog>;
}

function PlanDialog({ busy, children, onApply, onClose, open, title }: { busy: boolean; children: ReactNode; onApply(): Promise<void>; onClose(): void; open: boolean; title: string }) {
    return <ModalDialog open={open} title={title} onClose={onClose}>{children}<p className="risk-summary">The daemon will revalidate this frozen plan. A stale plan fails instead of being silently replaced.</p><div className="dialog-actions"><button className="button button--tonal" disabled={busy} onClick={onClose} type="button">Discard plan</button><button className="button button--filled" data-dialog-initial-focus disabled={busy} onClick={() => void onApply()} type="button">{busy ? "Applying…" : "Apply reviewed plan"}</button></div></ModalDialog>;
}

function TemplatePlanDialog(props: { busy: boolean; onApply(): Promise<void>; onClose(): void; plan?: TemplatePlan; title: string }) {
    return <PlanDialog busy={props.busy} open={props.plan !== undefined} title={props.title} onApply={props.onApply} onClose={props.onClose}>{props.plan === undefined ? null : <><p>Action: <strong>{humanize(props.plan.action)}</strong></p><p>Plan fingerprint: <code>{shortValue(props.plan.planFingerprint)}</code></p></>}</PlanDialog>;
}

function ExtensionPlanDialog(props: { busy: boolean; onApply(): Promise<void>; onClose(): void; plan?: ExtensionPlan; title: string }) {
    return <PlanDialog busy={props.busy} open={props.plan !== undefined} title={props.title} onApply={props.onApply} onClose={props.onClose}>{props.plan === undefined ? null : <dl className="dialog-summary"><div><dt>Extension</dt><dd>{props.plan.extensionId}</dd></div><div><dt>Version</dt><dd>{props.plan.version}</dd></div><div><dt>Publisher</dt><dd><code>{shortValue(props.plan.publisherFingerprint)}</code></dd></div><div><dt>Trust</dt><dd>{humanize(props.plan.trustDecision)}</dd></div><div><dt>Data</dt><dd>{humanize(props.plan.dataDisposition)}</dd></div></dl>}</PlanDialog>;
}

function ModalDialog({ children, onClose, open, title }: { children: ReactNode; onClose(): void; open: boolean; title: string }) {
    const ref = useRef<HTMLDialogElement>(null);
    const openerRef = useRef<HTMLElement | null>(null);
    const titleId = useId();
    useEffect(() => {
        const dialog = ref.current;
        if (dialog === null) return;
        if (open && !dialog.open) {
            openerRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
            dialog.showModal();
            dialog.querySelector<HTMLElement>("[data-dialog-initial-focus]")?.focus();
        }
        if (!open && dialog.open) {
            dialog.close();
            const opener = openerRef.current;
            openerRef.current = null;
            window.requestAnimationFrame(() => opener?.focus());
        }
    }, [open]);
    const keepFocusInside = (event: React.KeyboardEvent<HTMLDialogElement>) => {
        if (event.key !== "Tab") return;
        const focusable = Array.from(event.currentTarget.querySelectorAll<HTMLElement>("button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), a[href], [tabindex]:not([tabindex='-1'])"));
        if (focusable.length === 0) {
            event.preventDefault();
            return;
        }
        const first = focusable[0];
        const last = focusable[focusable.length - 1];
        if (event.shiftKey && document.activeElement === first) {
            event.preventDefault();
            last?.focus();
        } else if (!event.shiftKey && document.activeElement === last) {
            event.preventDefault();
            first?.focus();
        }
    };
    return <dialog aria-labelledby={titleId} className="modal-dialog" onCancel={(event) => { event.preventDefault(); onClose(); }} onClose={onClose} onKeyDown={keepFocusInside} ref={ref}><h2 id={titleId}>{title}</h2>{children}</dialog>;
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
    return <section className="operation-follow" aria-live="polite" aria-atomic="true" role="status"><h3>{title}</h3>{operation === undefined ? <p>Reading progress…</p> : <><p><strong>{humanize(operation.state)}</strong>{operation.progress?.phase === undefined ? "" : ` · ${humanize(operation.progress.phase)}`}</p><progress aria-label={`${title} progress`} max={1} value={isTerminal(operation.state) ? 1 : undefined} /></>}{error === undefined ? null : <p className="inline-error">Progress is temporarily unavailable. ALCOMD will keep following the same task.</p>}</section>;
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
    return "ALCOMD could not complete the package changes. Try again or open Log for more information.";
}
function shortValue(value: string): string { return value.length <= 28 ? value : `${value.slice(0, 20)}…${value.slice(-8)}`; }
function isTerminal(state: string): boolean { return ["succeeded", "failed", "cancelled"].includes(state); }
