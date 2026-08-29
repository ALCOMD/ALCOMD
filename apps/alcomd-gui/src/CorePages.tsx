import type { ExtensionRecord, RpcError } from "@alcomd/sdk";
import {
    accountCircleIcon,
    arrowBackIcon,
    arrowDownwardIcon,
    arrowUpwardIcon,
    backupIcon,
    deleteIcon,
    downloadIcon,
    historyIcon,
    helpIcon,
    moreVertIcon,
    playArrowIcon,
    publicIcon,
    refreshIcon,
    searchIcon,
    starIcon,
    syncIcon,
    upgradeIcon,
    viewGridIcon,
    viewListIcon
} from "@alcomd/ui/icons";
import { useCallback, useEffect, useRef, useState, type FormEvent, type ReactNode } from "react";

import type {
    ActivityItem,
    BackupRecord,
    DiagnosticItem,
    OfficialSettings,
    Operation,
    PackageCursor,
    PackageSourceSelector,
    ProjectCopyPlan,
    ProjectEditorSelectionState,
    ProjectSnapshot,
    RegistryCursor,
    RepositoryPackageVersion,
    RepositorySnapshot,
    TemplateRecord,
    UnityInstallation,
    UserPackageRecord,
    SettingsGetResult,
    SettingsLocale
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
    RegisterRepositoryPanel,
    RepositoryActions,
    TemplateActions,
    TemplateImportPanel,
    UnityRegistryActions
} from "./CoreActions";
import { DataTableHeader, MaterialDataTable } from "./DataTable";
import type { GuiRpcClient } from "./rpc";
import { Button, Checkbox, Dialog, Icon, IconButton, Menu, MenuItem, Select, TextField } from "./Material";
import { CreateProjectDialog, RestoreProjectDialog } from "./ProjectCreationDialogs";

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
            <Button disabled={refreshing} onClick={refresh} type="button" variant="tonal">
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
    const disconnected = error.code === "daemon_unavailable";
    return (
        <section className={`route-state route-state--${disconnected ? "disconnected" : "error"}`} role="alert">
            <h2>{disconnected ? "ALCOMD core disconnected" : "Request failed"}</h2>
            <p><code>{error.code}</code></p>
            {error.diagnosticId === undefined ? null : <p>Diagnostic ID: <code>{error.diagnosticId}</code></p>}
            <Button onClick={retry} type="button">Reconnect and retry</Button>
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
                <TextField aria-label="Search projects" className="projects-search" label="" leadingIcon={<Icon asset={searchIcon} slot="leading-icon" />} onInput={setSearch} placeholder="Search..." type="search" value={search} variant="filled" />
                <Button onClick={() => setView((current) => current === "list" ? "grid" : "list")} type="button" variant="text">
                    <Icon asset={view === "list" ? viewGridIcon : viewListIcon} slot="icon" />
                    <StateSizedLabel current={view === "list" ? "Grid view" : "List view"} labels={["Grid view", "List view"]} />
                </Button>
                <Button onClick={() => setRestoreOpen(true)} ref={restoreButtonRef} type="button" variant="tonal">Restore project</Button>
                <Button disabled={selectingDirectory} onClick={() => void chooseProjectDirectory()} ref={registerButtonRef} type="button">
                    <StateSizedLabel current={selectingDirectory ? "Choosing…" : "Register project"} labels={["Register project", "Choosing…"]} />
                </Button>
                <Button onClick={() => setCreateOpen(true)} ref={createButtonRef} type="button">Create project</Button>
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

function ProjectRowActions({ client, navigate, onChanged, onFeedback, onProjectChanged, project }: { client: GuiRpcClient; navigate(path: string): void; onChanged(): void; onFeedback(message: string): void; onProjectChanged(project: ProjectSnapshot): void; project: ProjectSnapshot }) {
    const [copyParent, setCopyParent] = useState("");
    const [copyOpen, setCopyOpen] = useState(false);
    const [selectingCopyTarget, setSelectingCopyTarget] = useState(false);
    const [openingDirectory, setOpeningDirectory] = useState(false);
    const [opening, setOpening] = useState(false);
    const [menuOpen, setMenuOpen] = useState(false);
    const [unregistering, setUnregistering] = useState(false);
    const [confirmUnregister, setConfirmUnregister] = useState(false);
    const menuAnchorRef = useRef<HTMLElement>(null);
    const projectId = project.projectId;
    const revision = project.revision;
    if (projectId === undefined) return null;

    const openUnity = async () => {
        if (revision === undefined) return;
        setOpening(true);
        onFeedback("Opening Unity…");
        try {
            const result = await client.unityLaunch(projectId, revision);
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
            onChanged();
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

    const beginCopy = async () => {
        setMenuOpen(false);
        setSelectingCopyTarget(true);
        try {
            const selected = await client.selectDirectory();
            if (selected === undefined) return;
            setCopyParent(selected);
            setCopyOpen(true);
        } catch (caught: unknown) {
            onFeedback(`Unable to select copy destination: ${safeError(caught).code}`);
        } finally {
            setSelectingCopyTarget(false);
        }
    };

    return (
        <>
            <div className="project-row-actions">
                <ProjectFavoriteButton client={client} onChanged={onProjectChanged} onFeedback={onFeedback} onRefresh={onChanged} project={project} />
                <Button className="project-open-unity-action" disabled={opening || revision === undefined} onClick={() => void openUnity()} type="button">
                    <Icon asset={playArrowIcon} slot="icon" />
                    <StateSizedLabel current={opening ? "Opening…" : "Open Unity"} labels={["Open Unity", "Opening…"]} />
                </Button>
                <Button onClick={() => navigate(`/projects/${projectId}`)} type="button" variant="tonal">Manage</Button>
                <Button onClick={() => navigate(`/projects/${projectId}/backups`)} type="button" variant="tonal">Backups</Button>
                <IconButton className="project-more-actions" label={`More actions for ${projectName(project)}`} onClick={() => setMenuOpen(true)} ref={menuAnchorRef} type="button">
                    <Icon asset={moreVertIcon} size={24} />
                </IconButton>
                <Menu anchorRef={menuAnchorRef} className="project-actions-menu" onClose={() => setMenuOpen(false)} open={menuOpen}>
                    <MenuItem className="project-actions-menu-item" disabled={openingDirectory} label={openingDirectory ? "Opening Project Directory…" : "Open Project Directory"} onClick={() => void openDirectory()} />
                    <MenuItem className="project-actions-menu-item" disabled={revision === undefined || selectingCopyTarget} label={selectingCopyTarget ? "Choosing Copy Destination…" : "Copy Project"} onClick={() => void beginCopy()} />
                    <MenuItem className="project-actions-menu-item project-actions-menu-item--danger" disabled={revision === undefined} label="Remove Project" onClick={() => setConfirmUnregister(true)} />
                </Menu>
            </div>
            <Dialog onClose={() => setConfirmUnregister(false)} open={confirmUnregister} title="Remove this project?">
                <p>This removes the project from ALCOMD. It does not delete the Unity project directory.</p>
                <div className="dialog-actions">
                    <Button disabled={unregistering} onClick={() => setConfirmUnregister(false)} type="button" variant="text">Cancel</Button>
                    <Button className="material-button--danger" disabled={unregistering || revision === undefined} onClick={() => void unregister()} type="button" variant="text">
                        <StateSizedLabel current={unregistering ? "Removing…" : "Remove"} labels={["Remove", "Removing…"]} />
                    </Button>
                </div>
            </Dialog>
            <CopyProjectDialog
                client={client}
                onCompleted={() => {
                    onFeedback("Project copy completed.");
                    onChanged();
                }}
                onClose={() => setCopyOpen(false)}
                open={copyOpen}
                project={project}
                targetParent={copyParent}
            />
        </>
    );
}

function CopyProjectDialog({ client, onCompleted, onClose, open, project, targetParent }: { client: GuiRpcClient; onCompleted(targetProjectId: string): void; onClose(): void; open: boolean; project: ProjectSnapshot; targetParent: string }) {
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
                        <p>Choose the new project name. ALCOMD will copy the registered project into the selected directory without overwriting an existing target.</p>
                        <code>{targetParent}</code>
                        <TextField aria-label="Copied project name" label="Project name" onInput={setTargetLeaf} value={targetLeaf} />
                        <div className="dialog-actions">
                            <Button disabled={busy} onClick={onClose} type="button" variant="text">Cancel</Button>
                            <Button disabled={busy || targetLeaf.trim().length === 0} onClick={() => void createPlan()} type="button">{busy ? "Planning…" : "Review copy"}</Button>
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
                            <Button disabled={busy} onClick={() => void apply()} type="button">{busy ? "Starting…" : "Start copy"}</Button>
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
    return <IconButton aria-pressed={favorite} className="project-favorite-action" disabled={busy || revision === undefined} label={label} onClick={() => void toggle()} title={label} type="button"><Icon asset={starIcon} /></IconButton>;
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
                    <Button disabled={busy || path.length === 0} onClick={() => void register()} ref={confirmRef} type="button">{busy ? "Registering…" : "Confirm"}</Button>
                </div>
            </div>
            {error === undefined ? null : <p className="inline-error" role="alert">Registration failed: {error.code}</p>}
        </Dialog>
    );
}

function projectName(project: ProjectSnapshot): string {
    return displayProjectPath(project.rootPath).split(/[\\/]/).at(-1) ?? "Unity project";
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

function PackageSourceMenu({ label, onChange, options, value }: { label: string; onChange(value: string): void; options: PackageWorkspaceRow["sourceOptions"]; value: string }) {
    const [open, setOpen] = useState(false);
    const anchorRef = useRef<HTMLElement>(null);
    const selected = options.find((option) => option.key === value) ?? options[0];
    return (
        <>
            <Button aria-expanded={open} aria-label={label} className="package-row-source-menu" onClick={() => setOpen(true)} ref={anchorRef} type="button" variant="outlined">
                {selected?.label ?? "Choose source"}
            </Button>
            <Menu anchorRef={anchorRef} onClose={() => setOpen(false)} open={open}>
                {options.map((option) => <MenuItem key={option.key} label={option.label} onClick={() => { onChange(option.key); setOpen(false); }} />)}
            </Menu>
        </>
    );
}

interface PackageWorkspaceValue {
    catalog: WorkspaceCatalogVersion[];
    project: ProjectSnapshot;
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

async function listAllRepositoryPackages(client: GuiRpcClient, repositoryId: string): Promise<RepositoryPackageVersion[]> {
    const packages: RepositoryPackageVersion[] = [];
    let cursor: PackageCursor | undefined;
    do {
        const page = await client.repositoryPackages(repositoryId, cursor);
        packages.push(...page.packages);
        cursor = page.nextCursor;
    } while (cursor !== undefined);
    return packages;
}

async function loadPackageWorkspace(client: GuiRpcClient, projectId: string): Promise<PackageWorkspaceValue> {
    const [project, repositories, settings, userPackages] = await Promise.all([client.projectGet(projectId), listAllRepositories(client), client.settingsGet(), listAllUserPackages(client)]);
    const hidden = new Set(settings.settings.packages.hiddenRepositoryIds);
    const catalogs = await Promise.all(repositories.filter((repository) => repository.repositoryId === undefined || !hidden.has(repository.repositoryId)).map(async (repository) => {
        if (repository.repositoryId === undefined) return [] as WorkspaceCatalogVersion[];
        const packages = await listAllRepositoryPackages(client, repository.repositoryId);
        const source = repository.name ?? repository.declaredId ?? sourceText(repository);
        return packages
            .filter((item) => settings.settings.packages.showPrerelease || item.prerelease !== true)
            .map((item) => ({ ...item, repositoryId: repository.repositoryId as string, source, sourceKey: `repository:${repository.repositoryId}`, sourceKind: repository.source.kind, sourceSelector: { kind: "repository" as const, repositoryId: repository.repositoryId as string } }));
    }));
    const local = settings.settings.packages.hideLocalUserPackages ? [] : userPackages.map((item): WorkspaceCatalogVersion => ({
        packageId: item.packageId,
        version: item.version,
        ...(item.displayName === undefined ? {} : { displayName: item.displayName }),
        yanked: false,
        source: item.displayName ?? "Local / User Package",
        sourceKey: `user-package:${item.userPackageId}`,
        sourceKind: "user-package",
        sourceSelector: { kind: "user_package", userPackageId: item.userPackageId }
    }));
    return { project: project.project, catalog: [...catalogs.flat(), ...local] };
}

function ProjectWorkspaceCopyAction({ client, navigate, project }: { client: GuiRpcClient; navigate(path: string): void; project: ProjectSnapshot }) {
    const [copyParent, setCopyParent] = useState("");
    const [copyOpen, setCopyOpen] = useState(false);
    const [selecting, setSelecting] = useState(false);
    const [error, setError] = useState<RpcError>();

    const beginCopy = async () => {
        setSelecting(true);
        setError(undefined);
        try {
            const selected = await client.selectDirectory();
            if (selected === undefined) return;
            setCopyParent(selected);
            setCopyOpen(true);
        } catch (caught: unknown) {
            setError(safeError(caught));
        } finally {
            setSelecting(false);
        }
    };

    return (
        <>
            {error === undefined ? null : <span className="inline-error" role="alert">Copy unavailable: {error.code}</span>}
            <Button disabled={selecting || project.revision === undefined} onClick={() => void beginCopy()} type="button" variant="text">
                {selecting ? "Choosing destination…" : "Copy project"}
            </Button>
            <CopyProjectDialog
                client={client}
                onCompleted={(targetProjectId) => navigate(`/projects/${targetProjectId}`)}
                onClose={() => setCopyOpen(false)}
                open={copyOpen}
                project={project}
                targetParent={copyParent}
            />
        </>
    );
}

function ProjectPackageWorkspace({ client, navigate, projectId }: PageProps & { projectId: string }) {
    const load = useCallback(() => loadPackageWorkspace(client, projectId), [client, projectId]);
    const [search, setSearch] = useState("");
    const [filter, setFilter] = useState("all");
    const [sourceFilter, setSourceFilter] = useState<"all" | "local" | "remote" | "user-package">("all");
    const [sourceSelections, setSourceSelections] = useState<Record<string, string>>({});
    const [bulkSelection, setBulkSelection] = useState<string[]>([]);
    const [repositoryRefresh, setRepositoryRefresh] = useState<RepositoryRefreshProgress>();
    const [selection, setSelection] = useState<PackageActionSelection>();
    const selectedSource = (row: PackageWorkspaceRow): PackageSourceSelector | undefined => {
        const selectedKey = sourceSelections[row.packageId];
        return row.sourceOptions.find((option) => option.key === selectedKey)?.selector ?? (row.sourceOptions.length === 1 ? row.sourceOptions[0]?.selector : undefined);
    };
    const selectAction = (action: PackageActionSelection["action"], row?: PackageWorkspaceRow, version?: string) => {
        const packageId = row?.packageId ?? "";
        const source = row === undefined ? undefined : selectedSource(row);
        setSelection({ action, key: (selection?.key ?? 0) + 1, packageId, ...(version === undefined ? {} : { version }), ...(source === undefined ? {} : { source }) });
    };
    const selectBulkReinstall = (rows: PackageWorkspaceRow[]) => {
        const selectedRows = rows.filter((row) => bulkSelection.includes(row.packageId) && row.installedVersion !== undefined);
        const sources = selectedRows.flatMap((row) => {
            const source = selectedSource(row);
            return source === undefined ? [] : [{ packageId: row.packageId, source }];
        });
        setSelection({ action: "bulk-reinstall", key: (selection?.key ?? 0) + 1, packageId: "", packageIds: selectedRows.map((row) => row.packageId), sources });
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
        <ResourcePage load={load} showRefreshBar={false}>{({ project, catalog }, refresh, refreshing, refreshError) => {
            const rows = packageWorkspaceRows(project, catalog).filter((row) => {
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
                                <p title={project.rootPath}>{project.rootPath}</p>
                            </div>
                        </div>
                        <nav aria-label="Project actions" className="project-workspace-actions">
                            {refreshError === undefined ? null : <span className="inline-error" role="alert">Refresh failed: {refreshError.code}</span>}
                            <span className="project-unity-version">Unity {project.unityVersion}</span>
                            <Button onClick={() => navigate(`/projects/${projectId}/unity`)} type="button"><Icon asset={playArrowIcon} slot="icon" />Open Unity</Button>
                            <Button onClick={() => navigate(`/projects/${projectId}/backups`)} type="button" variant="text"><Icon asset={backupIcon} slot="icon" />Backups</Button>
                            <ProjectWorkspaceCopyAction client={client} navigate={navigate} project={project} />
                        </nav>
                    </header>
                    <section aria-labelledby="packages-heading" className="package-workspace-surface">
                        <header className="package-workspace-toolbar">
                            <h2 id="packages-heading">Manage packages</h2>
                            <Button className="package-refresh-action" disabled={refreshing || repositoryRefresh?.running === true} onClick={() => void refreshRepositories(refresh)} type="button" variant="text"><Icon asset={refreshIcon} slot="icon" />{repositoryRefresh?.running === true ? "Refreshing…" : "Refresh"}</Button>
                            <TextField className="package-workspace-search" label="Search packages" leadingIcon={<Icon asset={searchIcon} slot="leading-icon" />} onInput={setSearch} value={search} />
                            <Select
                                className="package-workspace-filter"
                                label="Filter"
                                onChange={setFilter}
                                options={[
                                    { label: "All packages", value: "all" },
                                    { label: "Installed", value: "installed" },
                                    { label: "Available", value: "available" },
                                    { label: "Missing source", value: "missing" }
                                ]}
                                value={filter}
                            />
                            <Select
                                className="package-workspace-source-filter"
                                label="Source"
                                onChange={(value) => setSourceFilter(value as typeof sourceFilter)}
                                options={[
                                    { label: "All", value: "all" },
                                    { label: "Remote", value: "remote" },
                                    { label: "Local repository", value: "local" },
                                    { label: "User Packages", value: "user-package" }
                                ]}
                                value={sourceFilter}
                            />
                            <Button onClick={() => selectAction("resolve")} type="button" variant="text"><Icon asset={syncIcon} slot="icon" />Resolve</Button>
                            <Button onClick={() => selectAction("reinstall-all")} type="button" variant="text">Reinstall all</Button>
                            <Button disabled={bulkSelection.length === 0} onClick={() => selectBulkReinstall(rows)} type="button" variant="text">Reinstall selected</Button>
                            <span className="package-workspace-count" role="status" aria-live="polite">{rows.length} packages</span>
                        </header>
                        {repositoryRefresh === undefined ? null : (
                            <div className={repositoryRefresh.failures.length > 0 ? "package-refresh-status package-refresh-status--failed" : "package-refresh-status"} role={repositoryRefresh.failures.length > 0 ? "alert" : "status"} aria-live="polite">
                                <span>{repositoryRefresh.running
                                    ? `Refreshing repositories: ${repositoryRefresh.completed} of ${repositoryRefresh.total}.`
                                    : `${repositoryRefresh.successes} refreshed, ${repositoryRefresh.failures.length} failed.`}</span>
                                <ul className="visually-hidden">{repositoryRefresh.results.map((result) => <li key={result.repositoryId}>{result.repositoryId}: {result.status}{result.code === undefined ? "" : ` (${result.code})`}</li>)}</ul>
                            </div>
                        )}
                        <div className="package-workspace-table-scroll">
                            {rows.length === 0 ? <section className="projects-empty" role="status"><h3>No matching packages</h3><p>Change the search or package filter.</p></section> : (
                                <MaterialDataTable className="package-workspace-table" label="Packages" minWidth={920}>
                                    <colgroup><col className="package-column-select" /><col className="package-column-name" /><col className="package-column-installed" /><col className="package-column-latest" /><col className="package-column-source" /><col className="package-column-actions" /></colgroup>
                                    <thead><tr><DataTableHeader><span className="visually-hidden">Select</span></DataTableHeader><DataTableHeader>Package</DataTableHeader><DataTableHeader>Installed</DataTableHeader><DataTableHeader>Latest</DataTableHeader><DataTableHeader>Source</DataTableHeader><DataTableHeader><span className="visually-hidden">Actions</span></DataTableHeader></tr></thead>
                                    <tbody>{rows.map((row) => {
                                        const latest = row.availableVersions.at(-1);
                                        const canUpgrade = row.installedVersion !== undefined && latest !== undefined && latest !== row.installedVersion;
                                        return (
                                            <tr key={row.packageId}>
                                                <td><Checkbox checked={bulkSelection.includes(row.packageId)} disabled={row.installedVersion === undefined} label={`Select ${row.displayName}`} onChange={(checked) => setBulkSelection((current) => checked ? [...new Set([...current, row.packageId])] : current.filter((packageId) => packageId !== row.packageId))} /></td>
                                                <td><strong>{row.displayName}</strong><small>{row.packageId}</small></td>
                                                <td>{row.installedVersion ?? "—"}{row.requestedRange === undefined ? null : <small>Requested {row.requestedRange}</small>}</td>
                                                <td>{canUpgrade ? <Button onClick={() => selectAction("upgrade", row, latest)} type="button" variant="tonal"><Icon asset={upgradeIcon} slot="icon" />{latest}</Button> : latest ?? "—"}</td>
                                                <td>{row.sourceOptions.length === 0 ? <span className="package-source-missing">No configured source</span> : row.sourceOptions.length === 1 ? row.sourceOptions[0]?.label : <PackageSourceMenu label={`Source for ${row.displayName}`} onChange={(key) => setSourceSelections((current) => ({ ...current, [row.packageId]: key }))} options={row.sourceOptions} value={sourceSelections[row.packageId] ?? ""} />}</td>
                                                <td><div className="package-row-actions"><PackageLinkActions client={client} packageId={row.packageId} target={row.linkTarget} />{row.installedVersion === undefined ? <Button disabled={latest === undefined} onClick={() => selectAction("install", row, latest)} type="button" variant="tonal"><Icon asset={downloadIcon} slot="icon" />Install</Button> : <><Button onClick={() => selectAction("reinstall", row)} type="button" variant="text">Reinstall</Button><Button onClick={() => selectAction("downgrade", row)} type="button" variant="text"><Icon asset={historyIcon} slot="icon" />Versions</Button><Button onClick={() => selectAction("remove", row)} type="button" variant="text"><Icon asset={deleteIcon} slot="icon" />Remove</Button></>}</div></td>
                                            </tr>
                                        );
                                    })}</tbody>
                                </MaterialDataTable>
                            )}
                        </div>
                    </section>
                    <PackageActions client={client} onChanged={refresh} project={project} selection={selection} />
                </section>
            );
        }}</ResourcePage>
    );
}

function packageWorkspaceRows(project: ProjectSnapshot, catalog: WorkspaceCatalogVersion[]): PackageWorkspaceRow[] {
    const catalogByPackage = new Map<string, WorkspaceCatalogVersion[]>();
    for (const item of catalog) {
        const versions = catalogByPackage.get(item.packageId) ?? [];
        versions.push(item);
        catalogByPackage.set(item.packageId, versions);
    }
    const installed = new Map(project.lockedDependencies.map((item) => [item.packageId, item.value]));
    const requested = new Map(project.directDependencies.map((item) => [item.packageId, item.value]));
    const packageIds = new Set([...installed.keys(), ...requested.keys(), ...catalogByPackage.keys()]);
    return [...packageIds].sort((left, right) => left.localeCompare(right)).map((packageId) => {
        const versions = catalogByPackage.get(packageId) ?? [];
        const availableVersions = [...new Set(versions.filter((item) => !item.yanked).map((item) => item.version))].sort((left, right) => left.localeCompare(right));
        const sources = [...new Set(versions.map((item) => item.source))].sort((left, right) => left.localeCompare(right));
        const sourceKinds = [...new Set(versions.map((item) => item.sourceKind))].sort();
        const sourceOptions = [...new Map(versions.map((item) => [item.sourceKey, { key: item.sourceKey, label: item.source, selector: item.sourceSelector }])).values()].sort((left, right) => left.label.localeCompare(right.label));
        const installedVersion = installed.get(packageId);
        const preferredVersion = installedVersion ?? availableVersions.at(-1);
        const linkVersion = versions
            .filter((item): item is WorkspaceCatalogVersion & { repositoryId: string } => item.version === preferredVersion && item.links !== undefined && item.repositoryId !== undefined)
            .sort((left, right) => left.repositoryId.localeCompare(right.repositoryId))[0];
        return {
            availableVersions,
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

function PackageLinkActions({ client, packageId, target }: {
    client: GuiRpcClient;
    packageId: string;
    target?: PackageWorkspaceRow["linkTarget"];
}) {
    const [error, setError] = useState<string>();
    if (target === undefined) return null;
    const open = async (kind: "documentation" | "changelog") => {
        setError(undefined);
        try {
            await client.openPackageLink(target.repositoryId, packageId, target.version, kind);
        } catch (caught: unknown) {
            setError(safeError(caught).code);
        }
    };
    return <>{target.documentation ? <Button onClick={() => void open("documentation")} type="button" variant="text">Docs</Button> : null}{target.changelog ? <Button onClick={() => void open("changelog")} type="button" variant="text">Changelog</Button> : null}{error === undefined ? null : <span className="visually-hidden" role="alert">Package link unavailable: {error}</span>}</>;
}

export function RepositoriesPage({ client, navigate }: PageProps) {
    const load = useCallback(() => client.repositoriesList(), [client]);
    return <Page title="Repositories" eyebrow="VPM sources"><ResourceNavigation navigate={navigate} /><ResourcePage load={load}>{(value, refresh) => <>{value.repositories.length === 0 ? <RouteState kind="empty" title="No repositories registered" /> : <CardList>{value.repositories.map((repository) => <RepositoryCard key={repository.repositoryId ?? sourceText(repository)} repository={repository} navigate={navigate} />)}</CardList>}<RegisterRepositoryPanel client={client} onChanged={refresh} /></>}</ResourcePage></Page>;
}

function ResourceNavigation({ navigate }: { navigate(path: string): void }) {
    return <nav aria-label="Resources" className="resource-navigation"><Button onClick={() => navigate("/repositories")} type="button" variant="text">Repositories</Button><Button onClick={() => navigate("/user-packages")} type="button" variant="text">User Packages</Button><Button onClick={() => navigate("/templates")} type="button" variant="text">Templates</Button></nav>;
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
    const load = useCallback(() => listAllUserPackages(client), [client]);
    return <Page title="User Packages" eyebrow="Local package sources"><ResourceNavigation navigate={navigate} /><ResourcePage load={load}>{(packages, refresh) => <UserPackageManager client={client} onChanged={refresh} packages={packages} />}</ResourcePage></Page>;
}

function UserPackageManager({ client, onChanged, packages }: { client: GuiRpcClient; onChanged(): void; packages: UserPackageRecord[] }) {
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
    const refresh = async (item: UserPackageRecord) => {
        setBusy(`refresh:${item.userPackageId}`);
        setError(undefined);
        try {
            await client.userPackageRefresh(item.userPackageId, item.revision);
            onChanged();
        } catch (caught: unknown) {
            setError(safeError(caught));
        } finally {
            setBusy(undefined);
        }
    };
    const remove = async (item: UserPackageRecord) => {
        setBusy(`remove:${item.userPackageId}`);
        setError(undefined);
        try {
            await client.userPackageRemove(item.userPackageId, item.revision);
            onChanged();
        } catch (caught: unknown) {
            setError(safeError(caught));
        } finally {
            setBusy(undefined);
        }
    };
    return <section className="user-packages"><div className="action-section"><h2>Enrolled folders</h2><p>ALCOMD validates a package folder and creates an immutable cache snapshot. The source folder is never deleted.</p><Button disabled={busy !== undefined} onClick={() => void enroll()} type="button">{busy === "enroll" ? "Enrolling…" : "Enroll folder"}</Button>{error === undefined ? null : <p className="inline-error" role="alert">User Package request failed: {error.code}</p>}</div>{packages.length === 0 ? <RouteState kind="empty" title="No User Packages enrolled" detail="Enroll a loose package directory to make it available as a package source." /> : <CardList>{packages.map((item) => <article className="resource-card" key={item.userPackageId}><h2>{item.displayName ?? item.packageId}</h2><p>{item.packageId} · {item.version}</p><p>Local / User Package · revision {item.revision}</p><p>Archive {item.archiveSha256.slice(0, 12)}…</p><div className="card-actions"><Button disabled={busy !== undefined} onClick={() => void refresh(item)} type="button" variant="text">{busy === `refresh:${item.userPackageId}` ? "Refreshing…" : "Refresh"}</Button><Button disabled={busy !== undefined} onClick={() => void remove(item)} type="button" variant="text">{busy === `remove:${item.userPackageId}` ? "Removing…" : "Remove enrollment"}</Button></div></article>)}</CardList>}</section>;
}

function RepositoryCard({ repository, navigate }: { repository: RepositorySnapshot; navigate(path: string): void }) {
    return <article className="resource-card"><h2>{repository.name ?? repository.declaredId ?? "Repository"}</h2><p>{sourceText(repository)}</p><p>Revision {repository.revision ?? "unregistered"}</p>{repository.repositoryId === undefined ? null : <Button onClick={() => navigate(`/repositories/${repository.repositoryId}`)} type="button" variant="text">Browse packages</Button>}</article>;
}

export function RepositoryDetailPage({ client, repositoryId }: PageProps & { repositoryId: string }) {
    const load = useCallback(async () => Promise.all([client.repositoryGet(repositoryId), client.repositoryPackages(repositoryId)]), [client, repositoryId]);
    return <Page title="Repository" eyebrow={repositoryId}><ResourcePage load={load}>{([detail, catalog], refresh) => <><dl className="detail-grid"><Detail label="Name" value={detail.repository.name ?? "Unnamed"} /><Detail label="Source" value={sourceText(detail.repository)} /><Detail label="Revision" value={String(detail.repository.revision ?? "—")} /><Detail label="Issues" value={String(detail.repository.issues.length)} /></dl><h2>Packages</h2><DataTable headers={["Package", "Version", "Status"]} rows={catalog.packages.map((item) => [item.displayName ?? item.packageId, item.version, item.yanked ? "Yanked" : "Available"])} /><RepositoryActions client={client} onChanged={refresh} repository={detail.repository} /></>}</ResourcePage></Page>;
}

export function TemplatesPage({ client, navigate }: PageProps) {
    const load = useCallback(() => client.templatesList(), [client]);
    return <Page title="Templates" eyebrow="Project starters"><ResourceNavigation navigate={navigate} /><ResourcePage load={load}>{(value, refresh) => <>{value.templates.length === 0 ? <RouteState kind="empty" title="No templates available" /> : <CardList>{value.templates.map((template) => <TemplateCard key={template.templateId} template={template} navigate={navigate} />)}</CardList>}<TemplateImportPanel client={client} onChanged={refresh} /></>}</ResourcePage></Page>;
}

function TemplateCard({ template, navigate }: { template: TemplateRecord; navigate(path: string): void }) {
    return <article className="resource-card"><h2>{template.displayName}</h2><p>{template.description ?? "No description"}</p><p>{template.sourceKind} · v{template.templateVersion}{template.favorite ? " · Favorite" : ""}</p><Button onClick={() => navigate(`/templates/${template.templateId}`)} type="button" variant="text">View template</Button></article>;
}

export function TemplateDetailPage({ client, templateId }: PageProps & { templateId: string }) {
    const load = useCallback(() => client.templateGet(templateId), [client, templateId]);
    return <Page title="Template detail" eyebrow={templateId}><ResourcePage load={load}>{({ template }, refresh) => <><dl className="detail-grid"><Detail label="Name" value={template.displayName} /><Detail label="Version" value={template.templateVersion} /><Detail label="Source" value={template.sourceKind} /><Detail label="Revision" value={String(template.revision)} /><Detail label="Provenance" value={template.provenance} /><Detail label="Favorite" value={template.favorite ? "Yes" : "No"} /></dl><TemplateActions client={client} onChanged={refresh} template={template} /></>}</ResourcePage></Page>;
}

export function UnityPage({ client }: PageProps) {
    const load = useCallback(() => client.unityInstallationsList(), [client]);
    return <Page title="Unity" eyebrow="Editor installations"><ResourcePage load={load}>{(value, refresh) => <>{value.installations.length === 0 ? <RouteState kind="empty" title="No Unity installations registered" /> : <CardList>{value.installations.map((item) => <UnityCard installation={item} key={item.installationId} />)}</CardList>}<UnityRegistryActions client={client} installations={value.installations} onChanged={refresh} /></>}</ResourcePage></Page>;
}

function UnityCard({ installation }: { installation: UnityInstallation }) {
    return <article className="resource-card"><h2>Unity {installation.unityVersion}</h2><p>{installation.architecture} · {installation.sourceKind}</p><p className="private-value" title="Private path hidden">Executable path is stored by the daemon</p></article>;
}

export function ProjectUnityPage({ client, projectId }: PageProps & { projectId: string }) {
    const load = useCallback(async () => {
        const [project, installations, selection, writer] = await Promise.all([
            client.projectGet(projectId),
            loadAllUnityInstallations(client),
            client.unityProjectEditorSelectionGet(projectId),
            client.unityWriterState(projectId).catch(() => undefined)
        ]);
        return { project: project.project, installations, selection: selection.preference, writer };
    }, [client, projectId]);
    return <Page title="Project Unity" eyebrow={projectId}><ResourcePage load={load}>{({ project, installations, selection, writer }, refresh) => <><dl className="detail-grid"><Detail label="Selected editor" value={editorSelectionLabel(selection, installations)} /><Detail label="Writer observation" value={writer?.state ?? "Unknown"} /><Detail label="Arguments" value={selection.arguments.length === 0 ? "Default" : `${selection.arguments.length} configured arguments`} /><Detail label="Observed" value={writer === undefined ? "—" : formatTime(writer.checkedAtMs)} /></dl><ProjectUnityActions client={client} installations={installations} onChanged={refresh} project={project} selection={selection} /></>}</ResourcePage></Page>;
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

function editorSelectionLabel(selection: ProjectEditorSelectionState, installations: UnityInstallation[]): string {
    if (selection.selection.mode === "automatic") return "Automatic";
    const installationId = selection.selection.installationId;
    const installation = installations.find((item) => item.installationId === installationId);
    return installation === undefined ? "Selected editor unavailable" : `Unity ${installation.unityVersion} (${installation.architecture})`;
}

export function ProjectBackupsPage({ client, navigate, projectId }: PageProps & { projectId: string }) {
    const load = useCallback(async () => ({ backups: await client.backupsList(projectId), project: (await client.projectGet(projectId)).project }), [client, projectId]);
    return <Page title="Backups" eyebrow={`Project ${projectId}`}><ResourcePage load={load}>{(value, refresh) => <>{value.backups.backups.length === 0 ? <RouteState kind="empty" title="No backups for this project" /> : <CardList>{value.backups.backups.map((backup) => <BackupCard backup={backup} key={backup.backupId} navigate={navigate} />)}</CardList>}<BackupCreatePanel client={client} onChanged={refresh} project={value.project} /></>}</ResourcePage></Page>;
}

function BackupCard({ backup, navigate }: { backup: BackupRecord; navigate(path: string): void }) {
    return <article className="resource-card"><h2>{formatTime(backup.createdAtMs)}</h2><p>{formatBytes(backup.archiveBytes)} · {backup.compressionMode}</p><p>{backup.excludeVpmPackages ? "VPM packages excluded" : "VPM packages included"}</p><Button onClick={() => navigate(`/backups/${backup.backupId}`)} type="button" variant="text">View backup</Button></article>;
}

export function BackupDetailPage({ client, backupId }: PageProps & { backupId: string }) {
    const load = useCallback(() => client.backupGet(backupId), [client, backupId]);
    return <Page title="Backup detail" eyebrow={backupId}><ResourcePage load={load}>{(backup) => <><dl className="detail-grid"><Detail label="Source project" value={backup.sourceProjectId} /><Detail label="Created" value={formatTime(backup.createdAtMs)} /><Detail label="Archive size" value={formatBytes(backup.archiveBytes)} /><Detail label="Format" value={`v${backup.formatVersion}`} /><Detail label="Compression" value={backup.compressionMode} /><Detail label="Integrity" value={shortHash(backup.archiveSha256)} /></dl><BackupRestorePanel backup={backup} client={client} /></>}</ResourcePage></Page>;
}

export function OperationsPage({ client, navigate }: PageProps) {
    const load = useCallback(() => client.operationsList(), [client]);
    return <Page title="Operations" eyebrow="Durable work"><ResourcePage load={load} empty={(value) => value.operations.length === 0} emptyTitle="No operations yet">{(value) => <CardList>{value.operations.map((operation) => <OperationCard key={operation.operationId} operation={operation} navigate={navigate} />)}</CardList>}</ResourcePage></Page>;
}

function OperationCard({ operation, navigate }: { operation: Operation; navigate(path: string): void }) {
    return <article className="resource-card"><h2>{humanize(operation.kind)}</h2><p className={`state-chip state-chip--${operation.state}`}>{humanize(operation.state)}</p><p>{operation.progress?.phase === undefined ? "No phase reported" : humanize(operation.progress.phase)}</p><Button onClick={() => navigate(`/operations/${operation.operationId}`)} type="button" variant="text">View operation</Button></article>;
}

export function OperationDetailPage({ client, operationId }: PageProps & { operationId: string }) {
    const load = useCallback(() => client.operationGet(operationId), [client, operationId]);
    return <Page title="Operation detail" eyebrow={operationId}><ResourcePage load={load}>{(operation, refresh) => <><dl className="detail-grid"><Detail label="Kind" value={operation.kind} /><Detail label="State" value={operation.state} /><Detail label="Phase" value={operation.progress?.phase ?? "—"} /><Detail label="Revision" value={String(operation.revision)} /><Detail label="Updated" value={formatTime(operation.updatedAtMs)} /><Detail label="Error" value={operation.errorCode ?? "None"} /></dl>{operation.diagnosticId === undefined ? null : <p>Diagnostic ID: <code>{operation.diagnosticId}</code></p>}<OperationActions client={client} onChanged={refresh} operation={operation} /></>}</ResourcePage></Page>;
}

export function ExtensionsPage({ client, navigate }: PageProps) {
    const load = useCallback(() => client.extensionsList(), [client]);
    return <Page title="Extensions" eyebrow="First-party and third-party use the same contract"><ResourcePage load={load}>{(value, refresh) => <>{value.extensions.length === 0 ? <RouteState kind="empty" title="No extensions installed" /> : <CardList>{value.extensions.map((extension) => <ExtensionCard extension={extension} key={extension.extensionId} navigate={navigate} />)}</CardList>}<ExtensionInstallPanel client={client} onChanged={refresh} /></>}</ResourcePage></Page>;
}

function ExtensionCard({ extension, navigate }: { extension: ExtensionRecord; navigate(path: string): void }) {
    return <article className="resource-card"><h2>{extension.extensionId}</h2><p>v{extension.version} · {humanize(extension.trustDecision)}</p><p>{humanize(extension.desiredState)} · {humanize(extension.runtimeState)}</p><Button onClick={() => navigate(`/extensions/${extension.extensionId}`)} type="button" variant="text">Manage extension</Button></article>;
}

export function ExtensionDetailPage({ client, navigate, extensionId }: PageProps & { extensionId: string }) {
    const load = useCallback(() => client.extensionGet(extensionId), [client, extensionId]);
    return <Page title="Extension detail" eyebrow={extensionId}><ResourcePage load={load}>{({ extension }, refresh) => <><dl className="detail-grid"><Detail label="Version" value={extension.version} /><Detail label="Publisher" value={shortHash(extension.publisherFingerprint)} /><Detail label="Trust" value={humanize(extension.trustDecision)} /><Detail label="Desired state" value={humanize(extension.desiredState)} /><Detail label="Runtime" value={humanize(extension.runtimeState)} /><Detail label="Quarantine" value={humanize(extension.quarantineState)} /><Detail label="Grant revision" value={String(extension.grantRevision)} /><Detail label="Record revision" value={String(extension.revision)} /></dl>{extension.ui?.protocol === "portable-v1" ? <Button onClick={() => navigate(`/extensions/${extensionId}/ui`)} type="button">Open Portable UI</Button> : <RouteState kind="empty" title="This extension has no Portable UI" />}<ExtensionActions client={client} extension={extension} onChanged={refresh} /></>}</ResourcePage></Page>;
}

export function AboutPage({ client }: PageProps) {
    const load = useCallback(() => client.systemStatus(), [client]);
    return <Page title="About" eyebrow="ALCOMD platform"><ResourcePage load={load}>{(status) => <><dl className="detail-grid"><Detail label="Product" value={status.product} /><Detail label="Daemon" value={status.daemonVersion} /><Detail label="RPC" value={`v${status.rpcVersion}`} /><Detail label="State" value={status.state} /></dl><section className="notice-card"><h2>Licenses</h2><p>ALCOMD source is licensed under AGPL-3.0-only.</p><p>The canonical third-party dependency notice source is <code>THIRD_PARTY_NOTICES.md</code> in the product distribution.</p></section></>}</ResourcePage></Page>;
}

export function ActivityPage({ client, navigate }: PageProps) {
    const load = useCallback(() => client.activityList(), [client]);
    return (
        <Page title="Activity" eyebrow="Redacted history">
            <ResourcePage load={load} empty={(value) => value.items.length === 0} emptyTitle="No activity yet">
                {(value) => <CardList>{value.items.map((item) => <ActivityCard item={item} key={`${item.type}-${item.operationId ?? item.eventSequence ?? item.occurredAtMs}`} navigate={navigate} />)}</CardList>}
            </ResourcePage>
        </Page>
    );
}

function ActivityCard({ item, navigate }: { item: ActivityItem; navigate(path: string): void }) {
    return (
        <article className="resource-card">
            <h2>{humanize(item.summaryCode)}</h2>
            <p>{humanize(item.type)} · {formatTime(item.occurredAtMs)}</p>
            {item.state === undefined ? null : <p className={`state-chip state-chip--${item.state}`}>{humanize(item.state)}</p>}
            {item.operationId === undefined ? null : <Button onClick={() => navigate(`/operations/${item.operationId}`)} type="button" variant="text">View operation</Button>}
        </article>
    );
}

export function DiagnosticsPage({ client }: PageProps) {
    const load = useCallback(() => client.diagnosticsList(), [client]);
    const [operationId, setOperationId] = useState<string>();
    const [checkError, setCheckError] = useState<RpcError>();
    const [checking, setChecking] = useState(false);
    const runStateCheck = async () => {
        setChecking(true);
        setCheckError(undefined);
        try {
            const result = await client.stateCheck();
            setOperationId(result.operationId);
        } catch (caught: unknown) {
            setCheckError(safeError(caught));
        } finally {
            setChecking(false);
        }
    };
    return (
        <Page title="Diagnostics" eyebrow="Redacted technical status">
            <p className="supporting-copy">This view excludes raw logs, stack traces, process details, credentials, and private paths.</p>
            <section className="action-section">
                <h2>State integrity</h2>
                <p>Start the daemon-owned read-only integrity check and follow its durable Operation.</p>
                <Button disabled={checking} onClick={() => void runStateCheck()} type="button">{checking ? "Starting…" : "Run state check"}</Button>
                {checkError === undefined ? null : <p className="inline-error" role="alert"><code>{checkError.code}</code> — the state check could not be started.</p>}
                {operationId === undefined ? null : <OperationFollow client={client} operationId={operationId} />}
            </section>
            <ResourcePage load={load} empty={(value) => value.items.length === 0} emptyTitle="No diagnostics reported">
                {(value) => <CardList>{value.items.map((item) => <DiagnosticCard item={item} key={`${item.operationId ?? "diagnostic"}-${item.occurredAtMs}`} />)}</CardList>}
            </ResourcePage>
        </Page>
    );
}

function DiagnosticCard({ item }: { item: DiagnosticItem }) {
    return (
        <article className="resource-card">
            <h2>{humanize(item.code)}</h2>
            <p>{item.summary}</p>
            <dl className="detail-grid">
                <Detail label="Subsystem" value={item.subsystem} />
                <Detail label="Severity" value={humanize(item.severity)} />
                <Detail label="Occurred" value={formatTime(item.occurredAtMs)} />
                <Detail label="Diagnostic ID" value={item.diagnosticId ?? "Not available"} />
            </dl>
        </article>
    );
}

export function SettingsPage({
    client,
    onApplied,
    onDirtyChange
}: PageProps & {
    onApplied(value: SettingsGetResult): void;
    onDirtyChange(dirty: boolean): void;
}) {
    const load = useCallback(() => client.settingsGet(), [client]);
    return (
        <Page title="Settings" eyebrow="Appearance and language">
            <ResourcePage load={load}>
                {(value, refresh) => (
                    <SettingsForm
                        client={client}
                        key={value.revision}
                        onApplied={(next) => { onApplied(next); refresh(); }}
                        onDirtyChange={onDirtyChange}
                        snapshot={value}
                    />
                )}
            </ResourcePage>
        </Page>
    );
}

function SettingsForm({
    client,
    onApplied,
    onDirtyChange,
    snapshot
}: {
    client: GuiRpcClient;
    onApplied(value: SettingsGetResult): void;
    onDirtyChange(dirty: boolean): void;
    snapshot: SettingsGetResult;
}) {
    const [settings, setSettings] = useState<OfficialSettings>(snapshot.settings);
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState<RpcError>();
    const [repositories, setRepositories] = useState<RepositorySnapshot[]>([]);
    const dirty = JSON.stringify(settings) !== JSON.stringify(snapshot.settings);

    useEffect(() => {
        onDirtyChange(dirty);
        return () => onDirtyChange(false);
    }, [dirty, onDirtyChange]);

    useEffect(() => {
        let active = true;
        void listAllRepositories(client).then((value) => {
            if (active) setRepositories(value);
        }).catch(() => undefined);
        return () => { active = false; };
    }, [client]);

    const updateAppearance = <Key extends keyof OfficialSettings["appearance"]>(
        key: Key,
        value: OfficialSettings["appearance"][Key]
    ) => setSettings((current) => ({
        ...current,
        appearance: { ...current.appearance, [key]: value }
    }));
    const updatePackages = <Key extends keyof OfficialSettings["packages"]>(
        key: Key,
        value: OfficialSettings["packages"][Key]
    ) => setSettings((current) => ({
        ...current,
        packages: { ...current.packages, [key]: value }
    }));
    const setRepositoryHidden = (repositoryId: string, hidden: boolean) => {
        const values = new Set(settings.packages.hiddenRepositoryIds);
        if (hidden) values.add(repositoryId);
        else values.delete(repositoryId);
        updatePackages("hiddenRepositoryIds", [...values].sort());
    };

    const save = async (event: FormEvent<HTMLFormElement>) => {
        event.preventDefault();
        if (!dirty || busy) return;
        setBusy(true);
        setError(undefined);
        try {
            const updated = await client.settingsUpdate(snapshot.revision, settings);
            onDirtyChange(false);
            onApplied(updated);
        } catch (caught: unknown) {
            setError(safeError(caught));
        } finally {
            setBusy(false);
        }
    };

    return (
        <form className="action-panel settings-form" onSubmit={(event) => void save(event)}>
            <Select id="settings-theme" label="Theme" onChange={(next) => updateAppearance("mode", next as OfficialSettings["appearance"]["mode"])} options={[{ label: "System", value: "system" }, { label: "Light", value: "light" }, { label: "Dark", value: "dark" }]} value={settings.appearance.mode} />
            <Select id="settings-color" label="Source color" onChange={(next) => updateAppearance("sourceColor", next || null)} options={[{ label: "Product default", value: "" }, { label: "Violet", value: "#6750A4" }, { label: "Blue", value: "#315DA8" }, { label: "Teal", value: "#006A60" }]} supportingText="Saved as a canonical #RRGGBB value; extensions never receive this preference." value={settings.appearance.sourceColor ?? ""} />
            <Select id="settings-density" label="Density" onChange={(next) => updateAppearance("density", next as OfficialSettings["appearance"]["density"])} options={[{ label: "Default", value: "default" }, { label: "Compact", value: "compact" }]} value={settings.appearance.density} />
            <Select id="settings-motion" label="Motion" onChange={(next) => updateAppearance("motion", next as OfficialSettings["appearance"]["motion"])} options={[{ label: "System preference", value: "system" }, { label: "Reduce motion", value: "reduced" }]} value={settings.appearance.motion} />
            <Select id="settings-locale" label="Language" onChange={(next) => {
                const locale = next as SettingsLocale;
                setSettings((current) => ({ ...current, locale }));
            }} options={[{ label: "System language", value: "system" }, { label: "English", value: "en-US" }, { label: "简体中文", value: "zh-CN" }, { label: "日本語", value: "ja-JP" }]} value={settings.locale} />
            <fieldset className="settings-package-options">
                <legend>Packages</legend>
                <Checkbox checked={settings.packages.showPrerelease} label="Show prerelease package versions" onChange={(checked) => updatePackages("showPrerelease", checked)} />
                <Checkbox checked={settings.packages.hideLocalUserPackages} label="Hide local User Packages" onChange={(checked) => updatePackages("hideLocalUserPackages", checked)} />
                {repositories.map((repository) => repository.repositoryId === undefined ? null : (
                    <Checkbox
                        checked={settings.packages.hiddenRepositoryIds.includes(repository.repositoryId)}
                        key={repository.repositoryId}
                        label={`Hide ${repository.name ?? repository.declaredId ?? repository.repositoryId}`}
                        onChange={(checked) => setRepositoryHidden(repository.repositoryId as string, checked)}
                    />
                ))}
            </fieldset>
            {error === undefined ? null : <p className="form-error" role="alert">{error.code === "revision_conflict" ? "Settings changed elsewhere. Reload and try again." : "Settings could not be saved."}</p>}
            <div className="action-row">
                <Button disabled={!dirty || busy} type="submit">{busy ? "Saving…" : "Save settings"}</Button>
                <Button disabled={!dirty || busy} onClick={() => setSettings(snapshot.settings)} type="button" variant="tonal">Discard changes</Button>
            </div>
            <p aria-live="polite" className="field-hint">Config Schema {snapshot.configSchema} · revision {snapshot.revision}</p>
        </form>
    );
}

export function Page({ children, eyebrow, title }: { children: ReactNode; eyebrow: string; title: string }) {
    return <section className="page-surface" aria-labelledby="route-title"><header className="page-header"><p className="eyebrow">{eyebrow}</p><h1 id="route-title" tabIndex={-1}>{title}</h1></header>{children}</section>;
}

function CardList({ children }: { children: ReactNode }) { return <div className="card-list">{children}</div>; }

function Detail({ label, value }: { label: string; value: string }) { return <div><dt>{label}</dt><dd>{value}</dd></div>; }

function DataTable({ headers, rows }: { headers: string[]; rows: string[][] }) {
    if (rows.length === 0) return <RouteState kind="empty" title="No matching items" />;
    return <div className="table-scroll"><table><thead><tr>{headers.map((header) => <th key={header} scope="col">{header}</th>)}</tr></thead><tbody>{rows.map((row, rowIndex) => <tr key={`${rowIndex}-${row[0]}`}>{row.map((cell, index) => <td key={`${headers[index]}-${index}`}>{cell}</td>)}</tr>)}</tbody></table></div>;
}

function sourceText(repository: RepositorySnapshot): string { return repository.source.kind === "local" ? "Local repository" : repository.source.url; }
function formatTime(value: number): string { return new Intl.DateTimeFormat("en-US", { dateStyle: "medium", timeStyle: "short", timeZone: "UTC" }).format(new Date(value)); }
function formatBytes(value: number): string { return value < 1024 * 1024 ? `${Math.ceil(value / 1024)} KiB` : `${(value / (1024 * 1024)).toFixed(1)} MiB`; }
function shortHash(value: string): string { return value.length <= 24 ? value : `${value.slice(0, 16)}…${value.slice(-8)}`; }
function humanize(value: string): string { return value.replaceAll("_", " "); }

function safeError(caught: unknown): RpcError {
    if (typeof caught === "object" && caught !== null && "code" in caught && typeof caught.code === "string") {
        return { code: caught.code, message: "The request could not be completed.", ...("diagnosticId" in caught && typeof caught.diagnosticId === "string" ? { diagnosticId: caught.diagnosticId } : {}) };
    }
    return { code: "internal_error", message: "The request could not be completed." };
}
