import type { CandidateCursor, PackageCandidateEvidence, PackageCandidateSummary, RpcError } from "@alcomd/sdk";
import "./LogsWorkspace.css";
import { actionableVersion, candidateSource, candidateSourceKey, choiceText, intentForSummary, queryCandidates, readCandidateSummary, uncertainUpdate, updateReason, validateVersionPage, type CandidateSummarySet } from "./package-candidates";
import {
    accountCircleIcon,
    arrowBackIcon,
    arrowDownwardIcon,
    arrowUpwardIcon,
    backupIcon,
    downloadIcon,
    helpIcon,
    moreVertIcon,
    playArrowIcon,
    publicIcon,
    refreshIcon,
    starIcon,
    upgradeIcon,
    viewGridIcon,
    viewListIcon
} from "@alcomd/ui/icons";
import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";

import type {
    OfficialSettings,
    Operation,
    PackageSourceSelector,
    ProjectCopyPlan,
    ProjectDeletePlan,
    ProjectSnapshot,
    RegistryCursor,
    RepositoryPackageVersion,
    RepositorySnapshot,
    UnityInstallation,
    UnityLaunchOptionsResult,
    UserPackageRecord,
    SettingsGetResult
} from "./core-models";
import {
    BackupCreatePanel,
    BackupRestorePanel,
    ExtensionActions,
    ExtensionInstallPanel,
    OperationActions,
    OperationFollow,
    PackageActions,
    type PackageActionSelection,
    ProjectUnityActions,
    ProjectUnityWorkspaceActions,
    RegisterRepositoryPanel,
    RepositoryActions,
    TemplateActions,
    TemplateImportPanel,
    UnityRegistryActions
} from "./CoreActions";
import { DataTableHeader, MaterialDataTable } from "./DataTable";
import { UtilityActionDialog } from "./UtilityActionDialog";
import { UtilityWorkspace } from "./UtilityWorkspace";
import "./ExtensionWorkspace.css";
import { SettingsWorkspace } from "./SettingsWorkspace";
import { PagedItems } from "./PagedItems";
import type { GuiRpcClient } from "./rpc";
import { Button, Checkbox, Dialog, FilterPopover, Icon, IconButton, Menu, MenuItem, SearchField, Select, Switch, TextField } from "./Material";
import { CreateProjectDialog, RestoreProjectDialog } from "./ProjectCreationDialogs";
import { capabilities, capabilityUnavailableTitle, useCapability, useCapabilityState, useReconnect } from "./capabilities";

interface PageProps {
    client: GuiRpcClient;
    navigate(path: string): void;
}

interface ResourcePageProps<T> {
    load(): Promise<T>;
    children(value: T, refresh: () => void, refreshing: boolean, error?: RpcError): ReactNode;
    empty?(value: T): boolean;
    emptyTitle?: string;
    showRefreshBar?: boolean;
}

function ResourcePage<T>({ load, children, empty, emptyTitle = "Nothing here yet", showRefreshBar = true }: ResourcePageProps<T>) {
    const [state, setState] = useState<{
        value?: T;
        error?: RpcError;
        loading: boolean;
        refreshing: boolean;
        generation: number;
    }>({ loading: true, refreshing: false, generation: 0 });

    useEffect(() => {
        let active = true;
        setState((current) => ({
            ...current,
            error: undefined,
            loading: current.value === undefined,
            refreshing: current.value !== undefined
        }));
        void load().then((value) => {
            if (active) {
                setState((current) => ({ ...current, value, error: undefined, loading: false, refreshing: false }));
            }
        }).catch((caught: unknown) => {
            if (active) {
                setState((current) => ({
                    ...current,
                    error: safeError(caught),
                    loading: false,
                    refreshing: false
                }));
            }
        });
        return () => { active = false; };
    }, [load, state.generation]);

    const refresh = () => setState((current) => ({ ...current, generation: current.generation + 1 }));
    if (state.loading) {
        return <RouteState kind="loading" title="Loading" detail="Reading the current ALCOMD state…" />;
    }
    if (state.error !== undefined && state.value === undefined) {
        return <ErrorState error={state.error} retry={refresh} />;
    }
    if (state.value === undefined) {
        return <RouteState kind="error" title="No response" />;
    }
    if (empty?.(state.value) === true) {
        return (
            <>
                {showRefreshBar ? <RefreshBar error={state.error} refresh={refresh} refreshing={state.refreshing} /> : null}
                <RouteState kind="empty" title={emptyTitle} />
            </>
        );
    }
    return (
        <>
            {showRefreshBar ? <RefreshBar error={state.error} refresh={refresh} refreshing={state.refreshing} /> : null}
            {children(state.value, refresh, state.refreshing, state.error)}
        </>
    );
}

function RefreshBar({ error, refresh, refreshing }: { error?: RpcError; refresh(): void; refreshing: boolean }) {
    return (
        <div className="refresh-bar" role="status" aria-live="polite">
            <span>{refreshing ? "Refreshing while keeping the last result…" : "Current daemon state"}</span>
            {error === undefined ? null : <span className="inline-error">Refresh failed: {error.code}</span>}
            <Button widthLabels={["Refreshing…", "Refresh"]} disabled={refreshing} onClick={refresh} type="button" variant="tonal">
                {refreshing ? "Refreshing…" : "Refresh"}
            </Button>
        </div>
    );
}

export function RouteState({ kind, title, detail }: { kind: "loading" | "empty" | "error" | "disconnected"; title: string; detail?: string }) {
    return (
        <section className={`route-state route-state--${kind}`} role={kind === "error" ? "alert" : "status"} aria-live="polite">
            <h2>{title}</h2>
            {detail === undefined ? null : <p>{detail}</p>}
        </section>
    );
}

function ErrorState({ error, retry }: { error: RpcError; retry(): void }) {
    const reconnect = useReconnect();
    const disconnected = error.code === "daemon_unavailable";
    return (
        <section className={`route-state route-state--${disconnected ? "disconnected" : "error"}`} role="alert">
            <h2>{disconnected ? "ALCOMD core disconnected" : "Request failed"}</h2>
            <p><code>{error.code}</code></p>
            {error.diagnosticId === undefined ? null : <p>Diagnostic ID: <code>{error.diagnosticId}</code></p>}
            <Button onClick={() => { if (disconnected) reconnect?.(); retry(); }} type="button">Reconnect and retry</Button>
        </section>
    );
}

export function HomePage({ client, navigate }: PageProps) {
    const load = useCallback(() => client.systemStatus(), [client]);
    return (
        <Page title="Home" eyebrow="Official desktop client">
            <ResourcePage load={load}>{(status) => (
                <div className="dashboard-grid">
                    <article className="summary-card"><h2>ALCOMD core</h2><p className="status-value">{status.state}</p><p>Daemon {status.daemonVersion}</p></article>
                    <article className="summary-card"><h2>Protocol</h2><p className="status-value">RPC v{status.rpcVersion}</p><p>{status.capabilities.length} capabilities negotiated</p></article>
                    <article className="summary-card"><h2>Get started</h2><Button onClick={() => navigate("/projects")} type="button" variant="text">Open projects</Button></article>
                </div>
            )}</ResourcePage>
        </Page>
    );
}

export function ProjectsPage({ client, navigate }: PageProps) {
    const canRegister = useCapability(capabilities.projectsRegistry);
    const canCreate = useCapability(capabilities.templatesCreateProject);
    const canRestore = useCapability(capabilities.backupsRestore);
    const [state, setState] = useState<{
        error?: RpcError;
        loading: boolean;
        projects: ProjectSnapshot[];
        refreshing: boolean;
    }>({ loading: true, projects: [], refreshing: false });
    const [search, setSearch] = useState("");
    const [sort, setSort] = useState("observed");
    const [descending, setDescending] = useState(true);
    const [view, setView] = useState<"list" | "grid">("list");
    const [registerOpen, setRegisterOpen] = useState(false);
    const [createOpen, setCreateOpen] = useState(false);
    const [restoreOpen, setRestoreOpen] = useState(false);
    const [registerPath, setRegisterPath] = useState("");
    const [selectingDirectory, setSelectingDirectory] = useState(false);
    const [registrationMessage, setRegistrationMessage] = useState<string>();
    const registerButtonRef = useRef<HTMLElement>(null);
    const createButtonRef = useRef<HTMLElement>(null);
    const restoreButtonRef = useRef<HTMLElement>(null);

    const refresh = useCallback(async () => {
        setState((current) => ({ ...current, error: undefined, refreshing: current.projects.length > 0 }));
        try {
            const projects: ProjectSnapshot[] = [];
            let cursor: RegistryCursor | undefined;
            do {
                const value = await client.projectsList(cursor);
                projects.push(...value.projects);
                cursor = value.nextCursor;
            } while (cursor !== undefined);
            setState({ loading: false, projects, refreshing: false });
        } catch (caught: unknown) {
            setState((current) => ({ ...current, error: safeError(caught), loading: false, refreshing: false }));
        }
    }, [client]);

    useEffect(() => {
        void refresh();
    }, [refresh]);

    const projects = [...state.projects]
        .filter((project) => projectName(project).toLocaleLowerCase().includes(search.toLocaleLowerCase()))
        .sort((left, right) => {
            const favorite = Number(right.favorite === true) - Number(left.favorite === true);
            if (favorite !== 0) return favorite;
            let compared = 0;
            if (sort === "name") compared = projectName(left).localeCompare(projectName(right));
            if (sort === "type") compared = left.projectType.localeCompare(right.projectType);
            if (sort === "unity") compared = left.unityVersion.localeCompare(right.unityVersion);
            if (sort === "added") compared = (left.registeredAtMs ?? 0) - (right.registeredAtMs ?? 0);
            if (sort === "observed") compared = left.observedAtMs - right.observedAtMs;
            return descending ? -compared : compared;
        });

    const updateProject = (project: ProjectSnapshot) => {
        setState((current) => ({
            ...current,
            projects: current.projects.map((existing) => existing.projectId === project.projectId ? project : existing)
        }));
    };

    const updateSort = (nextSort: string) => {
        if (sort === nextSort) {
            setDescending((current) => !current);
            return;
        }
        setSort(nextSort);
        setDescending(nextSort === "added" || nextSort === "observed");
    };

    const chooseProjectDirectory = async () => {
        setSelectingDirectory(true);
        setRegistrationMessage(undefined);
        try {
            const selected = await client.selectDirectory();
            if (selected === undefined) return;
            setRegisterPath(selected);
            setRegisterOpen(true);
        } catch (caught: unknown) {
            setRegistrationMessage(`Unable to select directory: ${safeError(caught).code}`);
        } finally {
            setSelectingDirectory(false);
        }
    };

    const closeRegisterDialog = () => {
        setRegisterOpen(false);
        window.requestAnimationFrame(() => registerButtonRef.current?.focus());
    };

    return (
        <section className="projects-page">
            <header className="projects-toolbar">
                <h1 id="route-title" tabIndex={-1}>Projects</h1>
                <IconButton disabled={state.refreshing} label={state.refreshing ? "Refreshing projects" : "Refresh projects"} onClick={() => void refresh()} type="button">
                    <Icon asset={refreshIcon} />
                </IconButton>
                <SearchField className="projects-search" label="Search projects" onInput={setSearch} placeholder="Search..." value={search} />
                <Button onClick={() => setView((current) => current === "list" ? "grid" : "list")} type="button" variant="text">
                    <Icon asset={view === "list" ? viewGridIcon : viewListIcon} slot="icon" />
                    <StateSizedLabel current={view === "list" ? "Grid view" : "List view"} labels={["Grid view", "List view"]} />
                </Button>
                <Button disabled={!canRestore} onClick={() => setRestoreOpen(true)} ref={restoreButtonRef} title={capabilityUnavailableTitle(canRestore, capabilities.backupsRestore)} type="button" variant="tonal">Restore project</Button>
                <Button disabled={!canRegister || selectingDirectory} onClick={() => void chooseProjectDirectory()} ref={registerButtonRef} title={capabilityUnavailableTitle(canRegister, capabilities.projectsRegistry)} type="button">
                    <StateSizedLabel current={selectingDirectory ? "Choosing…" : "Register project"} labels={["Register project", "Choosing…"]} />
                </Button>
                <Button disabled={!canCreate} onClick={() => setCreateOpen(true)} ref={createButtonRef} title={capabilityUnavailableTitle(canCreate, capabilities.templatesCreateProject)} type="button">Create project</Button>
            </header>
            {view === "grid" ? (
                <div className="projects-secondary-toolbar">
                    <span className="projects-sort-label">Sort by:</span>
                    <Select
                        aria-label="Sort by"
                        className="projects-sort"
                        label=""
                        onChange={updateSort}
                        options={[
                            { label: "Last observed", value: "observed" },
                            { label: "Name", value: "name" },
                            { label: "Project type", value: "type" },
                            { label: "Unity version", value: "unity" },
                            { label: "Added", value: "added" }
                        ]}
                        value={sort}
                        variant="filled"
                    />
                    <IconButton label={descending ? "Sort descending" : "Sort ascending"} onClick={() => setDescending((current) => !current)} type="button">
                        <Icon asset={descending ? arrowDownwardIcon : arrowUpwardIcon} />
                    </IconButton>
                    <span className="projects-result-count" role="status" aria-live="polite">
                        {projects.length} {projects.length === 1 ? "project" : "projects"}
                    </span>
                </div>
            ) : null}
            <div className="projects-content">
                {state.loading ? <RouteState kind="loading" title="Loading projects" /> : null}
                {state.error !== undefined && state.projects.length === 0 ? <ErrorState error={state.error} retry={() => void refresh()} /> : null}
                {!state.loading && state.error === undefined && projects.length === 0 ? (
                    <section className="projects-empty" role="status">
                        <h2>{search.length === 0 ? "No registered projects" : "No matching projects"}</h2>
                        <p>{search.length === 0 ? "Register an existing Unity project from the toolbar." : "Change the search text to see other projects."}</p>
                    </section>
                ) : null}
                {projects.length > 0 && view === "list" ? <ProjectsTable client={client} descending={descending} navigate={navigate} onChanged={() => void refresh()} onFeedback={setRegistrationMessage} onProjectChanged={updateProject} onSort={updateSort} projects={projects} sort={sort} /> : null}
                {projects.length > 0 && view === "grid" ? <div className="projects-grid">{projects.map((project) => <ProjectCard client={client} key={project.projectId ?? project.rootPath} onChanged={updateProject} onFeedback={setRegistrationMessage} onRefresh={() => void refresh()} project={project} navigate={navigate} />)}</div> : null}
            </div>
            {registrationMessage === undefined ? null : <p className="operation-feedback" role="status">{registrationMessage}</p>}
            <RegisterProjectDialog
                client={client}
                onChanged={() => {
                    setRegistrationMessage("Project registered");
                    void refresh();
                }}
                onClose={closeRegisterDialog}
                open={registerOpen}
                path={registerPath}
            />
            <CreateProjectDialog
                client={client}
                onCompleted={(projectId) => {
                    setRegistrationMessage("Project created");
                    setCreateOpen(false);
                    void refresh().then(() => navigate(`/projects/${projectId}`));
                }}
                onClose={() => {
                    setCreateOpen(false);
                    window.requestAnimationFrame(() => createButtonRef.current?.focus());
                }}
                open={createOpen}
            />
            <RestoreProjectDialog
                client={client}
                onCompleted={(projectId) => {
                    setRegistrationMessage("Project restored");
                    setRestoreOpen(false);
                    void refresh().then(() => navigate(`/projects/${projectId}`));
                }}
                onClose={() => {
                    setRestoreOpen(false);
                    window.requestAnimationFrame(() => restoreButtonRef.current?.focus());
                }}
                open={restoreOpen}
            />
        </section>
    );
}

function ProjectsTable({
    client,
    descending,
    navigate,
    onChanged,
    onFeedback,
    onProjectChanged,
    onSort,
    projects,
    sort
}: {
    client: GuiRpcClient;
    descending: boolean;
    navigate(path: string): void;
    onChanged(): void;
    onFeedback(message: string): void;
    onProjectChanged(project: ProjectSnapshot): void;
    onSort(sort: string): void;
    projects: ProjectSnapshot[];
    sort: string;
}) {
    const sortableHeader = (label: string, key: string) => {
        const active = sort === key;
        return (
            <DataTableHeader onSort={() => onSort(key)} sortDirection={active ? (descending ? "descending" : "ascending") : undefined}>
                {label}
            </DataTableHeader>
        );
    };
    return (
        <MaterialDataTable className="projects-table" label="Projects" minWidth={720}>
                <colgroup><col className="projects-column-project" /><col className="projects-column-type" /><col className="projects-column-unity" /><col className="projects-column-added" /><col className="projects-column-observed" /><col className="projects-column-actions" /></colgroup>
                <thead><tr>{sortableHeader("Project", "name")}{sortableHeader("Type", "type")}{sortableHeader("Unity", "unity")}{sortableHeader("Added", "added")}{sortableHeader("Last observed", "observed")}<DataTableHeader><span className="visually-hidden">Actions</span></DataTableHeader></tr></thead>
                <tbody>{projects.map((project) => {
                    const id = project.projectId;
                    return (
                        <tr key={id ?? project.rootPath}>
                            <td><strong>{projectName(project)}</strong><small title={displayProjectPath(project.rootPath)}>{displayProjectPath(project.rootPath)}</small></td>
                            <td><span className="project-type project-type--table"><Icon asset={projectTypeIcon(project.projectType)} /><span>{projectTypeLabel(project.projectType)}</span></span></td>
                            <td>{project.unityVersion || "Unknown"}</td>
                            <td>{formatRegistered(project.registeredAtMs)}</td>
                            <td>{formatObserved(project.observedAtMs)}</td>
                            <td>{id === undefined ? <span className="project-unregistered">Unregistered</span> : <ProjectRowActions client={client} navigate={navigate} onChanged={onChanged} onFeedback={onFeedback} onProjectChanged={onProjectChanged} project={project} />}</td>
                        </tr>
                    );
                })}</tbody>
        </MaterialDataTable>
    );
}

function ProjectRowActions({ client, context = "row", navigate, onChanged, onCopyCompleted, onFeedback, onProjectChanged, onRemoved, project }: {
    client: GuiRpcClient;
    context?: "row" | "workspace";
    navigate(path: string): void;
    onChanged(): void;
    onCopyCompleted?(projectId: string): void;
    onFeedback(message: string): void;
    onProjectChanged(project: ProjectSnapshot): void;
    onRemoved?(): void;
    project: ProjectSnapshot;
}) {
    const canReadProjects = useCapability(capabilities.projectsRead);
    const canManageProjects = useCapability(capabilities.projectsRegistry);
    const canCopyProjects = useCapability(capabilities.projectsCopy);
    const canDeleteProjects = useCapability(capabilities.projectsDelete);
    const canReadBackups = useCapability(capabilities.backupsRead);
    const canLaunchUnity = useCapability(capabilities.unityLaunch);
    const [copyOpen, setCopyOpen] = useState(false);
    const [openingDirectory, setOpeningDirectory] = useState(false);
    const [opening, setOpening] = useState(false);
    const [menuOpen, setMenuOpen] = useState(false);
    const [removalIntent, setRemovalIntent] = useState<"unregister" | "delete">("unregister");
    const [unregistering, setUnregistering] = useState(false);
    const [confirmUnregister, setConfirmUnregister] = useState(false);
    const [deletePlan, setDeletePlan] = useState<ProjectDeletePlan>();
    const [deleteConfirmation, setDeleteConfirmation] = useState("");
    const [deleting, setDeleting] = useState(false);
    const [deleteOperation, setDeleteOperation] = useState<Operation>();
    const menuAnchorRef = useRef<HTMLElement>(null);
    const projectId = project.projectId;
    const revision = project.revision;
    const workspace = context === "workspace";

    useEffect(() => {
        if (deleteOperation === undefined || !["queued", "running", "recovering", "cancelling", "interrupted"].includes(deleteOperation.state)) return;
        let active = true;
        const timer = window.setTimeout(() => {
            void client.operationGet(deleteOperation.operationId).then((next) => {
                if (!active) return;
                setDeleteOperation(next);
                if (next.state === "succeeded") {
                    setConfirmUnregister(false);
                    setDeletePlan(undefined);
                    onFeedback("Project directory permanently deleted.");
                    if (onRemoved !== undefined) onRemoved();
                    else onChanged();
                } else if (["failed", "cancelled"].includes(next.state)) {
                    setDeleting(false);
                    onFeedback(`Unable to delete project directory: ${next.errorCode ?? next.state}`);
                }
            }).catch((caught: unknown) => {
                if (active) onFeedback(`Unable to follow project deletion: ${safeError(caught).code}`);
            });
        }, 250);
        return () => { active = false; window.clearTimeout(timer); };
    }, [client, deleteOperation, onChanged, onFeedback, onRemoved]);
    if (projectId === undefined) return null;

    const openUnity = async () => {
        if (revision === undefined) return;
        setOpening(true);
        onFeedback("Opening Unity…");
        try {
            const options = await client.unityLaunchOptions(projectId, revision);
            if (options.exactMatchingInstallations.length !== 1) {
                navigate(`/projects/${projectId}/unity`);
                onFeedback(options.exactMatchingInstallations.length === 0
                    ? `Unity ${options.projectUnityVersion} is required. Choose a migration target.`
                    : "Choose the Unity installation for this launch.");
                return;
            }
            const result = await client.unityLaunch(projectId, options.exactMatchingInstallations[0]!.installationId, revision);
            onFeedback(`Unity launch ${result.launch.state}.`);
        } catch (caught: unknown) {
            onFeedback(`Unable to open Unity: ${safeError(caught).code}`);
        } finally {
            setOpening(false);
        }
    };

    const unregister = async () => {
        if (revision === undefined) return;
        setUnregistering(true);
        try {
            await client.projectUnregister(projectId, revision);
            setConfirmUnregister(false);
            onFeedback("Project unregistered. Files were not deleted.");
            if (onRemoved !== undefined) onRemoved();
            else onChanged();
        } catch (caught: unknown) {
            onFeedback(`Unable to unregister project: ${safeError(caught).code}`);
        } finally {
            setUnregistering(false);
        }
    };

    const openDirectory = async () => {
        setOpeningDirectory(true);
        setMenuOpen(false);
        try {
            await client.openProjectDirectory(projectId);
            onFeedback("Project directory opened.");
        } catch (caught: unknown) {
            onFeedback(`Unable to open project directory: ${safeError(caught).code}`);
        } finally {
            setOpeningDirectory(false);
        }
    };

    const beginCopy = () => {
        setMenuOpen(false);
        setCopyOpen(true);
    };

    const planDelete = async () => {
        if (revision === undefined) return;
        setDeleting(true);
        try {
            const result = await client.projectPlanDeleteDirectory(projectId, revision);
            setDeletePlan(result.plan);
            setDeleteConfirmation("");
        } catch (caught: unknown) {
            onFeedback(`Unable to plan project deletion: ${safeError(caught).code}`);
        } finally {
            setDeleting(false);
        }
    };

    const beginUnregister = () => {
        setMenuOpen(false);
        setRemovalIntent("unregister");
        setDeletePlan(undefined);
        setConfirmUnregister(true);
    };

    const beginDeleteDirectory = () => {
        setMenuOpen(false);
        setRemovalIntent("delete");
        setDeletePlan(undefined);
        setConfirmUnregister(true);
        void planDelete();
    };

    const applyDelete = async () => {
        if (revision === undefined || deletePlan === undefined || deleteConfirmation !== deletePlan.normalizedLeaf) return;
        setDeleting(true);
        try {
            const result = await client.projectApplyDeleteDirectory(deletePlan.planId, revision);
            const operation = await client.operationGet(result.operationId);
            setDeleteOperation(operation);
            onFeedback("Project directory deletion started.");
        } catch (caught: unknown) {
            setDeleting(false);
            onFeedback(`Unable to delete project directory: ${safeError(caught).code}`);
        }
    };

    return (
        <>
            <div className={workspace ? "project-row-actions project-workspace-more-actions" : "project-row-actions"}>
                {workspace ? null : <ProjectFavoriteButton client={client} onChanged={onProjectChanged} onFeedback={onFeedback} onRefresh={onChanged} project={project} />}
                {workspace ? null : <Button className="project-open-unity-action" disabled={!canLaunchUnity || opening || revision === undefined} onClick={() => void openUnity()} title={capabilityUnavailableTitle(canLaunchUnity, capabilities.unityLaunch)} type="button">
                    <Icon asset={playArrowIcon} slot="icon" />
                    <StateSizedLabel current={opening ? "Opening…" : "Open Unity"} labels={["Open Unity", "Opening…"]} />
                </Button>}
                {workspace ? null : <Button disabled={!canReadProjects} onClick={() => navigate(`/projects/${projectId}`)} title={capabilityUnavailableTitle(canReadProjects, capabilities.projectsRead)} type="button" variant="tonal">Manage</Button>}
                {workspace ? null : <Button disabled={!canReadBackups} onClick={() => navigate(`/projects/${projectId}/backups`)} title={capabilityUnavailableTitle(canReadBackups, capabilities.backupsRead)} type="button" variant="tonal">Backups</Button>}
                <IconButton className="project-more-actions" label={`More actions for ${projectName(project)}`} onClick={() => setMenuOpen(true)} ref={menuAnchorRef} type="button">
                    <Icon asset={moreVertIcon} size={24} />
                </IconButton>
                <Menu anchorRef={menuAnchorRef} className="project-actions-menu" onClose={() => setMenuOpen(false)} open={menuOpen}>
                    <MenuItem className="project-actions-menu-item" disabled={!canReadProjects || openingDirectory} label={openingDirectory ? "Opening Project Directory…" : "Open Project Directory"} onClick={() => void openDirectory()} title={capabilityUnavailableTitle(canReadProjects, capabilities.projectsRead)} />
                    <MenuItem className="project-actions-menu-item" disabled={!canCopyProjects || revision === undefined} label="Copy Project" onClick={beginCopy} title={capabilityUnavailableTitle(canCopyProjects, capabilities.projectsCopy)} />
                    {workspace ? <>
                        <MenuItem className="project-actions-menu-item project-actions-menu-item--danger project-actions-menu-item--danger-group" disabled={!canManageProjects || revision === undefined} label="Remove from list" onClick={beginUnregister} title={capabilityUnavailableTitle(canManageProjects, capabilities.projectsRegistry)} />
                        <MenuItem className="project-actions-menu-item project-actions-menu-item--danger" disabled={!canDeleteProjects || revision === undefined} label="Delete Project Directory…" onClick={beginDeleteDirectory} title={capabilityUnavailableTitle(canDeleteProjects, capabilities.projectsDelete)} />
                    </> : <MenuItem className="project-actions-menu-item project-actions-menu-item--danger" disabled={!canManageProjects || revision === undefined} label="Remove Project" onClick={beginUnregister} title={capabilityUnavailableTitle(canManageProjects, capabilities.projectsRegistry)} />}
                </Menu>
            </div>
            <Dialog onClose={() => { if (!deleting) { setConfirmUnregister(false); setDeletePlan(undefined); setRemovalIntent("unregister"); } }} open={confirmUnregister} title={deletePlan === undefined ? removalIntent === "delete" ? "Preparing permanent deletion…" : workspace ? "Remove this project from the list?" : "Remove this project?" : "Permanently delete project directory?"}>
                {deletePlan === undefined && removalIntent === "unregister" ? <>
                    <p>{workspace ? "Remove the ALCOMD registration for this project?" : "Choose whether to remove only the ALCOMD registration or permanently delete the local Unity project directory."}</p>
                    <p>Removing from the list does not delete files.</p>
                    <div className={workspace ? "dialog-actions" : "dialog-actions dialog-actions--split"}>
                        <Button disabled={unregistering || deleting} onClick={() => setConfirmUnregister(false)} type="button" variant="text">Cancel</Button>
                        <Button disabled={!canManageProjects || unregistering || deleting || revision === undefined} onClick={() => void unregister()} title={capabilityUnavailableTitle(canManageProjects, capabilities.projectsRegistry)} type="button" variant="tonal">
                            <StateSizedLabel current={unregistering ? "Removing…" : "Remove from list"} labels={["Remove from list", "Removing…"]} />
                        </Button>
                        {workspace ? null : <Button className="material-button--danger" disabled={!canDeleteProjects || unregistering || deleting || revision === undefined} onClick={() => { setRemovalIntent("delete"); void planDelete(); }} title={capabilityUnavailableTitle(canDeleteProjects, capabilities.projectsDelete)} type="button" variant="text">Delete Project Directory…</Button>}
                    </div>
                </> : deletePlan === undefined ? <>
                    <p>Loading the destructive review. No files have been changed.</p>
                    <div className="dialog-actions"><Button disabled={deleting} onClick={() => setConfirmUnregister(false)} type="button" variant="text">Cancel</Button></div>
                </> : <>
                    <p><strong>This permanently deletes local files and does not use the Recycle Bin or Trash.</strong></p>
                    <p>No automatic backup will be created. This is different from removing the project from the list.</p>
                    <p className="project-delete-path"><code>{deletePlan.canonicalRootPath}</code></p>
                    <p>Unity writer observation: <strong>{deletePlan.writerEvidence.state}</strong>.</p>
                    <TextField aria-label="Confirm project directory name" label={`Type ${deletePlan.normalizedLeaf} to confirm`} onInput={setDeleteConfirmation} required value={deleteConfirmation} />
                    {deleteOperation === undefined ? null : <p>Operation: <code>{deleteOperation.operationId}</code> · {deleteOperation.state}</p>}
                    <div className="dialog-actions">
                        <Button disabled={deleting} onClick={() => { setDeletePlan(undefined); setDeleteConfirmation(""); }} type="button" variant="text">Back</Button>
                        <Button className="material-button--danger" disabled={deleting || deleteConfirmation !== deletePlan.normalizedLeaf} onClick={() => void applyDelete()} type="button" variant="text">
                            <StateSizedLabel current={deleting ? "Deleting…" : "Permanently delete"} labels={["Permanently delete", "Deleting…"]} />
                        </Button>
                    </div>
                </>}
            </Dialog>
            <CopyProjectDialog
                client={client}
                onCompleted={(targetProjectId) => {
                    if (onCopyCompleted !== undefined) onCopyCompleted(targetProjectId);
                    else {
                        onFeedback("Project copy completed.");
                        onChanged();
                    }
                }}
                onClose={() => setCopyOpen(false)}
                open={copyOpen}
                project={project}
            />
        </>
    );
}

function CopyProjectDialog({ client, onCompleted, onClose, open, project }: { client: GuiRpcClient; onCompleted(targetProjectId: string): void; onClose(): void; open: boolean; project: ProjectSnapshot }) {
    const [targetParent, setTargetParent] = useState("");
    const [targetLeaf, setTargetLeaf] = useState("");
    const [plan, setPlan] = useState<ProjectCopyPlan>();
    const [operation, setOperation] = useState<Operation>();
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState<RpcError>();

    useEffect(() => {
        if (!open) {
            setPlan(undefined);
            setOperation(undefined);
            setBusy(false);
            setError(undefined);
            return;
        }
        setTargetParent(projectParentPath(project.rootPath));
        setTargetLeaf(`${projectName(project)} Copy`);
    }, [open, project]);

    useEffect(() => {
        if (operation === undefined || !["queued", "running", "recovering", "cancelling", "interrupted"].includes(operation.state)) return;
        let active = true;
        const timer = window.setTimeout(() => {
            void client.operationGet(operation.operationId).then((next) => {
                if (!active) return;
                setOperation(next);
                if (next.state === "succeeded" && plan !== undefined) onCompleted(plan.targetProjectId);
            }).catch((caught: unknown) => {
                if (active) setError(safeError(caught));
            });
        }, 300);
        return () => {
            active = false;
            window.clearTimeout(timer);
        };
    }, [client, onCompleted, operation, plan]);

    const chooseTargetParent = async () => {
        setBusy(true);
        setError(undefined);
        try {
            const selected = await client.selectDirectory();
            if (selected !== undefined) setTargetParent(selected);
        } catch (caught: unknown) {
            setError(safeError(caught));
        } finally {
            setBusy(false);
        }
    };

    const createPlan = async () => {
        if (project.projectId === undefined || project.revision === undefined) return;
        setBusy(true);
        setError(undefined);
        try {
            const result = await client.projectPlanCopy(project.projectId, project.revision, targetParent, targetLeaf.trim());
            setPlan(result.plan);
        } catch (caught: unknown) {
            setError(safeError(caught));
        } finally {
            setBusy(false);
        }
    };

    const apply = async () => {
        if (plan === undefined) return;
        setBusy(true);
        setError(undefined);
        try {
            const accepted = await client.projectApplyCopy(plan.planId, plan.sourceProjectRevision);
            setOperation(await client.operationGet(accepted.operationId));
        } catch (caught: unknown) {
            setError(safeError(caught));
        } finally {
            setBusy(false);
        }
    };

    const cancel = async () => {
        if (operation === undefined) return;
        setBusy(true);
        try {
            const result = await client.operationCancel(operation.operationId, operation.revision);
            setOperation(result.operation);
        } catch (caught: unknown) {
            setError(safeError(caught));
        } finally {
            setBusy(false);
        }
    };

    const terminal = operation !== undefined && ["succeeded", "failed", "cancelled"].includes(operation.state);
    return (
        <Dialog onClose={onClose} open={open} title="Copy project">
            <div className="project-copy-review">
                {plan === undefined ? (
                    <>
                        <p>Choose the new project name. By default, ALCOMD creates the copy beside the original project without overwriting an existing target.</p>
                        <TextField aria-label="Copied project name" label="Project name" onInput={setTargetLeaf} value={targetLeaf} />
                        <div className="project-copy-location">
                            <TextField aria-label="Copy project location" label="Project location" readOnly value={targetParent} />
                            <Button disabled={busy} onClick={() => void chooseTargetParent()} type="button" variant="tonal">Choose…</Button>
                        </div>
                        <div className="dialog-actions">
                            <Button disabled={busy} onClick={onClose} type="button" variant="text">Cancel</Button>
                            <Button widthLabels={["Planning…", "Review copy"]} disabled={busy || targetParent.length === 0 || targetLeaf.trim().length === 0} onClick={() => void createPlan()} type="button">{busy ? "Planning…" : "Review copy"}</Button>
                        </div>
                    </>
                ) : operation === undefined ? (
                    <>
                        <p><strong>Source:</strong> {projectName(project)}</p>
                        <p><strong>Destination:</strong> {plan.targetParentCanonicalPath}\{plan.normalizedTargetLeaf}</p>
                        <p><strong>Unity writer:</strong> {plan.writerEvidence.state.replaceAll("_", " ")}</p>
                        <p><strong>Copy profile:</strong> v{plan.profile.version}; excludes Logs, Obj, Temp and .git.</p>
                        <div className="dialog-actions">
                            <Button disabled={busy} onClick={() => setPlan(undefined)} type="button" variant="text">Back</Button>
                            <Button widthLabels={["Starting…", "Start copy"]} disabled={busy} onClick={() => void apply()} type="button">{busy ? "Starting…" : "Start copy"}</Button>
                        </div>
                    </>
                ) : (
                    <>
                        <p role="status">Copy {operation.state.replaceAll("_", " ")}{operation.progress?.phase === undefined ? "." : `: ${operation.progress.phase.replaceAll("_", " ")}.`}</p>
                        <div className="dialog-actions">
                            {!terminal ? <Button disabled={busy} onClick={() => void cancel()} type="button" variant="text">Cancel copy</Button> : null}
                            <Button disabled={!terminal} onClick={onClose} type="button">Close</Button>
                        </div>
                    </>
                )}
                {error === undefined ? null : <p className="inline-error" role="alert">Copy failed: {error.code}</p>}
            </div>
        </Dialog>
    );
}

function StateSizedLabel({ current, labels }: { current: string; labels: readonly string[] }) {
    return (
        <span className="state-sized-label">
            {labels.map((label) => <span aria-hidden="true" className="state-sized-label-reserve" key={label}>{label}</span>)}
            <span className="state-sized-label-current">{current}</span>
        </span>
    );
}

function ProjectCard({ client, onChanged, onFeedback, onRefresh, project, navigate }: { client: GuiRpcClient; onChanged(project: ProjectSnapshot): void; onFeedback(message: string): void; onRefresh(): void; project: ProjectSnapshot; navigate(path: string): void }) {
    const id = project.projectId;
    return (
        <article className="project-card">
            <h2>{projectName(project)}</h2>
            <p className="project-path" title={displayProjectPath(project.rootPath)}>{displayProjectPath(project.rootPath)}</p>
            <p><span className="project-type"><Icon asset={projectTypeIcon(project.projectType)} /><span>{projectTypeLabel(project.projectType)}</span></span> · Unity {project.unityVersion || "unknown"}</p>
            <p className="project-meta">Added {formatRegistered(project.registeredAtMs)} · observed {formatObserved(project.observedAtMs)}</p>
            {id === undefined ? <span className="project-unregistered">Unregistered</span> : <div className="project-card-actions"><ProjectFavoriteButton client={client} onChanged={onChanged} onFeedback={onFeedback} onRefresh={onRefresh} project={project} /><Button onClick={() => navigate(`/projects/${id}`)} variant="tonal">Manage</Button><Button onClick={() => navigate(`/projects/${id}/backups`)} variant="text">Backups</Button></div>}
        </article>
    );
}

function ProjectFavoriteButton({ client, onChanged, onFeedback, onRefresh, project }: { client: GuiRpcClient; onChanged(project: ProjectSnapshot): void; onFeedback(message: string): void; onRefresh(): void; project: ProjectSnapshot }) {
    const canManageProjects = useCapability(capabilities.projectsRegistry);
    const [busy, setBusy] = useState(false);
    const projectId = project.projectId;
    const revision = project.revision;
    const favorite = project.favorite === true;
    const toggle = async () => {
        if (projectId === undefined || revision === undefined) return;
        setBusy(true);
        try {
            const result = await client.projectSetFavorite(projectId, !favorite, revision);
            onChanged(result.project);
            onFeedback(result.project.favorite === true ? "Project added to favorites." : "Project removed from favorites.");
        } catch (caught: unknown) {
            const error = safeError(caught);
            onFeedback(`Unable to update favorite: ${error.code}`);
            if (error.code === "revision_conflict") onRefresh();
        } finally {
            setBusy(false);
        }
    };
    const label = busy ? "Updating favorite" : favorite ? "Remove from favorites" : "Add to favorites";
    return <IconButton aria-pressed={favorite} className="project-favorite-action" disabled={!canManageProjects || busy || revision === undefined} label={label} onClick={() => void toggle()} title={capabilityUnavailableTitle(canManageProjects, capabilities.projectsRegistry) ?? label} type="button"><Icon asset={starIcon} /></IconButton>;
}

function RegisterProjectDialog({ client, onChanged, onClose, open, path }: { client: GuiRpcClient; onChanged(): void; onClose(): void; open: boolean; path: string }) {
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState<RpcError>();
    const confirmRef = useRef<HTMLElement>(null);

    useEffect(() => {
        if (open) return;
        setBusy(false);
        setError(undefined);
    }, [open]);

    useEffect(() => {
        if (!open) return;
        window.requestAnimationFrame(() => confirmRef.current?.focus());
    }, [open]);

    const register = async () => {
        setBusy(true);
        setError(undefined);
        try {
            await client.projectRegister(path);
            onChanged();
            onClose();
        } catch (caught: unknown) {
            setBusy(false);
            setError(safeError(caught));
        }
    };

    return (
        <Dialog onClose={onClose} open={open} title="Register this project?">
            <div className="project-register-review">
                <p>ALCOMD will inspect this Unity project and add it to the per-user registry.</p>
                <code>{path}</code>
                <div className="dialog-actions">
                    <Button disabled={busy} onClick={onClose} type="button" variant="text">Cancel</Button>
                    <Button widthLabels={["Registering…", "Confirm"]} disabled={busy || path.length === 0} onClick={() => void register()} ref={confirmRef} type="button">{busy ? "Registering…" : "Confirm"}</Button>
                </div>
            </div>
            {error === undefined ? null : <p className="inline-error" role="alert">Registration failed: {error.code}</p>}
        </Dialog>
    );
}

function projectName(project: ProjectSnapshot): string {
    return displayProjectPath(project.rootPath).split(/[\\/]/).at(-1) ?? "Unity project";
}

function projectParentPath(rootPath: string): string {
    const normalized = rootPath.replace(/[\\/]+$/, "");
    const separatorIndex = Math.max(normalized.lastIndexOf("\\"), normalized.lastIndexOf("/"));
    if (separatorIndex < 0) return "";
    if (separatorIndex === 0) return normalized.slice(0, 1);
    if (separatorIndex === 2 && /^[A-Za-z]:[\\/]/.test(normalized)) return normalized.slice(0, 3);
    return normalized.slice(0, separatorIndex);
}

function formatObserved(value: number): string {
    const elapsed = Date.now() - value;
    const absoluteElapsed = Math.abs(elapsed);
    const minute = 60_000;
    const hour = 60 * minute;
    const day = 24 * hour;
    const week = 7 * day;
    const month = 30 * day;
    const year = 365 * day;
    const formatter = new Intl.RelativeTimeFormat("en", { numeric: "always" });

    if (!Number.isFinite(value)) return "Unknown";
    if (absoluteElapsed < minute) return elapsed >= 0 ? "moments ago" : "in moments";
    if (absoluteElapsed < hour) return formatter.format(-Math.trunc(elapsed / minute), "minute");
    if (absoluteElapsed < day) return formatter.format(-Math.trunc(elapsed / hour), "hour");
    if (absoluteElapsed < week) return formatter.format(-Math.trunc(elapsed / day), "day");
    if (absoluteElapsed < month) return formatter.format(-Math.trunc(elapsed / week), "week");
    if (absoluteElapsed < year) return formatter.format(-Math.trunc(elapsed / month), "month");
    return formatter.format(-Math.trunc(elapsed / year), "year");
}

function formatRegistered(value: number | undefined): string {
    if (value === undefined || !Number.isFinite(value)) return "Unknown";
    const date = new Date(value);
    const year = date.getFullYear().toString().padStart(4, "0");
    const month = (date.getMonth() + 1).toString().padStart(2, "0");
    const day = date.getDate().toString().padStart(2, "0");
    return `${year}-${month}-${day}`;
}

function displayProjectPath(path: string): string {
    if (path.startsWith("\\\\?\\UNC\\")) return `\\\\${path.slice(8)}`;
    if (path.startsWith("\\\\?\\")) return path.slice(4);
    return path;
}

function projectTypeIcon(projectType: string) {
    const normalized = projectType.toLocaleLowerCase();
    if (normalized.includes("avatar")) return accountCircleIcon;
    if (normalized.includes("world")) return publicIcon;
    return helpIcon;
}

function projectTypeLabel(projectType: string): string {
    const normalized = projectType.toLocaleLowerCase();
    if (normalized.includes("avatar")) return "Avatar";
    if (normalized.includes("world")) return "World";
    return projectType.length === 0 ? "Unknown" : projectType;
}

export function ProjectDetailPage(props: PageProps & { projectId: string }) {
    return <ProjectPackageWorkspace {...props} />;
}

export function ProjectPackagesPage(props: PageProps & { projectId: string }) {
    return <ProjectPackageWorkspace {...props} />;
}

interface WorkspaceCatalogVersion extends RepositoryPackageVersion {
    repositoryId?: string;
    source: string;
    sourceKey: string;
    sourceKind: RepositorySnapshot["source"]["kind"] | "user-package";
    sourceSelector: PackageSourceSelector;
}

interface PackageWorkspaceRow {
    availableVersions: string[];
    evidence?: PackageCandidateSummary;
    displayName: string;
    installedVersion?: string;
    linkTarget?: {
        changelog: boolean;
        documentation: boolean;
        repositoryId: string;
        version: string;
    };
    packageId: string;
    requestedRange?: string;
    sourceKinds: Array<RepositorySnapshot["source"]["kind"] | "user-package">;
    sourceOptions: Array<{ key: string; label: string; selector: PackageSourceSelector }>;
    sources: string[];
    status: "available" | "installed" | "missing-source";
}

function PackageVersionMenu({ client, project, snapshot, disabled, canPlan, onSelect, onStale, row }: { client: GuiRpcClient; project: ProjectSnapshot; snapshot?: string; disabled: boolean; canPlan: boolean; onSelect(version: PackageCandidateEvidence): void; onStale(): void; row: PackageWorkspaceRow }) {
    const [open, setOpen] = useState(false);
    const [items, setItems] = useState<PackageCandidateEvidence[]>([]);
    const [cursor, setCursor] = useState<CandidateCursor | null>();
    const [error, setError] = useState<string>();
    const [busy, setBusy] = useState(false);
    const pending = useRef(false);
    const generation = useRef(0);
    const traversal = useRef({ pages: 0, count: 0 });
    const anchorRef = useRef<HTMLElement>(null);
    useEffect(() => { generation.current += 1; traversal.current = { pages: 0, count: 0 }; setOpen(false); setItems([]); setCursor(undefined); setError(undefined); }, [snapshot]);
    const load = async (next?: CandidateCursor) => {
        if (pending.current || !snapshot || project.revision === undefined || project.projectId === undefined) return;
        pending.current = true;
        setBusy(true);
        setError(undefined);
        const current = generation.current;
        try {
            const page = await queryCandidates(client, { projectId: project.projectId, expectedRevision: project.revision, expectedSnapshot: snapshot, view: { kind: "versions", packageId: row.packageId }, limit: 64, ...(next ? { cursor: next } : {}) });
            if (current !== generation.current) return;
            validateVersionPage(page, { projectId: project.projectId, projectRevision: project.revision, packageId: row.packageId, fingerprint: snapshot, ...traversal.current, offset: next?.offset ?? 0 });
            traversal.current = { pages: traversal.current.pages + 1, count: traversal.current.count + page.items.length };
            setItems((existing) => next ? [...existing, ...page.items] : page.items);
            setCursor(page.nextCursor);
        } catch (caught) {
            if (current !== generation.current) return;
            const code = safeError(caught).code;
            setItems([]); setCursor(undefined); setError(code);
            if (code === "package_candidate_evidence_stale") onStale();
        } finally { pending.current = false; setBusy(false); }
    };
    return <>
        <Button aria-expanded={open} aria-haspopup="menu" aria-label={`Version for ${row.displayName}: ${row.installedVersion ?? "Not installed"}`} disabled={disabled || !snapshot} onClick={() => { setOpen(true); if (cursor === undefined) void load(); }} ref={anchorRef} type="button" variant="text">
            {row.installedVersion ?? "Not installed"}
        </Button>
        <Menu anchorRef={anchorRef} onClose={() => setOpen(false)} open={open}>
            {items.map((option) => <MenuItem
                disabled={disabled || !canPlan || busy || !actionableVersion(option)}
                key={`${candidateSourceKey(option.source)}:${option.version}`}
                label={`${option.version} · ${row.sourceOptions.find((source) => source.key === candidateSourceKey(option.source))?.label ?? candidateSourceKey(option.source)}${option.classification === "prerelease" ? " · Prerelease" : ""}${option.relation === "same_precedence" ? " · Installed precedence" : ""}${option.reasons.length ? ` · ${option.reasons.join(", ").replaceAll("_", " ")}` : ""}${canPlan ? "" : " · Planning unavailable"}`}
                onClick={() => { setOpen(false); onSelect(option); }}
            />)}
            {busy ? <MenuItem disabled label="Loading versions…" /> : null}
            {error ? <MenuItem disabled label={`Versions unavailable: ${error}`} /> : null}
            {!busy && !error && cursor === null && items.length === 0 ? <MenuItem disabled label="No recorded versions" /> : null}
            {cursor ? <MenuItem disabled={busy} label="Load more versions" onClick={() => void load(cursor)} /> : null}
        </Menu>
    </>;
}

function PackageSourceMenu({ label, onChange, options, value, disabled }: { label: string; onChange(value: string): void; options: PackageWorkspaceRow["sourceOptions"]; value: string; disabled: boolean }) {
    const [open, setOpen] = useState(false);
    const anchorRef = useRef<HTMLElement>(null);
    const selected = options.find((option) => option.key === value);
    return (
        <>
            <Button aria-expanded={open} aria-label={label} className="package-row-source-menu" disabled={disabled} onClick={() => setOpen(true)} ref={anchorRef} type="button" variant="outlined">
                {selected?.label ?? (value ? "Selected source unavailable" : "Automatic source selection")}
            </Button>
            <Menu anchorRef={anchorRef} onClose={() => setOpen(false)} open={open}>
                <MenuItem label="Automatic source selection" onClick={() => { onChange(""); setOpen(false); }} />
                {options.map((option) => <MenuItem key={option.key} label={option.label} onClick={() => { onChange(option.key); setOpen(false); }} />)}
            </Menu>
        </>
    );
}

function PackageRowMoreMenu({ canPlanV1, canPlanV2, client, onAction, row }: {
    canPlanV1: boolean;
    canPlanV2: boolean;
    client: GuiRpcClient;
    onAction(action: PackageActionSelection["action"]): void;
    row: PackageWorkspaceRow;
}) {
    const [error, setError] = useState<string>();
    const [open, setOpen] = useState(false);
    const anchorRef = useRef<HTMLElement>(null);
    const hasMenuActions = row.installedVersion !== undefined || row.linkTarget?.documentation === true || row.linkTarget?.changelog === true;
    const openLink = async (kind: "documentation" | "changelog") => {
        if (row.linkTarget === undefined) return;
        setOpen(false);
        setError(undefined);
        try {
            await client.openPackageLink(row.linkTarget.repositoryId, row.packageId, row.linkTarget.version, kind);
        } catch (caught: unknown) {
            setError(safeError(caught).code);
        }
    };
    if (!hasMenuActions) return null;
    return (
        <>
            <IconButton className="package-more-actions" label={`More actions for ${row.displayName}`} onClick={() => setOpen(true)} ref={anchorRef} type="button">
                <Icon asset={moreVertIcon} size={24} />
            </IconButton>
            <Menu anchorRef={anchorRef} className="package-actions-menu" onClose={() => setOpen(false)} open={open}>
                {row.installedVersion === undefined ? null : <MenuItem disabled={!canPlanV2} label="Reinstall" onClick={() => { setOpen(false); onAction("reinstall"); }} title={capabilityUnavailableTitle(canPlanV2, capabilities.packagesPlanV2)} />}
                {row.linkTarget?.documentation === true ? <MenuItem label="Documentation" onClick={() => void openLink("documentation")} /> : null}
                {row.linkTarget?.changelog === true ? <MenuItem label="Changelog" onClick={() => void openLink("changelog")} /> : null}
                {row.installedVersion === undefined ? null : <MenuItem className="package-actions-menu-item--danger" disabled={!canPlanV1} label="Remove" onClick={() => { setOpen(false); onAction("remove"); }} title={capabilityUnavailableTitle(canPlanV1, capabilities.packagesPlanV1)} />}
            </Menu>
            {error === undefined ? null : <span className="visually-hidden" role="alert">Package link unavailable: {error}</span>}
        </>
    );
}

function PackageWorkspaceMoreMenu({ canPlanV1, canPlanV2, onResolve, onReinstallAll }: { canPlanV1: boolean; canPlanV2: boolean; onResolve(): void; onReinstallAll(): void }) {
    const [open, setOpen] = useState(false);
    const anchorRef = useRef<HTMLElement>(null);
    return (
        <>
            <IconButton label="More package actions" onClick={() => setOpen(true)} ref={anchorRef} type="button">
                <Icon asset={moreVertIcon} size={24} />
            </IconButton>
            <Menu anchorRef={anchorRef} onClose={() => setOpen(false)} open={open}>
                <MenuItem disabled={!canPlanV1} label="Resolve Dependencies…" onClick={() => { setOpen(false); onResolve(); }} title={capabilityUnavailableTitle(canPlanV1, capabilities.packagesPlanV1)} />
                <MenuItem disabled={!canPlanV2} label="Reinstall All Installed Packages…" onClick={() => { setOpen(false); onReinstallAll(); }} title={capabilityUnavailableTitle(canPlanV2, capabilities.packagesPlanV2)} />
            </Menu>
        </>
    );
}

interface PackageWorkspaceValue {
    sourceSelectionKey: string;
    sourceRegistry: Record<string, { label: string; kind: "local" | "remote" | "user-package" }>;
    candidates?: CandidateSummarySet;
    candidateError?: string;
    catalog: WorkspaceCatalogVersion[];
    installations: UnityInstallation[];
    launchOptions: UnityLaunchOptionsResult;
    project: ProjectSnapshot;
    repositories: RepositorySnapshot[];
    settings: SettingsGetResult;
}

function PackageFilters({ client, settings, repositories, onChanged, filter, setFilter, sourceFilter, setSourceFilter, canUseUserPackages }: {
    client: GuiRpcClient;
    settings: SettingsGetResult;
    repositories: RepositorySnapshot[];
    onChanged(): void;
    filter: string;
    setFilter(value: string): void;
    sourceFilter: string;
    setSourceFilter(value: "all" | "local" | "remote" | "user-package"): void;
    canUseUserPackages: boolean;
}) {
    const [open, setOpen] = useState(false);
    const [confirmPrerelease, setConfirmPrerelease] = useState(false);
    const [snapshot, setSnapshot] = useState(settings);
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState<RpcError>();
    const anchorRef = useRef<HTMLElement>(null);
    const saving = useRef(false);
    useEffect(() => setSnapshot(settings), [settings]);
    const save = async (update: Partial<OfficialSettings["packages"]>) => {
        if (saving.current) return;
        saving.current = true;
        setBusy(true);
        setError(undefined);
        try {
            const next = await client.settingsUpdate(snapshot.revision, { packages: update });
            setSnapshot(next);
            onChanged();
        } catch (caught: unknown) {
            setError(safeError(caught));
        } finally {
            saving.current = false;
            setBusy(false);
        }
    };
    return <>
        <Button aria-expanded={open} aria-haspopup="dialog" className="package-filter-trigger" onClick={() => setOpen(!open)} ref={anchorRef} type="button" variant="tonal">Filter packages</Button>
        <FilterPopover anchorRef={anchorRef} label="Package filters" onClose={() => setOpen(false)} open={open}>
            <fieldset disabled={busy}>
                <legend>Repositories</legend>
                {repositories.map((repository) => repository.repositoryId === undefined ? null : <Checkbox
                    checked={!snapshot.settings.packages.hiddenRepositoryIds.includes(repository.repositoryId)}
                    disabled={busy}
                    key={repository.repositoryId}
                    label={repository.name ?? repository.declaredId ?? sourceText(repository)}
                    onChange={(checked) => {
                        const hidden = new Set(snapshot.settings.packages.hiddenRepositoryIds);
                        if (checked) hidden.delete(repository.repositoryId as string);
                        else hidden.add(repository.repositoryId as string);
                        void save({ hiddenRepositoryIds: [...hidden].sort() });
                    }}
                />)}
                {canUseUserPackages ? <Checkbox checked={!snapshot.settings.packages.hideLocalUserPackages} disabled={busy} label="Local User Packages" onChange={(checked) => void save({ hideLocalUserPackages: !checked })} /> : null}
            </fieldset>
            <fieldset disabled={busy}>
                <legend>Other filters</legend>
                <Checkbox checked={snapshot.settings.packages.showPrerelease} disabled={busy} label="Show prerelease versions" onChange={(checked) => { if (checked) { setOpen(false); setConfirmPrerelease(true); } else void save({ showPrerelease: false }); }} />
                <Select label="Status" onChange={setFilter} options={[{ label: "All packages", value: "all" }, { label: "Installed", value: "installed" }, { label: "Available", value: "available" }, { label: "Missing source", value: "missing" }]} value={filter} />
                <Select label="Source" onChange={(value) => setSourceFilter(value as "all" | "local" | "remote" | "user-package")} options={[{ label: "All", value: "all" }, { label: "Remote", value: "remote" }, { label: "Local repository", value: "local" }, ...(canUseUserPackages ? [{ label: "User Packages", value: "user-package" }] : [])]} value={sourceFilter} />
            </fieldset>
            {busy ? <p role="status">Saving filters…</p> : null}
            {error === undefined ? null : <div role="alert"><p>{error.code === "revision_conflict" ? "Settings changed elsewhere. Reload filters before trying again." : `Could not save filters: ${error.code}`}</p><Button onClick={() => { setError(undefined); onChanged(); }} type="button" variant="text">Reload filters</Button></div>}
        </FilterPopover>
        <Dialog onClose={() => { setConfirmPrerelease(false); setOpen(true); }} open={confirmPrerelease} title="Show prerelease versions?">
            <p>Prerelease packages may be unstable. Showing them does not install or update any package.</p>
            <div className="dialog-actions"><Button onClick={() => setConfirmPrerelease(false)} type="button" variant="text">Cancel</Button><Button onClick={() => { setConfirmPrerelease(false); void save({ showPrerelease: true }); }} type="button">Show prerelease versions</Button></div>
        </Dialog>
    </>;
}

interface RepositoryRefreshProgress {
    completed: number;
    failures: Array<{ code: string; repositoryId: string }>;
    results: Array<{ code?: string; repositoryId: string; status: "failure" | "success" }>;
    running: boolean;
    successes: number;
    total: number;
}

async function listAllRepositories(client: GuiRpcClient): Promise<RepositorySnapshot[]> {
    const repositories: RepositorySnapshot[] = [];
    let cursor: RegistryCursor | undefined;
    do {
        const page = await client.repositoriesList(cursor);
        repositories.push(...page.repositories);
        cursor = page.nextCursor;
    } while (cursor !== undefined);
    return repositories;
}

async function loadPackageWorkspace(client: GuiRpcClient, projectId: string, includeUserPackages: boolean, canQuery: boolean, sourceSelections: Record<string, string>): Promise<PackageWorkspaceValue> {
    const project = (await client.projectGet(projectId)).project;
    if (project.revision === undefined) throw { code: "project_not_registered" };
    const [repositories, settings, userPackages, installations, launchOptions] = await Promise.all([
        listAllRepositories(client),
        client.settingsGet(),
        includeUserPackages ? listAllUserPackages(client) : Promise.resolve([]),
        loadAllUnityInstallations(client),
        client.unityLaunchOptions(projectId, project.revision)
    ]);
    let candidates: CandidateSummarySet | undefined;
    let candidateError: string | undefined = canQuery ? undefined : "Candidate evidence unavailable: packages.candidates.v1 was not negotiated.";
    try {
        if (canQuery) {
            const sources = Object.entries(sourceSelections).filter(([, key]) => key.length > 0).map(([packageId, key]) => ({ packageId, source: key.startsWith("repository:") ? { kind: "repository" as const, repository_id: key.slice(11) } : { kind: "user_package" as const, user_package_id: key.slice(13) } }));
            if (sources.length > 256) throw { code: "package_candidate_limit_exceeded" };
            candidates = await readCandidateSummary(client, { projectId, expectedRevision: project.revision, view: { kind: "summary", sources } });
            if (candidates.snapshot.configRevision !== settings.revision) throw { code: "package_candidate_evidence_stale" };
            const lockedIds = project.lockedDependencies.map((item) => item.packageId);
            if (lockedIds.length > 100000) throw { code: "package_candidate_limit_exceeded" };
            const lockedRows: PackageCandidateSummary[] = [];
            for (let offset = 0; offset < lockedIds.length; offset += 256) {
                const packageIds = lockedIds.slice(offset, offset + 256);
                const batch = await readCandidateSummary(client, { projectId, expectedRevision: project.revision, expectedSnapshot: candidates.snapshot.fingerprint, view: { kind: "summary", packageIds, sources: sources.filter((source) => packageIds.includes(source.packageId)) } });
                if (batch.snapshot.fingerprint !== candidates.snapshot.fingerprint || batch.items.length !== packageIds.length || packageIds.some((id) => !batch.items.some((item) => item.packageId === id))) throw { code: "package_candidate_evidence_stale" };
                candidates.catalogComplete &&= batch.catalogComplete;
                lockedRows.push(...batch.items);
            }
            const lockedById = new Map(lockedRows.map((row) => [row.packageId, row]));
            candidates.items = candidates.items.map((row) => lockedById.get(row.packageId) ?? row);
        }
    } catch (caught) { candidates = undefined; candidateError = safeError(caught).code; }
    const catalog: WorkspaceCatalogVersion[] = [];
    // One existing bounded page per source supplies optional names/links only.
    // Never drain catalog versions or use their order/classification as evidence.
    if (repositories.length > 4096) throw { code: "package_candidate_limit_exceeded" };
    for (const repository of repositories) {
        if (!repository.repositoryId || settings.settings.packages.hiddenRepositoryIds.includes(repository.repositoryId)) continue;
        let page: Awaited<ReturnType<GuiRpcClient["repositoryPackages"]>>;
        try { page = await client.repositoryPackages(repository.repositoryId); }
        catch { continue; }
        for (const item of page.packages) catalog.push({ ...item, repositoryId: repository.repositoryId, source: repository.name ?? repository.declaredId ?? sourceText(repository), sourceKey: `repository:${repository.repositoryId}`, sourceKind: repository.source.kind, sourceSelector: { kind: "repository", repositoryId: repository.repositoryId } });
    }
    if (!settings.settings.packages.hideLocalUserPackages) for (const item of userPackages) catalog.push({ packageId: item.packageId, version: item.version, ...(item.displayName ? { displayName: item.displayName } : {}), yanked: false, source: item.displayName ?? "Local / User Package", sourceKey: `user-package:${item.userPackageId}`, sourceKind: "user-package", sourceSelector: { kind: "user_package", userPackageId: item.userPackageId } });
    for (const row of candidates?.items ?? []) {
        const choices = [row.latest, row.latestStable, row.projectLatest, row.projectLatestStable];
        for (const choice of choices) {
            if (choice.kind !== "candidate") continue;
            const value = choice.candidate;
            const sourceKey = candidateSourceKey(value.source);
            if (catalog.some((entry) => entry.packageId === row.packageId && entry.sourceKey === sourceKey && entry.version === value.version)) continue;
            const source = value.source;
            const repository = source.kind === "repository" ? repositories.find((entry) => entry.repositoryId === source.repository_id) : undefined;
            const local = source.kind === "user_package" ? userPackages.find((entry) => entry.userPackageId === source.user_package_id) : undefined;
            catalog.push({ packageId: row.packageId, version: value.version, yanked: false, prerelease: value.classification === "prerelease", ...(local?.displayName ? { displayName: local.displayName } : {}), ...(repository?.repositoryId ? { repositoryId: repository.repositoryId } : {}), source: repository?.name ?? repository?.declaredId ?? local?.displayName ?? sourceKey, sourceKey, sourceKind: repository?.source.kind ?? "user-package", sourceSelector: candidateSource(value.source) });
        }
    }
    const sourceRegistry: PackageWorkspaceValue["sourceRegistry"] = {};
    for (const repository of repositories) if (repository.repositoryId) sourceRegistry[`repository:${repository.repositoryId}`] = { label: repository.name ?? repository.declaredId ?? sourceText(repository), kind: repository.source.kind };
    for (const local of userPackages) sourceRegistry[`user-package:${local.userPackageId}`] = { label: local.displayName ?? "Local / User Package", kind: "user-package" };
    return { project, catalog, installations, launchOptions, repositories, settings, sourceRegistry, sourceSelectionKey: JSON.stringify(sourceSelections), ...(candidates ? { candidates } : {}), ...(candidateError ? { candidateError } : {}) };
}

function ProjectPackageWorkspace({ client, navigate, projectId }: PageProps & { projectId: string }) {
    const canReadBackups = useCapability(capabilities.backupsRead);
    const canManageRepositories = useCapability(capabilities.repositoriesRegistry);
    const canPlanPackagesV1 = useCapability(capabilities.packagesPlanV1);
    const canPlanPackagesV2 = useCapability(capabilities.packagesPlanV2);
    const canUseUserPackages = useCapability(capabilities.packagesUserPackages);
    const canQueryCandidates = useCapability(capabilities.packagesCandidates);
    const [search, setSearch] = useState("");
    const [filter, setFilter] = useState("all");
    const [sourceFilter, setSourceFilter] = useState<"all" | "local" | "remote" | "user-package">("all");
    const [sourceSelections, setSourceSelections] = useState<Record<string, string>>({});
    const load = useCallback(() => loadPackageWorkspace(client, projectId, canUseUserPackages, canQueryCandidates, sourceSelections), [canUseUserPackages, canQueryCandidates, client, projectId, sourceSelections]);
    const [evidenceInvalid, setEvidenceInvalid] = useState(false);
    const [packageMutationBusy, setPackageMutationBusy] = useState(false);
    useEffect(() => setEvidenceInvalid(false), [load]);
    const [bulkSelection, setBulkSelection] = useState<string[]>([]);
    const [repositoryRefresh, setRepositoryRefresh] = useState<RepositoryRefreshProgress>();
    const [selection, setSelection] = useState<PackageActionSelection>();
    const [workspaceFeedback, setWorkspaceFeedback] = useState<string>();
    const selectedSource = (row: PackageWorkspaceRow): PackageSourceSelector | undefined => {
        const selectedKey = sourceSelections[row.packageId];
        if (selectedKey?.startsWith("repository:")) return { kind: "repository", repositoryId: selectedKey.slice(11) };
        if (selectedKey?.startsWith("user-package:")) return { kind: "user_package", userPackageId: selectedKey.slice(13) };
        return undefined;
    };
    const selectAction = (action: PackageActionSelection["action"], row?: PackageWorkspaceRow, version?: string) => {
        const packageId = row?.packageId ?? "";
        const source = row === undefined ? undefined : selectedSource(row);
        setSelection({ action, key: (selection?.key ?? 0) + 1, packageId, ...(version === undefined ? {} : { version }), ...(source === undefined ? {} : { source }) });
    };
    const selectBulkInstalled = (rows: PackageWorkspaceRow[], action: "bulk-reinstall" | "bulk-remove") => {
        const selectedRows = rows.filter((row) => bulkSelection.includes(row.packageId) && (action === "bulk-remove" ? row.evidence?.direct === true : row.evidence?.installed.kind === "locked"));
        const sources = selectedRows.flatMap((row) => {
            const source = selectedSource(row);
            return source === undefined ? [] : [{ packageId: row.packageId, source }];
        });
        if (selectedRows.length === 0 || selectedRows.length !== bulkSelection.length || selectedRows.length > 256) return;
        setSelection({ action, key: (selection?.key ?? 0) + 1, packageId: "", packageIds: selectedRows.map((row) => row.packageId), sources });
    };
    const refreshRepositories = async (reload: () => void) => {
        setRepositoryRefresh({ completed: 0, failures: [], results: [], running: true, successes: 0, total: 0 });
        let repositories: RepositorySnapshot[];
        try {
            repositories = await listAllRepositories(client);
        } catch (caught: unknown) {
            const error = safeError(caught);
            setRepositoryRefresh({ completed: 1, failures: [{ code: error.code, repositoryId: "catalog" }], results: [{ code: error.code, repositoryId: "catalog", status: "failure" }], running: false, successes: 0, total: 1 });
            return;
        }
        setRepositoryRefresh({ completed: 0, failures: [], results: [], running: true, successes: 0, total: repositories.length });
        let successes = 0;
        const failures: RepositoryRefreshProgress["failures"] = [];
        const results: RepositoryRefreshProgress["results"] = [];
        for (const repository of repositories) {
            const repositoryId = repository.repositoryId;
            const revision = repository.revision;
            if (repositoryId === undefined || revision === undefined) {
                failures.push({ code: "invalid_repository_snapshot", repositoryId: repositoryId ?? "unknown" });
                results.push({ code: "invalid_repository_snapshot", repositoryId: repositoryId ?? "unknown", status: "failure" });
            } else {
                try {
                    await client.repositoryRefresh(repositoryId, revision);
                    successes += 1;
                    results.push({ repositoryId, status: "success" });
                } catch (caught: unknown) {
                    const error = safeError(caught);
                    if (error.code === "revision_conflict") {
                        try {
                            const current = await client.repositoryGet(repositoryId);
                            if (current.repository.revision === undefined) throw { code: "invalid_repository_snapshot" };
                            await client.repositoryRefresh(repositoryId, current.repository.revision);
                            successes += 1;
                            results.push({ repositoryId, status: "success" });
                        } catch (retryCaught: unknown) {
                            const retryError = safeError(retryCaught);
                            failures.push({ code: retryError.code, repositoryId });
                            results.push({ code: retryError.code, repositoryId, status: "failure" });
                        }
                    } else {
                        failures.push({ code: error.code, repositoryId });
                        results.push({ code: error.code, repositoryId, status: "failure" });
                    }
                }
            }
            setRepositoryRefresh({ completed: results.length, failures: [...failures], results: [...results], running: true, successes, total: repositories.length });
        }
        setRepositoryRefresh({ completed: repositories.length, failures, results, running: false, successes, total: repositories.length });
        reload();
    };
    return (
        <ResourcePage load={load} showRefreshBar={false}>{({ project, catalog, installations, launchOptions, repositories, settings, candidates, candidateError, sourceRegistry, sourceSelectionKey }, refresh, refreshing, refreshError) => {
            const workspaceRows = packageWorkspaceRows(project, catalog, candidates?.items, sourceRegistry);
            const currentEvidence = candidates !== undefined && sourceSelectionKey === JSON.stringify(sourceSelections) && !evidenceInvalid && !refreshing && refreshError === undefined;
            const catalogReady = currentEvidence && candidates.catalogComplete && !packageMutationBusy;
            const reloadEvidence = () => { setEvidenceInvalid(false); refresh(); };
            const chooseCandidate = (row: PackageWorkspaceRow, option: PackageCandidateEvidence) => {
                if (!catalogReady || !canPlanPackagesV1 || !canPlanPackagesV2 || !actionableVersion(option)) return;
                setSourceSelections((current) => ({ ...current, [row.packageId]: candidateSourceKey(option.source) }));
                setSelection({ action: option.relation === "older" ? "downgrade" : option.relation === "newer" ? "upgrade" : "install", key: (selection?.key ?? 0) + 1, packageId: row.packageId, version: option.relation === "older" ? option.version : `=${option.version}`, source: candidateSource(option.source), includePrerelease: option.classification === "prerelease", candidateSnapshot: candidates.snapshot.fingerprint });
            };
            const selectedRows = workspaceRows.filter((row) => bulkSelection.includes(row.packageId));
            const completeSelection = selectedRows.length > 0 && selectedRows.length === bulkSelection.length && selectedRows.length <= 256;
            const canBulkInstalled = catalogReady && completeSelection && selectedRows.every((row) => row.evidence?.installed.kind === "locked");
            const canBulkRemove = catalogReady && completeSelection && selectedRows.every((row) => row.evidence?.direct === true);
            const selectedIntents = (stable: boolean) => selectedRows.flatMap((row) => row.evidence ? [intentForSummary(row.evidence, stable)].filter((intent) => intent !== undefined) : []);
            const selectedLatest = selectedIntents(false);
            const selectedStable = selectedIntents(true);
            const canSelected = catalogReady && completeSelection && selectedLatest.length === selectedRows.length;
            const canSelectedStable = catalogReady && completeSelection && selectedStable.length === selectedRows.length;
            const stableSelectionDiffers = JSON.stringify(selectedLatest) !== JSON.stringify(selectedStable);
            const lockedRows = workspaceRows.filter((row) => project.lockedDependencies.some((locked) => locked.packageId === row.packageId));
            const allUpdate = (stable: boolean) => {
                const intents = lockedRows.flatMap((row) => row.evidence ? [intentForSummary(row.evidence, stable)].filter((intent) => intent?.kind === "upgrade") : []);
                const blocked = lockedRows.some((row) => !row.evidence || uncertainUpdate(row.evidence, stable));
                const exclusions = lockedRows.flatMap((row) => {
                    const update = stable ? row.evidence?.stableUpdate : row.evidence?.update;
                    return update && update.kind !== "target" ? [`${row.displayName}: ${updateReason(update)}`] : [];
                });
                return { intents, exclusions, enabled: catalogReady && !blocked && intents.length > 0 && intents.length <= 256 };
            };
            const allLatest = allUpdate(false);
            const allStable = allUpdate(true);
            const submitBulk = (intents: import("./core-models").PackageBulkIntent[], exclusions: string[] = []) => {
                if (!catalogReady || !intents.length || intents.length > 256) return;
                setSelection({ action: "bulk-candidates", key: (selection?.key ?? 0) + 1, packageId: "", intents, exclusions, candidateSnapshot: candidates.snapshot.fingerprint });
            };
            const rows = workspaceRows.filter((row) => {
                const query = search.toLocaleLowerCase();
                const matchesSearch = query.length === 0 || [row.displayName, row.packageId, ...row.sources].some((value) => value.toLocaleLowerCase().includes(query));
                const matchesFilter = filter === "all"
                    || (filter === "installed" && row.installedVersion !== undefined)
                    || (filter === "available" && row.installedVersion === undefined && row.availableVersions.length > 0)
                    || (filter === "missing" && row.status === "missing-source");
                const matchesSource = sourceFilter === "all" || row.sourceKinds.includes(sourceFilter);
                return matchesSearch && matchesFilter && matchesSource;
            });
            return (
                <section className="project-workspace">
                    <header className="project-workspace-header">
                        <div className="project-workspace-context">
                            <Button className="project-back-action" onClick={() => navigate("/projects")} type="button" variant="text"><Icon asset={arrowBackIcon} slot="icon" />Back</Button>
                            <div className="project-workspace-title">
                                <h1 id="route-title" tabIndex={-1}>{projectName(project)}</h1>
                                <p title={displayProjectPath(project.rootPath)}>{displayProjectPath(project.rootPath)}</p>
                            </div>
                        </div>
                        <nav aria-label="Project actions" className="project-workspace-action-cluster">
                            {refreshError === undefined ? null : <span className="inline-error" role="alert">Refresh failed: {refreshError.code}</span>}
                            <ProjectUnityWorkspaceActions client={client} installations={installations} launchOptions={launchOptions} navigate={navigate} project={project} />
                            <Button disabled={!canReadBackups} onClick={() => navigate(`/projects/${projectId}/backups`)} title={capabilityUnavailableTitle(canReadBackups, capabilities.backupsRead)} type="button" variant="text"><Icon asset={backupIcon} slot="icon" />Backups</Button>
                            <ProjectRowActions
                                client={client}
                                context="workspace"
                                navigate={navigate}
                                onChanged={refresh}
                                onCopyCompleted={(targetProjectId) => navigate(`/projects/${targetProjectId}`)}
                                onFeedback={setWorkspaceFeedback}
                                onProjectChanged={() => refresh()}
                                onRemoved={() => navigate("/projects")}
                                project={project}
                            />
                        </nav>
                    </header>
                    {workspaceFeedback === undefined ? null : <div className="project-workspace-feedback" role="status" aria-live="polite">{workspaceFeedback}</div>}
                    <section aria-labelledby="packages-heading" className="package-workspace-surface">
                        <header className="package-workspace-toolbar">
                            <h2 id="packages-heading">Manage packages</h2>
                            <IconButton className="package-refresh-action" disabled={!canManageRepositories || refreshing || repositoryRefresh?.running === true} label={repositoryRefresh?.running === true ? "Refreshing packages" : "Refresh"} onClick={() => void refreshRepositories(refresh)} title={capabilityUnavailableTitle(canManageRepositories, capabilities.repositoriesRegistry) ?? "Refresh packages"} type="button"><Icon asset={refreshIcon} /></IconButton>
                            <SearchField className="package-workspace-search" label="Search packages" onInput={setSearch} placeholder="Search..." value={search} />
                            <div className="package-workspace-secondary-actions">
                                <span className="visually-hidden" role="status" aria-live="polite">{rows.length} {rows.length === 1 ? "package" : "packages"}</span>
                                <PackageWorkspaceMoreMenu canPlanV1={canPlanPackagesV1 && catalogReady} canPlanV2={canPlanPackagesV2 && catalogReady} onResolve={() => selectAction("resolve")} onReinstallAll={() => selectAction("reinstall-all")} />
                                <PackageFilters canUseUserPackages={canUseUserPackages} client={client} filter={filter} onChanged={refresh} repositories={repositories} setFilter={setFilter} setSourceFilter={setSourceFilter} settings={settings} sourceFilter={sourceFilter} />
                            </div>
                        </header>
                        {candidateError || evidenceInvalid || !currentEvidence ? <div role="status">{candidateError ?? (evidenceInvalid ? "Candidate evidence is stale." : "Loading candidate evidence…")} <Button onClick={reloadEvidence} type="button" variant="text">Reload package evidence</Button></div> : null}
                        {currentEvidence && !candidates.catalogComplete ? <p role="status">Package catalog is incomplete. Refresh the affected sources before planning changes.</p> : null}
                        {currentEvidence ? <div className="package-workspace-secondary-actions">
                            <Button disabled={!canPlanPackagesV2 || !allLatest.enabled} onClick={() => submitBulk(allLatest.intents, allLatest.exclusions)} type="button" variant="tonal">Update All ({allLatest.intents.length})</Button>
                            {JSON.stringify(allLatest.intents) !== JSON.stringify(allStable.intents) ? <Button disabled={!canPlanPackagesV2 || !allStable.enabled} onClick={() => submitBulk(allStable.intents, allStable.exclusions)} type="button" variant="text">Update All Stable ({allStable.intents.length})</Button> : null}
                            {!allLatest.enabled ? <span role="status">{allLatest.intents.length > 256 ? "More than 256 updates; explicitly select a smaller batch." : lockedRows.some((row) => !row.evidence || uncertainUpdate(row.evidence, false)) ? "Resolve unknown or ambiguous package evidence before Update All." : "No actionable updates."}</span> : null}
                        </div> : null}
                        {repositoryRefresh === undefined ? null : (
                            <div className={repositoryRefresh.failures.length > 0 ? "package-refresh-status package-refresh-status--failed" : "package-refresh-status"} role={repositoryRefresh.failures.length > 0 ? "alert" : "status"} aria-live="polite">
                                <span>{repositoryRefresh.running
                                    ? `Refreshing repositories: ${repositoryRefresh.completed} of ${repositoryRefresh.total}.`
                                    : `${repositoryRefresh.successes} refreshed, ${repositoryRefresh.failures.length} failed.`}</span>
                                <ul className="visually-hidden">{repositoryRefresh.results.map((result) => <li key={result.repositoryId}>{result.repositoryId}: {result.status}{result.code === undefined ? "" : ` (${result.code})`}</li>)}</ul>
                            </div>
                        )}
                        {bulkSelection.length === 0 ? null : (
                            <div aria-label="Selected package actions" className="package-bulk-bar" role="region">
                                <strong>{bulkSelection.length} selected</strong>
                                <Button disabled={!canPlanPackagesV2 || !canSelected} onClick={() => submitBulk(selectedLatest)} type="button" variant="tonal">Install / Update selected</Button>
                                {stableSelectionDiffers ? <Button disabled={!canPlanPackagesV2 || !canSelectedStable} onClick={() => submitBulk(selectedStable)} type="button" variant="text">Install / Update selected stable</Button> : null}
                                <Button disabled={!canPlanPackagesV2 || !canBulkInstalled} onClick={() => selectBulkInstalled(workspaceRows, "bulk-reinstall")} title={capabilityUnavailableTitle(canPlanPackagesV2, capabilities.packagesPlanV2)} type="button" variant="tonal">Reinstall selected</Button>
                                <Button disabled={!canPlanPackagesV2 || !canBulkRemove} onClick={() => selectBulkInstalled(workspaceRows, "bulk-remove")} title={capabilityUnavailableTitle(canPlanPackagesV2, capabilities.packagesPlanV2)} type="button" variant="text">Remove selected</Button>
                                {!canSelected || !canBulkInstalled || !canBulkRemove ? <span role="status">Actions require every selected package, including hidden rows: install/update needs eligible targets; reinstall needs locks; remove needs direct requirements. Limits: 1–256 packages.</span> : null}
                                <Button onClick={() => setBulkSelection([])} type="button" variant="text">Clear selection</Button>
                            </div>
                        )}
                        <div className="package-workspace-table-scroll">
                            {rows.length === 0 ? <section className="projects-empty" role="status"><h3>{workspaceRows.length === 0 ? "No packages" : "No matching packages"}</h3><p>{workspaceRows.length === 0 ? "This project has no packages to manage." : "Change the search or package filter."}</p></section> : (
                                <MaterialDataTable className="package-workspace-table" label="Packages" minWidth={840}>
                                    <colgroup><col className="package-column-select" /><col className="package-column-name" /><col className="package-column-installed" /><col className="package-column-latest" /><col className="package-column-source" /><col className="package-column-actions" /></colgroup>
                                    <thead><tr><DataTableHeader><Checkbox checked={rows.length > 0 && rows.every((row) => bulkSelection.includes(row.packageId))} label="Select all visible packages" onChange={(checked) => setBulkSelection((current) => checked ? [...new Set([...current, ...rows.map((row) => row.packageId)])] : current.filter((id) => !rows.some((row) => row.packageId === id)))} /></DataTableHeader><DataTableHeader>Package</DataTableHeader><DataTableHeader>Installed</DataTableHeader><DataTableHeader>Latest</DataTableHeader><DataTableHeader>Source</DataTableHeader><DataTableHeader><span className="visually-hidden">Actions</span></DataTableHeader></tr></thead>
                                    <tbody>{rows.map((row) => {
                                        const evidence = currentEvidence ? row.evidence : undefined;
                                        const installChoice = evidence?.projectLatest;
                                        const update = evidence?.update;
                                        return (
                                            <tr key={row.packageId}>
                                                <td><Checkbox checked={bulkSelection.includes(row.packageId)} label={`Select ${row.displayName}`} onChange={(checked) => setBulkSelection((current) => checked ? [...new Set([...current, row.packageId])] : current.filter((packageId) => packageId !== row.packageId))} /></td>
                                                <td><strong>{row.displayName}</strong><small>{row.packageId}</small></td>
                                                <td><PackageVersionMenu client={client} project={project} snapshot={currentEvidence ? candidates.snapshot.fingerprint : undefined} disabled={!canQueryCandidates} canPlan={catalogReady && canPlanPackagesV1 && canPlanPackagesV2} onStale={() => setEvidenceInvalid(true)} onSelect={(option) => chooseCandidate(row, option)} row={row} />{row.requestedRange === undefined ? null : <small>Requested {row.requestedRange}</small>}</td>
                                                <td>{evidence ? choiceText(evidence.latest) : "Evidence unavailable"}{evidence && update?.kind !== "target" ? <small>{update ? updateReason(update) : ""}</small> : null}</td>
                                                <td>{row.sourceOptions.length === 1 && !sourceSelections[row.packageId] ? row.sourceOptions[0]?.label : <PackageSourceMenu disabled={!currentEvidence} label={`Source for ${row.displayName}`} onChange={(key) => setSourceSelections((current) => ({ ...current, [row.packageId]: key }))} options={row.sourceOptions} value={sourceSelections[row.packageId] ?? ""} />}</td>
                                                <td><div className="package-row-actions">
                                                    {row.installedVersion === undefined
                                                        ? <Button disabled={!canPlanPackagesV1 || !canPlanPackagesV2 || !catalogReady || installChoice?.kind !== "candidate" || installChoice.candidate.eligibility !== "eligible"} onClick={() => { if (installChoice?.kind === "candidate") chooseCandidate(row, installChoice.candidate); }} title={capabilityUnavailableTitle(canPlanPackagesV1 && canPlanPackagesV2, capabilities.packagesPlanV2)} type="button" variant="tonal"><Icon asset={downloadIcon} slot="icon" />Install</Button>
                                                        : update?.kind === "target"
                                                            ? <Button disabled={!canPlanPackagesV1 || !canPlanPackagesV2 || !catalogReady} onClick={() => chooseCandidate(row, update.candidate)} title={capabilityUnavailableTitle(canPlanPackagesV1 && canPlanPackagesV2, capabilities.packagesPlanV2)} type="button" variant="tonal"><Icon asset={upgradeIcon} slot="icon" />Update</Button>
                                                            : null}
                                                    <PackageRowMoreMenu canPlanV1={canPlanPackagesV1 && !packageMutationBusy && !refreshing && !evidenceInvalid} canPlanV2={canPlanPackagesV2 && catalogReady} client={client} onAction={(action) => selectAction(action, row)} row={row} />
                                                </div></td>
                                            </tr>
                                        );
                                    })}</tbody>
                                </MaterialDataTable>
                            )}
                        </div>
                    </section>
                    <PackageActions client={client} onChanged={reloadEvidence} onBusyChanged={setPackageMutationBusy} onEvidenceStale={() => setEvidenceInvalid(true)} project={project} selection={selection === undefined ? undefined : { ...selection, candidateSnapshot: selection.candidateSnapshot ?? candidates?.snapshot.fingerprint }} />
                </section>
            );
        }}</ResourcePage>
    );
}

function packageWorkspaceRows(project: ProjectSnapshot, catalog: WorkspaceCatalogVersion[], summaries: PackageCandidateSummary[] = [], sourceRegistry: PackageWorkspaceValue["sourceRegistry"] = {}): PackageWorkspaceRow[] {
    const catalogByPackage = new Map<string, WorkspaceCatalogVersion[]>();
    for (const item of catalog) {
        const versions = catalogByPackage.get(item.packageId) ?? [];
        versions.push(item);
        catalogByPackage.set(item.packageId, versions);
    }
    const installed = new Map(project.lockedDependencies.map((item) => [item.packageId, item.value]));
    const requested = new Map(project.directDependencies.map((item) => [item.packageId, item.value]));
    const packageIds = new Set([...installed.keys(), ...requested.keys(), ...catalogByPackage.keys(), ...summaries.map((item) => item.packageId)]);
    return [...packageIds].sort((left, right) => left.localeCompare(right)).map((packageId) => {
        const versions = catalogByPackage.get(packageId) ?? [];
        const availableVersions = [...new Set(versions.filter((item) => !item.yanked).map((item) => item.version))];
        const installedVersion = installed.get(packageId);
        const evidence = summaries.find((item) => item.packageId === packageId);
        const sourceOptions = (evidence?.providers ?? []).map((provider) => { const key = candidateSourceKey(provider.source); return { key, label: sourceRegistry[key]?.label ?? key, selector: candidateSource(provider.source) }; });
        const sources = sourceOptions.map((source) => source.label);
        const sourceKinds = [...new Set((evidence?.providers ?? []).flatMap((provider) => { const key = candidateSourceKey(provider.source); const kind = sourceRegistry[key]?.kind; return kind ? [kind] : provider.source.kind === "user_package" ? ["user-package" as const] : []; }))];
        const preferredVersion = installedVersion;
        const linkVersion = versions
            .filter((item): item is WorkspaceCatalogVersion & { repositoryId: string } => item.version === preferredVersion && item.links !== undefined && item.repositoryId !== undefined)
            .sort((left, right) => left.repositoryId.localeCompare(right.repositoryId))[0];
        return {
            availableVersions,
            ...(evidence ? { evidence } : {}),
            displayName: versions.find((item) => item.displayName !== undefined)?.displayName ?? packageId,
            ...(installedVersion === undefined ? {} : { installedVersion }),
            ...(linkVersion === undefined ? {} : {
                linkTarget: {
                    changelog: linkVersion.links?.changelog !== undefined,
                    documentation: linkVersion.links?.documentation !== undefined,
                    repositoryId: linkVersion.repositoryId,
                    version: linkVersion.version
                }
            }),
            packageId,
            ...(requested.has(packageId) ? { requestedRange: requested.get(packageId) } : {}),
            sourceKinds,
            sourceOptions,
            sources,
            status: installedVersion !== undefined ? "installed" : availableVersions.length > 0 ? "available" : "missing-source"
        };
    });
}

export function RepositoriesPage({ client, navigate }: PageProps) {
    const load = useCallback(() => listAllRepositories(client), [client]);
    const [selected, setSelected] = useState<RepositorySnapshot>();
    return <UtilityWorkspace title="Repositories" load={load} navigation={<ResourceNavigation current="/repositories" navigate={navigate} />} action={{ label: "Add repository", render: (_, refresh) => <RegisterRepositoryPanel client={client} onChanged={refresh} /> }}>
        {(repositories, refresh) => <>
            {repositories.length === 0 ? <RouteState kind="empty" title="No repositories registered" /> : <MaterialDataTable label="Repositories" minWidth={640}>
                <thead><tr>{["Repository", "Source", "Actions"].map((label) => <DataTableHeader key={label}>{label}</DataTableHeader>)}</tr></thead>
                <tbody>{repositories.map((repository) => <tr key={repository.repositoryId ?? sourceText(repository)}>
                    <td><strong>{repository.name ?? repository.declaredId ?? "Repository"}</strong></td>
                    <td title={sourceText(repository)}>{sourceText(repository)}</td>
                    <td><div className="card-actions"><Button disabled={repository.repositoryId === undefined} onClick={() => setSelected(repository)} type="button" variant="text">Browse packages</Button><RepositoryActions client={client} onChanged={refresh} repository={repository} /></div></td>
                </tr>)}</tbody>
            </MaterialDataTable>}
            {selected?.repositoryId === undefined ? null : <Dialog wide open title={selected.name ?? "Repository packages"} onClose={() => setSelected(undefined)}>
                <RepositoryPackageList client={client} repositoryId={selected.repositoryId} />
                <div className="dialog-actions"><Button onClick={() => setSelected(undefined)} type="button" variant="text">Close</Button></div>
            </Dialog>}
        </>}
    </UtilityWorkspace>;
}

function RepositoryPackageList({ client, repositoryId }: { client: GuiRpcClient; repositoryId: string }) {
    const load = useCallback(() => client.repositoryPackages(repositoryId), [client, repositoryId]);
    return <ResourcePage load={load} showRefreshBar={false}>{(catalog) => <PagedItems initialItems={catalog.packages} initialCursor={catalog.nextCursor} loadMore={async (cursor) => { const next = await client.repositoryPackages(repositoryId, cursor); return { items: next.packages, nextCursor: next.nextCursor }; }}>
        {(items) => <DataTable headers={["Package", "Version", "Status"]} rows={items.map((item) => [item.displayName ?? item.packageId, item.version, item.yanked ? "Yanked" : "Available"])} />}
    </PagedItems>}</ResourcePage>;
}

function ResourceNavigation({ current, navigate }: { current: string; navigate(path: string): void }) {
    const canUseUserPackages = useCapability(capabilities.packagesUserPackages);
    return <nav aria-label="Resources" className="resource-navigation">{[{ path: "/repositories", label: "Repositories" }, { path: "/user-packages", label: "User Packages" }, { path: "/templates", label: "Templates" }].map(({ path, label }) => <Button aria-current={current === path ? "page" : undefined} disabled={path === "/user-packages" && !canUseUserPackages} key={path} onClick={() => navigate(path)} title={path === "/user-packages" ? capabilityUnavailableTitle(canUseUserPackages, capabilities.packagesUserPackages) : undefined} type="button" variant={current === path ? "tonal" : "text"}>{label}</Button>)}</nav>;
}

function UtilityNavigation({ current, kind, navigate }: { current: string; kind: "settings" | "logs"; navigate(path: string): void }) {
    const unity = useCapability(capabilities.unityRead);
    const items = kind === "settings" ? [{ path: "/settings", label: "Preferences" }, { path: "/unity", label: "Unity installations" }] : [{ path: "/activity", label: "Activity" }, { path: "/diagnostics", label: "Diagnostics" }];
    return <nav aria-label={kind === "settings" ? "Settings sections" : "Logs"} className="resource-navigation">{items.map(({ path, label }) => <Button aria-current={current === path ? "page" : undefined} disabled={path === "/unity" && !unity} key={path} onClick={() => navigate(path)} type="button" variant={current === path ? "tonal" : "text"}>{label}</Button>)}</nav>;
}

function ActionDisclosure({ children, title }: { children: ReactNode; title: string }) {
    const [open, setOpen] = useState(false);
    return <section className="utility-disclosure"><Button aria-haspopup="dialog" onClick={() => setOpen(true)} type="button" variant="tonal">{title}</Button>{open ? <UtilityActionDialog title={title} onClose={() => setOpen(false)}>{children}</UtilityActionDialog> : null}</section>;
}

function BackTo({ label, navigate, path }: { label: string; navigate(path: string): void; path: string }) {
    return <Button onClick={() => navigate(path)} type="button" variant="text"><Icon asset={arrowBackIcon} size={24} />{label}</Button>;
}

async function listAllUserPackages(client: GuiRpcClient): Promise<UserPackageRecord[]> {
    const result: UserPackageRecord[] = [];
    let cursor: { updatedAtMs: number; userPackageId: string } | undefined;
    do {
        const page = await client.userPackagesList(cursor);
        result.push(...page.userPackages);
        cursor = page.nextCursor;
    } while (cursor !== undefined);
    return result;
}

export function UserPackagesPage({ client, navigate }: PageProps) {
    const capabilityState = useCapabilityState(capabilities.packagesUserPackages);
    const load = useCallback(() => listAllUserPackages(client), [client]);
    if (capabilityState === "checking") return <Page title="User Packages" eyebrow="Local package sources"><ResourceNavigation current="/user-packages" navigate={navigate} /><RouteState kind="loading" title="Checking capability" /></Page>;
    if (capabilityState === "unavailable") return <Page title="User Packages" eyebrow="Local package sources"><ResourceNavigation current="/user-packages" navigate={navigate} /><RouteState kind="error" title="Feature unavailable" detail={`${capabilities.packagesUserPackages} was not negotiated by the connected daemon.`} /></Page>;
    return <UtilityWorkspace title="User Packages" load={load} navigation={<ResourceNavigation current="/user-packages" navigate={navigate} />} tools={(_, refresh) => <UserPackageEnroll client={client} onChanged={refresh} />}>{(packages, refresh) => <UserPackageManager client={client} onChanged={refresh} packages={packages} />}</UtilityWorkspace>;
}

function UserPackageEnroll({ client, onChanged }: { client: GuiRpcClient; onChanged(): void }) {
    const [busy, setBusy] = useState<string>();
    const [error, setError] = useState<RpcError>();
    const enroll = async () => {
        setBusy("enroll");
        setError(undefined);
        try {
            const sourcePath = await client.selectDirectory();
            if (sourcePath !== undefined) {
                await client.userPackageEnroll(sourcePath);
                onChanged();
            }
        } catch (caught: unknown) {
            setError(safeError(caught));
        } finally {
            setBusy(undefined);
        }
    };
    return <><Button widthLabels={["Enrolling…", "Enroll folder"]} disabled={busy !== undefined} onClick={() => void enroll()} type="button">{busy === "enroll" ? "Enrolling…" : "Enroll folder"}</Button>{error === undefined ? null : <span role="alert">Enrollment failed: {error.code}</span>}</>;
}

function UserPackageManager({ client, onChanged, packages }: { client: GuiRpcClient; onChanged(): void; packages: UserPackageRecord[] }) {
    const [busy, setBusy] = useState<string>();
    const pending = useRef(false);
    const [removing, setRemoving] = useState<UserPackageRecord>();
    const [error, setError] = useState<RpcError>();
    const refresh = async (item: UserPackageRecord) => {
        if (pending.current) return;
        pending.current = true;
        setBusy(`refresh:${item.userPackageId}`);
        setError(undefined);
        try {
            await client.userPackageRefresh(item.userPackageId, item.revision);
            onChanged();
        } catch (caught: unknown) {
            setError(safeError(caught));
        } finally {
            setBusy(undefined);
            pending.current = false;
        }
    };
    const remove = async (item: UserPackageRecord) => {
        if (pending.current) return;
        pending.current = true;
        setBusy(`remove:${item.userPackageId}`);
        setError(undefined);
        try {
            await client.userPackageRemove(item.userPackageId, item.revision);
            setRemoving(undefined);
            onChanged();
        } catch (caught: unknown) {
            setError(safeError(caught));
        } finally {
            setBusy(undefined);
            pending.current = false;
        }
    };
    return <section className="user-packages">{error === undefined ? null : <p className="inline-error" role="alert">User Package request failed: {error.code}</p>}{packages.length === 0 ? <RouteState kind="empty" title="No User Packages enrolled" detail="Enroll a package folder to make it available as a source. Removing enrollment never deletes the source folder." /> : <UtilityTable label="User Packages" headers={["Package", "Version", "Source", "Actions"]} rows={packages.map((item) => ({ key: item.userPackageId, cells: [<><strong>{item.displayName ?? item.packageId}</strong><small>{item.packageId}</small></>, item.version, <span title={item.userPackageId}>Local / User Package<small>revision {item.revision}</small></span>, <div className="card-actions"><Button widthLabels={["Refreshing…", "Refresh"]} disabled={busy !== undefined} onClick={() => void refresh(item)} type="button" variant="text">{busy === "refresh:" + item.userPackageId ? "Refreshing…" : "Refresh"}</Button><Button widthLabels={["Removing…", "Remove enrollment"]} disabled={busy !== undefined} onClick={() => setRemoving(item)} type="button" variant="text">{busy === "remove:" + item.userPackageId ? "Removing…" : "Remove enrollment"}</Button></div>] }))} />}{removing === undefined ? null : <Dialog open dismissible={busy === undefined} title="Remove User Package?" onClose={() => { if (busy === undefined) setRemoving(undefined); }}>
        <p>Remove <strong>{removing.displayName ?? removing.packageId}</strong> ({removing.version}) from your package sources?</p><p>The source folder will stay on disk.</p>
        {error === undefined ? null : <p role="alert">Removal failed: {error.code}</p>}
        <div className="dialog-actions"><Button disabled={busy !== undefined} onClick={() => setRemoving(undefined)} type="button" variant="text">Cancel</Button><Button disabled={busy !== undefined} onClick={() => void remove(removing)} type="button">Remove enrollment</Button></div>
    </Dialog>}</section>;
}

export function RepositoryDetailPage({ client, navigate, repositoryId }: PageProps & { repositoryId: string }) {
    const load = useCallback(async () => Promise.all([client.repositoryGet(repositoryId), client.repositoryPackages(repositoryId)]), [client, repositoryId]);
    return <UtilityWorkspace title="Repository" back={<BackTo label="Repositories" navigate={navigate} path="/repositories" />} load={load}>{([detail, catalog], refresh) => <><dl className="detail-grid"><Detail label="Name" value={detail.repository.name ?? "Unnamed"} /><Detail label="Source" value={sourceText(detail.repository)} /><Detail label="Revision" value={String(detail.repository.revision ?? "—")} /><Detail label="Issues" value={String(detail.repository.issues.length)} /></dl><h2>Packages</h2><PagedItems initialItems={catalog.packages} initialCursor={catalog.nextCursor} loadMore={async (cursor) => { const next = await client.repositoryPackages(repositoryId, cursor); return { items: next.packages, nextCursor: next.nextCursor }; }}>{(items) => <DataTable headers={["Package", "Version", "Status"]} rows={items.map((item) => [item.displayName ?? item.packageId, item.version, item.yanked ? "Yanked" : "Available"])} />}</PagedItems><ActionDisclosure title="Manage repository"><RepositoryActions client={client} onChanged={refresh} repository={detail.repository} /></ActionDisclosure></>}</UtilityWorkspace>;
}

export function TemplatesPage({ client, navigate }: PageProps) {
    const load = useCallback(() => client.templatesList(), [client]);
    return <UtilityWorkspace title="Templates" load={load} navigation={<ResourceNavigation current="/templates" navigate={navigate} />} action={{ label: "Import template", render: (_, refresh) => <TemplateImportPanel client={client} onChanged={refresh} /> }}>
        {(value, refresh) => value.templates.length === 0 ? <RouteState kind="empty" title="No templates available" /> : <UtilityTable
            label="Templates"
            headers={["Template", "ID", "Last modified", "Source", "Actions"]}
            rows={value.templates.map((template) => ({
                key: template.templateId,
                cells: [
                    <><strong>{template.displayName}</strong><small>{template.description ?? template.templateVersion}</small></>,
                    template.templateId,
                    formatTime(template.updatedAtMs),
                    humanize(template.sourceKind),
                    <TemplateActions client={client} compact onChanged={refresh} onView={() => navigate("/templates/" + template.templateId)} template={template} />
                ]
            }))}
        />}
    </UtilityWorkspace>;
}

export function TemplateDetailPage({ client, navigate, templateId }: PageProps & { templateId: string }) {
    const load = useCallback(() => client.templateGet(templateId), [client, templateId]);
    return <UtilityWorkspace title="Template detail" back={<BackTo label="Templates" navigate={navigate} path="/templates" />} load={load}>{({ template }, refresh) => <><dl className="detail-grid"><Detail label="Name" value={template.displayName} /><Detail label="Version" value={template.templateVersion} /><Detail label="Source" value={template.sourceKind} /><Detail label="Revision" value={String(template.revision)} /><Detail label="Provenance" value={template.provenance} /><Detail label="Favorite" value={template.favorite ? "Yes" : "No"} /></dl><TemplateActions client={client} onChanged={refresh} template={template} /></>}</UtilityWorkspace>;
}

export function UnityPage({ client, navigate }: PageProps) {
    const load = useCallback(() => loadAllUnityInstallations(client), [client]);
    return <UtilityWorkspace title="Unity" load={load} navigation={<UtilityNavigation current="/unity" kind="settings" navigate={navigate} />} action={{ label: "Manage installations", render: (installations, refresh) => <UnityRegistryActions client={client} installations={installations} onChanged={refresh} /> }}>{(installations) => <>{installations.length === 0 ? <RouteState kind="empty" title="No Unity installations registered" /> : <UtilityTable label="Unity installations" headers={["Version", "Architecture", "Source", "Installation"]} rows={installations.map((item) => ({ key: item.installationId, cells: [item.unityVersion, item.architecture, humanize(item.sourceKind), item.installationId] }))} />}</>}</UtilityWorkspace>;
}

export function ProjectUnityPage({ afterMigrationOpen = false, client, navigate, projectId }: PageProps & { afterMigrationOpen?: boolean; projectId: string }) {
    const load = useCallback(async () => {
        const project = (await client.projectGet(projectId)).project;
        if (project.revision === undefined) throw { code: "project_not_registered" };
        const [installations, launchConfig, launchOptions, writer] = await Promise.all([
            loadAllUnityInstallations(client),
            client.unityProjectLaunchConfigGet(projectId),
            client.unityLaunchOptions(projectId, project.revision),
            client.unityWriterState(projectId).catch(() => undefined)
        ]);
        return { project, installations, launchConfig: launchConfig.config, launchOptions, writer };
    }, [client, projectId]);
    return <UtilityWorkspace title="Project Unity" back={<BackTo label="Project packages" navigate={navigate} path={`/projects/${projectId}/packages`} />} load={load}>{({ project, installations, launchConfig, launchOptions, writer }, refresh) => <><dl className="detail-grid"><Detail label="Project Unity version" value={project.unityVersion} /><Detail label="Exact installations" value={String(launchOptions.exactMatchingInstallations.length)} /><Detail label="Writer observation" value={writer?.state ?? "Unknown"} /><Detail label="Arguments" value={launchConfig.arguments.length === 0 ? "Default" : `${launchConfig.arguments.length} configured arguments`} /><Detail label="Observed" value={writer === undefined ? "—" : formatTime(writer.checkedAtMs)} /></dl><ProjectUnityActions afterMigrationOpen={afterMigrationOpen} client={client} installations={installations} launchConfig={launchConfig} launchOptions={launchOptions} onChanged={refresh} project={project} /></>}</UtilityWorkspace>;
}

async function loadAllUnityInstallations(client: GuiRpcClient): Promise<UnityInstallation[]> {
    const installations: UnityInstallation[] = [];
    let cursor: string | undefined;
    do {
        const page = await client.unityInstallationsList(cursor);
        installations.push(...page.installations);
        cursor = page.nextCursor;
    } while (cursor !== undefined);
    return installations;
}

export function ProjectBackupsPage({ client, navigate, projectId }: PageProps & { projectId: string }) {
    const load = useCallback(async () => ({ backups: await client.backupsList(projectId), project: (await client.projectGet(projectId)).project }), [client, projectId]);
    return <UtilityWorkspace title="Backups" back={<BackTo label="Project packages" navigate={navigate} path={"/projects/" + projectId + "/packages"} />} load={load}>{(value, refresh) => <><p>{projectName(value.project)}</p><ActionDisclosure title="Create backup"><BackupCreatePanel client={client} onChanged={refresh} project={value.project} /></ActionDisclosure>{value.backups.backups.length === 0 ? <RouteState kind="empty" title="No backups for this project" /> : <UtilityTable label="Backups" headers={["Created", "Size", "Packages", "Actions"]} rows={value.backups.backups.map((backup) => ({ key: backup.backupId, cells: [formatTime(backup.createdAtMs), formatBytes(backup.archiveBytes), backup.excludeVpmPackages ? "VPM packages excluded" : "VPM packages included", <Button onClick={() => navigate("/backups/" + backup.backupId)} type="button" variant="text">View backup</Button>] }))} />}</>}</UtilityWorkspace>;
}

export function BackupDetailPage({ client, navigate, backupId }: PageProps & { backupId: string }) {
    const load = useCallback(() => client.backupGet(backupId), [client, backupId]);
    return <UtilityWorkspace title="Backup detail" load={load}>{(backup) => <><BackTo label="Project backups" navigate={navigate} path={`/projects/${backup.sourceProjectId}/backups`} /><dl className="detail-grid"><Detail label="Source project" value={backup.sourceProjectId} /><Detail label="Created" value={formatTime(backup.createdAtMs)} /><Detail label="Archive size" value={formatBytes(backup.archiveBytes)} /><Detail label="Format" value={`v${backup.formatVersion}`} /><Detail label="Compression" value={backup.compressionMode} /><Detail label="Integrity" value={shortHash(backup.archiveSha256)} /></dl><ActionDisclosure title="Restore backup"><BackupRestorePanel backup={backup} client={client} /></ActionDisclosure></>}</UtilityWorkspace>;
}

export function OperationsPage({ client, navigate }: PageProps) {
    const load = useCallback(() => client.operationsList(), [client]);
    return <UtilityWorkspace title="Task Center" load={load}>{(value) => value.operations.length === 0 ? <RouteState kind="empty" title="No operations yet" /> : <UtilityTable label="Tasks" headers={["Task", "Status", "Updated", "Actions"]} rows={value.operations.map((operation) => ({ key: operation.operationId, cells: [humanize(operation.kind), <span className={"state-chip state-chip--" + operation.state}>{humanize(operation.state)}</span>, formatTime(operation.updatedAtMs), <Button onClick={() => navigate("/operations/" + operation.operationId)} type="button" variant="text">View operation</Button>] }))} />}</UtilityWorkspace>;
}

export function OperationDetailPage({ client, navigate, operationId }: PageProps & { operationId: string }) {
    const load = useCallback(() => client.operationGet(operationId), [client, operationId]);
    return <UtilityWorkspace title="Operation detail" back={<BackTo label="Task Center" navigate={navigate} path="/operations" />} load={load}>{(operation, refresh) => <><dl className="detail-grid"><Detail label="Kind" value={operation.kind} /><Detail label="State" value={operation.state} /><Detail label="Phase" value={operation.progress?.phase ?? "—"} /><Detail label="Revision" value={String(operation.revision)} /><Detail label="Updated" value={formatTime(operation.updatedAtMs)} /><Detail label="Error" value={operation.errorCode ?? "None"} /></dl>{operation.diagnosticId === undefined ? null : <p>Diagnostic ID: <code>{operation.diagnosticId}</code></p>}<OperationActions client={client} onChanged={refresh} operation={operation} /></>}</UtilityWorkspace>;
}

export function ExtensionsPage({ client, navigate }: PageProps) {
    const load = useCallback(() => client.extensionsList(), [client]);
    const canManage = useCapability(capabilities.extensionsLifecycle);
    const canOpen = useCapability(capabilities.extensionsPortableUi);
    const pending = useRef(false);
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState<string>();
    const [message, setMessage] = useState<string>();
    const toggle = async (id: string, revision: number, enabled: boolean, refresh: () => void) => {
        if (!canManage || pending.current) return;
        pending.current = true;
        setBusy(true);
        setError(undefined);
        setMessage(undefined);
        try {
            const result = await (enabled ? client.extensionEnable(id, revision) : client.extensionDisable(id, revision));
            setMessage(`${result.extension.extensionId}: ${result.extension.desiredState === "enabled" ? "enabled" : "disabled"}.`);
            refresh();
        } catch (caught: unknown) {
            setError(safeError(caught).code);
        } finally {
            pending.current = false;
            setBusy(false);
        }
    };
    return <UtilityWorkspace title="Extensions" load={load} action={{ label: "Install extension", render: (_, refresh) => <ExtensionInstallPanel client={client} onChanged={refresh} /> }}>{(value, refresh) => (
        <section className="extension-management" aria-labelledby="extension-management-title">
            <header className="extension-management-heading"><h2 id="extension-management-title">Manage extensions</h2><p>Open an extension or change whether it is enabled.</p></header>
            {error === undefined ? null : <p className="inline-error" role="alert">Extension action failed: {error}</p>}
            {message === undefined ? null : <p role="status">{message}</p>}
            <div className="extension-section-heading"><h3>Installed</h3><span>{value.extensions.length}</span></div>
            {value.extensions.length === 0 ? <RouteState kind="empty" title="No extensions installed" /> : <div className="extension-cards">{value.extensions.map((extension) => {
                const enabled = extension.desiredState === "enabled";
                const unavailable = extension.desiredState === "uninstalling" || extension.quarantineState === "quarantined";
                return <article className="extension-card" key={extension.extensionId} aria-label={extension.extensionId}>
                    <div className="extension-card-heading"><div><h4>{extension.extensionId}</h4><p>Version {extension.version}</p></div>{extension.ui?.protocol !== "portable-v1" ? null : <Button disabled={!canOpen || !enabled || unavailable || busy} onClick={() => navigate(`/extensions/${extension.extensionId}/ui`)} title={capabilityUnavailableTitle(canOpen, capabilities.extensionsPortableUi)} type="button" variant="tonal">Open</Button>}</div>
                    <p className="extension-card-status">{humanize(extension.runtimeState)} · {humanize(extension.trustDecision)}{extension.quarantineState === "clear" ? "" : ` · ${humanize(extension.quarantineState)}`}</p>
                    <div className="extension-card-actions"><Button onClick={() => navigate(`/extensions/${extension.extensionId}`)} type="button" variant="text">Manage extension</Button><Switch disabled={!canManage || busy || unavailable} label="Enabled" selected={enabled} onChange={(next) => void toggle(extension.extensionId, extension.revision, next, refresh)} /></div>
                </article>;
            })}</div>}
            {value.nextCursor === undefined ? null : <p role="status">More installed extensions exist. This view contains the first page.</p>}
        </section>
    )}</UtilityWorkspace>;
}

export function ExtensionDetailPage({ client, navigate, extensionId }: PageProps & { extensionId: string }) {
    const canUsePortableUi = useCapability(capabilities.extensionsPortableUi);
    const load = useCallback(() => client.extensionGet(extensionId), [client, extensionId]);
    return <UtilityWorkspace title="Extension detail" back={<BackTo label="Extensions" navigate={navigate} path="/extensions" />} load={load}>{({ extension }, refresh) => <><dl className="detail-grid"><Detail label="Version" value={extension.version} /><Detail label="Publisher" value={shortHash(extension.publisherFingerprint)} /><Detail label="Trust" value={humanize(extension.trustDecision)} /><Detail label="Desired state" value={humanize(extension.desiredState)} /><Detail label="Runtime" value={humanize(extension.runtimeState)} /><Detail label="Quarantine" value={humanize(extension.quarantineState)} /><Detail label="Grant revision" value={String(extension.grantRevision)} /><Detail label="Record revision" value={String(extension.revision)} /></dl>{extension.ui?.protocol === "portable-v1" ? <Button disabled={!canUsePortableUi} onClick={() => navigate(`/extensions/${extensionId}/ui`)} title={capabilityUnavailableTitle(canUsePortableUi, capabilities.extensionsPortableUi)} type="button">Open Portable UI</Button> : <RouteState kind="empty" title="This extension has no Portable UI" />}<ExtensionActions client={client} extension={extension} onChanged={refresh} /></>}</UtilityWorkspace>;
}

export function AboutPage({ client }: PageProps) {
    const load = useCallback(() => client.systemStatus(), [client]);
    return <UtilityWorkspace title="About" load={load}>{(status) => <><dl className="detail-grid"><Detail label="Product" value={status.product} /><Detail label="Daemon" value={status.daemonVersion} /><Detail label="RPC" value={`v${status.rpcVersion}`} /><Detail label="State" value={status.state} /></dl><section className="notice-card"><h2>Licenses</h2><p>ALCOMD source is licensed under AGPL-3.0-only.</p><p>The canonical third-party dependency notice source is <code>THIRD_PARTY_NOTICES.md</code> in the product distribution.</p></section></>}</UtilityWorkspace>;
}

export function ActivityPage({ client, navigate }: PageProps) {
    const load = useCallback(() => client.activityList(), [client]);
    const [search, setSearch] = useState("");
    return <UtilityWorkspace title="Activity" navigation={<UtilityNavigation current="/activity" kind="logs" navigate={navigate} />} load={load}
        tools={() => <SearchField className="logs-search" label="Search activity" value={search} onInput={setSearch} />}>
        {(value) => <PagedItems initialItems={value.items} initialCursor={value.nextCursor} loadMore={(cursor) => client.activityList(cursor)}>
            {(items) => <ActivityLogRows client={client} items={items} search={search} />}
        </PagedItems>}
    </UtilityWorkspace>;
}

type LoadedActivityItem = Awaited<ReturnType<GuiRpcClient["activityList"]>>["items"][number];
type LoadedDiagnosticItem = Awaited<ReturnType<GuiRpcClient["diagnosticsList"]>>["items"][number];

function LogFilter({ label, value, values, onChange }: { label: string; value: string; values: string[]; onChange(value: string): void }) {
    const options = [...new Set(values)].sort().map((item) => ({ label: item === "unknown" ? "Unknown" : humanize(item), value: item }));
    if (value !== "" && !options.some((item) => item.value === value)) options.push({ label: humanize(value), value });
    // Rapid filter changes must not be overwritten by the previous menu's closing animation.
    return <Select className="logs-filter" quick label={label} value={value} onChange={onChange} options={[{ label: `All ${label.toLowerCase()}`, value: "" }, ...options]} />;
}

function ActivityLogRows({ client, items, search }: { client: GuiRpcClient; items: LoadedActivityItem[]; search: string }) {
    const [status, setStatus] = useState("");
    const [kind, setKind] = useState("");
    const [details, setDetails] = useState(false);
    const [selected, setSelected] = useState<string>();
    const query = search.trim().toLowerCase();
    const shown = items.filter((item) => (status === "" || (item.state ?? "unknown") === status)
        && (kind === "" || item.type === kind)
        && [item.summaryCode, humanize(item.summaryCode), item.state, item.resourceKind, item.resourceId, item.operationId].some((value) => value?.toLowerCase().includes(query)));
    return <div className="logs-workspace">
        <div className="logs-filter-toolbar" aria-label="Activity filters">
            <LogFilter label="Status" value={status} values={items.map((item) => item.state ?? "unknown")} onChange={setStatus} />
            <LogFilter label="Record type" value={kind} values={items.map((item) => item.type)} onChange={setKind} />
            <Checkbox checked={details} label="Show details" onChange={setDetails} />
            <p className="logs-result-count" role="status">{shown.length} of {items.length} loaded entries · Search and filters apply to loaded entries only.</p>
        </div>
        <MaterialDataTable label="Activity" minWidth={720}>
            <thead><tr>{["Time", "Status", "Activity", "Target", "Actions"].map((label) => <DataTableHeader key={label}>{label}</DataTableHeader>)}</tr></thead>
            <tbody>{shown.map((item, index) => <tr key={`${item.type}-${item.eventSequence ?? item.operationId}-${item.occurredAtMs}-${index}`}>
                <td className="logs-time">{formatTime(item.occurredAtMs)}</td>
                <td><span className={`state-chip state-chip--${item.state ?? "unknown"}`}>{item.state === undefined ? "Unknown" : humanize(item.state)}</span></td>
                <td className="logs-summary"><span title={item.summaryCode}>{humanize(item.summaryCode)}</span><small>{humanize(item.type)}</small>{details ? <dl className="logs-details"><Detail label="Summary code" value={item.summaryCode} />{item.operationId === undefined ? null : <Detail label="Operation" value={item.operationId} />}{item.eventSequence === undefined ? null : <Detail label="Event sequence" value={String(item.eventSequence)} />}</dl> : null}</td>
                <td>{item.resourceKind === undefined ? "—" : humanize(item.resourceKind)}{item.resourceId === undefined ? null : <small>{item.resourceId}</small>}</td>
                <td>{item.operationId === undefined ? null : <Button onClick={() => setSelected(item.operationId)} type="button" variant="text">View operation</Button>}</td>
            </tr>)}{shown.length === 0 ? <tr><td colSpan={5}>{items.length === 0 ? "No activity yet" : "No matching loaded activity. Change the filters or load more entries."}</td></tr> : null}</tbody>
        </MaterialDataTable>
        {selected === undefined ? null : <LogOperationDialog client={client} operationId={selected} onClose={() => setSelected(undefined)} />}
    </div>;
}

function LogOperationDialog({ client, operationId, onClose }: { client: GuiRpcClient; operationId: string; onClose(): void }) {
    const load = useCallback(() => client.operationGet(operationId), [client, operationId]);
    return <Dialog wide open title="Operation detail" onClose={onClose}>
        <ResourcePage load={load} showRefreshBar={false}>{(operation) => <dl className="logs-operation-details">
            <Detail label="Operation" value={operation.operationId} /><Detail label="Kind" value={humanize(operation.kind)} />
            <Detail label="State" value={humanize(operation.state)} /><Detail label="Phase" value={operation.progress?.phase ?? "—"} />
            <Detail label="Updated" value={formatTime(operation.updatedAtMs)} /><Detail label="Error" value={operation.errorCode ?? "None"} />
            {operation.diagnosticId === undefined ? null : <Detail label="Diagnostic ID" value={operation.diagnosticId} />}
        </dl>}</ResourcePage>
        <div className="dialog-actions"><Button onClick={onClose} type="button" variant="text">Close</Button></div>
    </Dialog>;
}

function DiagnosticLogRows({ client, items, search }: { client: GuiRpcClient; items: LoadedDiagnosticItem[]; search: string }) {
    const [severity, setSeverity] = useState("");
    const [subsystem, setSubsystem] = useState("");
    const [selected, setSelected] = useState<string>();
    const query = search.trim().toLowerCase();
    const shown = items.filter((item) => (severity === "" || item.severity === severity)
        && (subsystem === "" || item.subsystem === subsystem)
        && [item.code, item.summary, item.subsystem, item.diagnosticId, item.operationId].some((value) => value?.toLowerCase().includes(query)));
    return <div className="logs-workspace">
        <div className="logs-filter-toolbar" aria-label="Diagnostic filters">
            <LogFilter label="Severity" value={severity} values={["warning", "error"]} onChange={setSeverity} />
            <LogFilter label="Subsystem" value={subsystem} values={items.map((item) => item.subsystem)} onChange={setSubsystem} />
            <p className="logs-result-count" role="status">{shown.length} of {items.length} loaded entries · Search and filters apply to loaded entries only.</p>
        </div>
        <MaterialDataTable label="Diagnostics" minWidth={720}>
            <thead><tr>{["Time", "Severity", "Subsystem", "Diagnostic", "Actions"].map((label) => <DataTableHeader key={label}>{label}</DataTableHeader>)}</tr></thead>
            <tbody>{shown.map((item, index) => <tr key={`${item.operationId}-${item.occurredAtMs}-${index}`}>
                <td className="logs-time">{formatTime(item.occurredAtMs)}</td><td><span className={`logs-severity logs-severity--${item.severity}`}>{humanize(item.severity)}</span></td><td>{item.subsystem}</td>
                <td className="logs-summary"><strong>{item.code}</strong><p>{item.summary}</p>{item.diagnosticId === undefined ? null : <small>Diagnostic ID: {item.diagnosticId}</small>}</td>
                <td>{item.operationId === undefined ? null : <Button onClick={() => setSelected(item.operationId)} type="button" variant="text">View operation</Button>}</td>
            </tr>)}{shown.length === 0 ? <tr><td colSpan={5}>{items.length === 0 ? "No diagnostics reported" : "No matching loaded diagnostics. Change the filters or load more entries."}</td></tr> : null}</tbody>
        </MaterialDataTable>
        {selected === undefined ? null : <LogOperationDialog client={client} operationId={selected} onClose={() => setSelected(undefined)} />}
    </div>;
}

export function DiagnosticsPage({ client, navigate }: PageProps) {
    const canCheckState = useCapability(capabilities.stateCheck);
    const load = useCallback(() => client.diagnosticsList(), [client]);
    const [operationId, setOperationId] = useState<string>();
    const [checkError, setCheckError] = useState<RpcError>();
    const [checking, setChecking] = useState(false);
    const checkingRef = useRef(false);
    const [search, setSearch] = useState("");
    const runStateCheck = async () => {
        if (!canCheckState || checkingRef.current) return;
        checkingRef.current = true;
        setChecking(true);
        setCheckError(undefined);
        try {
            const result = await client.stateCheck();
            setOperationId(result.operationId);
        } catch (caught: unknown) {
            setCheckError(safeError(caught));
        } finally {
            checkingRef.current = false;
            setChecking(false);
        }
    };
    return <UtilityWorkspace title="Diagnostics" load={load} navigation={<UtilityNavigation current="/diagnostics" kind="logs" navigate={navigate} />}
        tools={() => <><SearchField className="logs-search" label="Search diagnostics" value={search} onInput={setSearch} /><Button widthLabels={["Starting…", "Run state check"]} disabled={!canCheckState || checking} onClick={() => void runStateCheck()} title={capabilityUnavailableTitle(canCheckState, capabilities.stateCheck)} type="button" variant="tonal">{checking ? "Starting…" : "Run state check"}</Button></>}>
        {(value) => <>{checkError === undefined ? null : <p className="inline-error" role="alert">State check failed: {checkError.code}</p>}{operationId === undefined ? null : <OperationFollow client={client} operationId={operationId} />}
            <PagedItems initialItems={value.items} initialCursor={value.nextCursor} loadMore={(cursor) => client.diagnosticsList(cursor)}>{(items) => <DiagnosticLogRows client={client} items={items} search={search} />}</PagedItems>
        </>}
    </UtilityWorkspace>;
}

export function SettingsPage(props: PageProps & { onApplied(value: SettingsGetResult): void; onDirtyChange(dirty: boolean): void }) {
    return <SettingsWorkspace {...props} />;
}

export function Page({ actions, children, eyebrow, title }: { actions?: ReactNode; children: ReactNode; eyebrow: string; title: string }) {
    return <section className="page-surface page-surface--utility" aria-labelledby="route-title"><header className="page-header"><div><p className="eyebrow">{eyebrow}</p><h1 id="route-title" tabIndex={-1}>{title}</h1></div>{actions}</header>{children}</section>;
}

function Detail({ label, value }: { label: string; value: string }) { return <div><dt>{label}</dt><dd>{value}</dd></div>; }

function UtilityTable({ headers, label, rows }: { headers: string[]; label: string; rows: { key: string; cells: ReactNode[] }[] }) {
    return <MaterialDataTable label={label} minWidth={640}><thead><tr>{headers.map((header) => <DataTableHeader key={header}>{header}</DataTableHeader>)}</tr></thead><tbody>{rows.map((row) => <tr key={row.key}>{row.cells.map((cell, index) => <td key={headers[index]}>{cell}</td>)}</tr>)}</tbody></MaterialDataTable>;
}

function DataTable({ headers, rows }: { headers: string[]; rows: string[][] }) {
    if (rows.length === 0) return <RouteState kind="empty" title="No matching items" />;
    return (
        <MaterialDataTable label="Repository packages" minWidth={480}>
            <thead><tr>{headers.map((header) => <DataTableHeader key={header}>{header}</DataTableHeader>)}</tr></thead>
            <tbody>{rows.map((row, rowIndex) => <tr key={`${rowIndex}-${row[0]}`}>{row.map((cell, index) => <td key={`${headers[index]}-${index}`}>{cell}</td>)}</tr>)}</tbody>
        </MaterialDataTable>
    );
}

function sourceText(repository: RepositorySnapshot): string { return repository.source.kind === "local" ? "Local repository" : repository.source.url; }
function formatTime(value: number): string { return new Intl.DateTimeFormat("en-US", { dateStyle: "medium", timeStyle: "short", timeZone: "UTC" }).format(new Date(value)) + " UTC"; }
function formatBytes(value: number): string { return value < 1024 * 1024 ? `${Math.ceil(value / 1024)} KiB` : `${(value / (1024 * 1024)).toFixed(1)} MiB`; }
function shortHash(value: string): string { return value.length <= 24 ? value : `${value.slice(0, 16)}…${value.slice(-8)}`; }
function humanize(value: string): string { return value.replaceAll("_", " ").replaceAll(".", " "); }

function safeError(caught: unknown): RpcError {
    if (typeof caught === "object" && caught !== null && "code" in caught && typeof caught.code === "string") {
        return { code: caught.code, message: "The request could not be completed.", ...("diagnosticId" in caught && typeof caught.diagnosticId === "string" ? { diagnosticId: caught.diagnosticId } : {}) };
    }
    return { code: "internal_error", message: "The request could not be completed." };
}
