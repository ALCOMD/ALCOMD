import { Translated, useTranslation } from "./i18n";
import { backupCompressionLabel } from "./backup-compression";
import { productFamily } from "@alcomd/ui";
import { guiVersion } from "./build-info";
import { OperationItems } from "./OperationItems";
import { useCoreEvents, useCoreInvalidation, useViewState, useViewReadingPosition, useWorkspaceInvalidation } from "./core-events-ui";
import type { ChangeScope, CoreEvents } from "./core-events";
import type { SupportedLocale, Translate } from "./i18n-types";
import type { CandidateBulkAction, CandidateBulkActions, CandidateBulkIntent, CandidateCursor, CandidateFilter, PackageCandidateEvidence, PackageCandidateSummary, ProjectCandidateRequest, RpcError } from "@alcomd/sdk";
import { actionableVersion, bulkPlanIntent, candidateSource, candidateSourceKey, finishCandidateSummary, queryCandidates, readCandidateSummaryPage, reasonText, updateReason, validateVersionPage, type CandidateSummaryPage, type CandidateSummarySet } from "./package-candidates";
import { orderPackageWorkspaceRows } from "./package-workspace-order";
import {
    accountCircleIcon,
    arrowBackIcon,
    arrowDownwardIcon,
    arrowUpwardIcon,
    closeIcon,
    cancelIcon,
    downloadIcon,
    helpIcon,
    moreVertIcon,
    packagesIcon,
    publicIcon,
    starIcon,
    upgradeIcon,
    viewGridIcon,
    viewListIcon
} from "@alcomd/ui/icons";
import { useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { flushSync } from "react-dom";

import type {
    OfficialSettings,
    BackupCreateResult,
    Operation,
    PackageSourceSelector,
    ProjectCopyPlan,
    ProjectDeletePlan,
    ProjectListSort,
    ProjectSnapshot,
    RegistryCursor,
    RepositorySnapshot,
    UnityInstallation,
    UnityLaunchOptionsResult,
    UserPackageRecord,
    SettingsGetResult
} from "./core-models";
import {
    BackupCreatePanel,
    ExtensionActions,
    ExtensionInstallPanel,
    OperationActions,
    OperationFollow,
    OperationProgress,
    PackageActions,
    type PackageActionSelection,
    ProjectLaunchArgumentsDialog,
    ProjectUnityActions,
    ProjectUnityWorkspaceActions,
    RepositoryActions,
    useActionScope,
    UnityRegistryActions
} from "./CoreActions";
import { DesktopList, type DesktopListColumn } from "./DesktopList";
import { displayChildPath, displayPath } from "./display-path";
import { FilePicker } from "./FilePicker";
import { useProjectDirectory } from "./use-project-directory";
import { ScrollArea } from "./ScrollArea";
import { PageHeader } from "./PageHeader";
import { LoadingState, PageLoadingState, useDelayedPending } from "./LoadingState";
import { LocalPageReadContext, PageReadContext, primePageRead, useInitialPageRead, usePageReadPromise, type createPageRead, type PageReadResult } from "./PageRead";
import { navigationLabels } from "./navigation-labels";
import { UtilityActionDialog } from "./UtilityActionDialog";
import { UtilityWorkspace } from "./UtilityWorkspace";
import "./ExtensionWorkspace.css";
import { SettingsWorkspace } from "./SettingsWorkspace";
import { PagedItems } from "./PagedItems";
import type { GuiRpcClient } from "./rpc";
import { DialogCloseButton, Button, Checkbox, Dialog, FilterPopover, Icon, IconButton, Menu, MenuItem, RefreshButton, SearchField, SegmentedButtons, Select, Switch, TextField, type DialogOrigin } from "./Material";
import { SplitButton } from "./SplitButton";
import { RepositoryToolbar } from "./RepositoryListDialog";
import { isBuiltinRepository, repositoryDisplayName } from "./repository-display";
import { useProjectUnityLaunch, type ProjectUnityLaunchAction } from "./ProjectUnityLaunch";
import { ProjectTemplatesWorkspace } from "./ProjectTemplatesWorkspace";
import { CreateProjectDialog, RestoreProjectDialog } from "./ProjectCreationDialogs";
import { capabilities, capabilityUnavailableTitle, useCapability, useCapabilityState, useReconnect } from "./capabilities";
import { normalizeRequestError, requestErrorMessage } from "./request-errors";

interface PageProps {
    client: GuiRpcClient;
    navigate(path: string): void;
}

interface ResourcePageProps<T> {
    load(): Promise<T>;
    reload?(previous: T): Promise<T>;
    children(value: T, refresh: () => void, refreshing: boolean, error?: RpcError): ReactNode;
    empty?(value: T): boolean;
    emptyTitle?: string;
    changes?: readonly ChangeScope[];
}

function ResourcePage<T>({ load: sourceLoad, reload, children, empty, emptyTitle = "Nothing here yet", changes = [] }: ResourcePageProps<T>) {
    const { t, locale } = useTranslation();
    const localRead = useContext(LocalPageReadContext);
    const events = useCoreEvents();
    const scopeKey = changes.join("|");
    const load = useCallback((previous?: T) => {
        const read = () => previous !== undefined && reload ? reload(previous) : sourceLoad();
        return events && scopeKey ? events.consistentRead(scopeKey.split("|"), read) : read();
    }, [events, scopeKey, sourceLoad, reload]);
    const initial = useInitialPageRead(load);
    const initialLoader = useRef(load).current;
    const waiters = useRef<Array<{ resolve(): void; reject(error: unknown): void }>>([]);
    const [state, setState] = useState<{
        value?: T;
        error?: RpcError;
        loading: boolean;
        refreshing: boolean;
        generation: number;
    }>(() => ({ loading: initial === undefined, refreshing: false, generation: 0,
        ...(initial?.kind === "ready" ? { value: initial.value } : initial?.kind === "error" ? { error: safeError(t, locale, initial.error) } : {}) }));

    useEffect(() => {
        if (initial !== undefined && state.generation === 0 && load === initialLoader) return;
        let active = true;
        setState((current) => ({
            ...current,
            error: undefined,
            loading: current.value === undefined,
            refreshing: current.value !== undefined
        }));
        void load(state.value).then((value) => {
            if (active) {
                setState((current) => ({ ...current, value, error: undefined, loading: false, refreshing: false }));
                waiters.current.splice(0).forEach((waiter) => waiter.resolve());
            }
        }).catch((caught: unknown) => {
            if (active) {
                waiters.current.splice(0).forEach((waiter) => waiter.reject(caught));
                setState((current) => ({
                    ...current,
                    error: safeError(t, locale, caught),
                    loading: false,
                    refreshing: false
                }));
            }
        });
        return () => { active = false; };
    }, [load, state.generation, initialLoader, initial]);

    const readAgain = () => setState((current) => ({ ...current, generation: current.generation + 1 }));
    const retryRead = useWorkspaceInvalidation(changes, () => new Promise<void>((resolve, reject) => { waiters.current.push({ resolve, reject }); readAgain(); }));
    const refresh = () => { if (events) retryRead(); else readAgain(); };
    useEffect(() => { if (state.value !== undefined) events?.noteDisplayedData(); }, [events, state.value]);
    useEffect(() => () => { waiters.current.splice(0).forEach((waiter) => waiter.resolve()); }, []);
    if (state.loading) {
        return localRead ? <LoadingState title={t("Loading")} detail={t("Reading the current ALCOMD state…")} />
            : <PageLoadingState title={t("Loading")} detail={t("Reading the current ALCOMD state…")} />;
    }
    if (state.error !== undefined && state.value === undefined) {
        return <ErrorState error={state.error} retry={refresh} />;
    }
    if (state.value === undefined) {
        return <RouteState kind="error" title={t("No response")} />;
    }
    if (empty?.(state.value) === true) {
        return <RouteState kind="empty" title={t(emptyTitle)} />;
    }
    return children(state.value, refresh, state.refreshing, state.error);
}

export function RouteState({ kind, title, detail }: { kind: "loading" | "empty" | "error" | "disconnected"; title: string; detail?: string }) {
    if (kind === "loading") return <LoadingState title={title} detail={detail} />;
    return (
        <section className={`route-state route-state--${kind}`} role={kind === "error" ? "alert" : "status"} aria-live="polite">
            <h2>{title}</h2>
            {detail === undefined ? null : <p>{detail}</p>}
        </section>
    );
}

function ErrorState({ error, retry }: { error: RpcError; retry(): void }) {
    const { t, locale } = useTranslation();
    const reconnect = useReconnect();
    const disconnected = error.code === "daemon_unavailable";
    return (
        <section className={`route-state route-state--${disconnected ? "disconnected" : "error"}`}>
            <h2>{disconnected ? t("ALCOMD core disconnected") : t("Request failed")}</h2>
            <div role="alert"><p>{t(requestErrorMessage((error).code))}</p><code>{(error).code}</code>{error.diagnosticId === undefined ? null : <p><code>{error.diagnosticId}</code></p>}</div>
            <Button onClick={() => { if (disconnected) reconnect?.(); retry(); }} type="button">{t("Reconnect and retry")}</Button>
        </section>
    );
}

export function HomePage({ client, navigate }: PageProps) {
    const { t, locale } = useTranslation();
    const load = useCallback(() => client.systemStatus(), [client]);
    return (
        <UtilityWorkspace title={t("Home")} load={load}>{(status) => (
                <div className="dashboard-grid">
                    <article className="summary-card"><h2>{t("ALCOMD core")}</h2><p className="status-value">{humanize(t, status.state)}</p><p>{t("Daemon ")}{status.daemonVersion}</p></article>
                    <article className="summary-card"><h2>{t("Protocol")}</h2><p className="status-value">{t("RPC v")}{status.rpcVersion}</p><p>{status.capabilities.length} {t(" capabilities negotiated")}</p></article>
                    <article className="summary-card"><h2>{t("Get started")}</h2><Button onClick={() => navigate("/projects")} type="button" variant="text">{t("Open projects")}</Button></article>
                </div>
            )}</UtilityWorkspace>
    );
}

const projectsViewStorageKey = "alcomd.gui.projects.view";

export function ProjectsPage({ client, navigate }: PageProps) {
    const coreEvents = useCoreEvents();
    const root = useRef<HTMLElement>(null);
    const { t, locale } = useTranslation();
    const canRegister = useCapability(capabilities.projectsRegistry);
    const canCreate = useCapability(capabilities.projectsCreate);
    const canRestore = useCapability(capabilities.backupsRestore);
    const [search, setSearch] = useViewState("projects", "search", "");
    const [sort, setSort] = useViewState<ProjectListSort>("projects", "sort", "modified");
    const [descending, setDescending] = useViewState("projects", "descending", true);
    const load = useCallback(async () => {
        const read = async () => {
            const projects: ProjectSnapshot[] = [];
            const view = { search, sort, descending };
            let cursor: RegistryCursor | undefined;
            let sequence: number | undefined;
            do {
                const value = await client.projectsList(cursor, view, sequence);
                projects.push(...value.projects);
                cursor = value.nextCursor;
                sequence = value.viewSequence;
            } while (cursor !== undefined);
            return projects;
        };
        const readCurrent = async () => {
            try {
                return await read();
            } catch (error) {
                if (typeof error !== "object" || error === null || !("code" in error) || error.code !== "revision_conflict") throw error;
                return read();
            }
        };
        return coreEvents ? coreEvents.consistentRead(["projects"], readCurrent) : readCurrent();
    }, [client, coreEvents, search, sort, descending]);
    const initial = usePageReadPromise("primary", load);
    const [state, setState] = useState<{
        error?: RpcError;
        loaded: boolean;
        loading: boolean;
        projects: ProjectSnapshot[];
    }>({ loaded: false, loading: true, projects: [] });
    const [view, setView] = useState<"list" | "grid">(() => {
        try {
            return window.localStorage.getItem(projectsViewStorageKey) === "grid" ? "grid" : "list";
        } catch {
            return "list";
        }
    });
    const [registerOpen, setRegisterOpen] = useState(false);
    const [createOpen, setCreateOpen] = useState(false);
    const [restoreOpen, setRestoreOpen] = useState(false);
    const addProjectMenuRef = useRef<HTMLElement>(null);
    const createButtonRef = useRef<HTMLElement>(null);
    const listReadGeneration = useRef(0);
    const refreshingList = useRef(false);
    const queuedList = useRef(false);
    const listError = useRef<unknown>(undefined);
    const loadRef = useRef(load);
    const lastViewLoad = useRef(load);
    const initialLoad = useRef(load).current;
    loadRef.current = load;
    // The toolbar does not depend on project rows. Share the visit's first read,
    // but allow it to settle after the route commits; newer filters/reads win.
    useEffect(() => {
        let active = true;
        const generation = listReadGeneration.current;
        void initial.then((result) => {
            if (!active || listReadGeneration.current !== generation || loadRef.current !== initialLoad) return;
            setState(result.kind === "ready" ? { loaded: true, loading: false, projects: result.value }
                : { loaded: false, loading: false, projects: [], error: normalizeRequestError(result.error) });
        });
        return () => { active = false; };
    }, [initial, initialLoad]);
    useEffect(() => () => {
        listReadGeneration.current += 1;
        queuedList.current = false;
    }, [client]);

    const toggleView = () => {
        const next = view === "list" ? "grid" : "list";
        setView(next);
        try {
            window.localStorage.setItem(projectsViewStorageKey, next);
        } catch {
            // This disposable preference must not prevent switching views.
        }
    };

    const refresh = useCallback(async () => {
        if (refreshingList.current) { queuedList.current = true; listReadGeneration.current++; return; }
        refreshingList.current = true;
        listError.current = undefined;
        const request = ++listReadGeneration.current;
        const read = loadRef.current;
        try {
            const projects = await read();
            if (request === listReadGeneration.current && read === loadRef.current) setState({ loaded: true, loading: false, projects });
        } catch (caught: unknown) {
            if (request === listReadGeneration.current && read === loadRef.current) {
                listError.current = caught;
                const failure = safeError(t, locale, caught);
                setState((current) => ({ ...current, error: current.error?.code === failure.code ? current.error : failure, loading: false }));
            }
        } finally {
            refreshingList.current = false;
            if (queuedList.current) { queuedList.current = false; void refresh(); }
        }
    }, [t, locale]);

    useEffect(() => {
        if (lastViewLoad.current === load) return;
        const timer = window.setTimeout(() => {
            lastViewLoad.current = load;
            void refresh();
        }, 100);
        return () => window.clearTimeout(timer);
    }, [load, refresh]);

    const retryProjects = useCoreInvalidation("projects:list", ["projects"], async () => { await refresh(); if (listError.current !== undefined) throw listError.current; });
    useViewReadingPosition("projects", root, state.projects, !state.loading);
    useEffect(() => {
        if (state.loading) return;
        coreEvents?.noteDisplayedData(); coreEvents?.writeView("projects", state.projects, state.projects.length);
    }, [coreEvents, state.projects, state.loading]);

    const projects = state.projects;
    const updateProject = () => { void refresh(); };

    const updateSort = (nextSort: ProjectListSort) => {
        if (sort === nextSort) {
            setDescending((current) => !current);
            return;
        }
        setSort(nextSort);
        setDescending(nextSort === "added" || nextSort === "modified");
    };

    return <>{(
        <section className="projects-page" ref={root}>
            <PageHeader className="projects-toolbar" title={t(navigationLabels.projects)}>
                <SearchField className="projects-search" label={t("Search projects")} onInput={setSearch} placeholder={t("Search...")} value={search} />
                <Button onClick={toggleView} type="button" variant="text">
                    <Icon asset={view === "list" ? viewListIcon : viewGridIcon} slot="icon" />
                    <StateSizedLabel current={view === "list" ? t("List view") : t("Grid view")} labels={[t("List view"), t("Grid view")]} />
                </Button>
                <SplitButton disabled={!canCreate} label={t("Create project")} menuButtonRef={addProjectMenuRef} menuLabel={t("More project creation options")} onClick={() => setCreateOpen(true)} primaryRef={createButtonRef} title={capabilityUnavailableTitle(canCreate, capabilities.projectsCreate, t)}>
                    <MenuItem disabled={!canRegister} label={t("Add existing project")} onClick={() => setRegisterOpen(true)} title={capabilityUnavailableTitle(canRegister, capabilities.projectsRegistry, t)} />
                    <MenuItem disabled={!canRestore} label={t("Restore from backup")} onClick={() => setRestoreOpen(true)} title={capabilityUnavailableTitle(canRestore, capabilities.backupsRestore, t)} />
                </SplitButton>
            </PageHeader>
            {view === "grid" ? (
                <div className="alcomd-filter-bar projects-secondary-toolbar">
                    <span className="alcomd-filter-bar-label">{t("Sort by:")}</span>
                    <Select
                        aria-label={t("Sort by")}
                        className="projects-sort"
                        label=""
                        onChange={(value) => updateSort(value as ProjectListSort)}
                        options={[
                            { label: t("Last modified"), value: "modified" },
                            { label: t("Name"), value: "name" },
                            { label: t("Project type"), value: "type" },
                            { label: t("Unity version"), value: "unity" },
                            { label: t("Added"), value: "added" }
                        ]}
                        value={sort}
                    />
                    <IconButton label={descending ? t("Sort descending") : t("Sort ascending")} onClick={() => setDescending((current) => !current)} type="button">
                        <Icon asset={descending ? arrowDownwardIcon : arrowUpwardIcon} />
                    </IconButton>
                    <span className="alcomd-filter-bar-status" role="status" aria-live="polite">
                        {state.loading ? null : t(projects.length === 1 ? "{count} project" : "{count} projects", { count: projects.length })}
                    </span>
                </div>
            ) : null}
            <div className="projects-content" aria-busy={state.loading}>
                {state.loading ? <RouteState kind="loading" title={t("Loading projects")} /> : null}
                {state.error !== undefined && !state.loaded ? <ErrorState error={state.error} retry={retryProjects} /> : null}
                {state.loaded && projects.length === 0 ? (
                    <section className="projects-empty" role="status">
                        <h2>{search.length === 0 ? t("No registered projects") : t("No matching projects")}</h2>
                        <p>{search.length === 0 ? t("Register an existing Unity project from the toolbar.") : t("Change the search text to see other projects.")}</p>
                    </section>
                ) : null}
                {projects.length > 0 && view === "list" ? <ProjectsTable client={client} descending={descending} navigate={navigate} onChanged={() => void refresh()} onProjectChanged={updateProject} onSort={updateSort} projects={projects} sort={sort} /> : null}
                {projects.length > 0 && view === "grid" ? <ScrollArea className="projects-grid">{projects.map((project) => <ProjectCard client={client} key={project.projectId ?? project.rootPath} onChanged={updateProject} onRefresh={() => void refresh()} project={project} navigate={navigate} />)}</ScrollArea> : null}
            </div>
            <DirectorySelectionDialog
                client={client}
                label={t("Project location")}
                onChanged={() => void refresh()}
                onClose={() => setRegisterOpen(false)}
                onConfirm={(path) => client.projectRegister(path)}
                open={registerOpen}
                origin={addProjectMenuRef}
                title={t("Register this project?")}
            />
            <CreateProjectDialog
                client={client}
                onChanged={refresh}
                onCompleted={(projectId) => {

                    setCreateOpen(false);
                    navigate(`/projects/${projectId}`);
                }}
                onClose={() => {
                    setCreateOpen(false);
                    window.requestAnimationFrame(() => createButtonRef.current?.focus());
                }}
                open={createOpen}
            />
            {restoreOpen ? <RestoreProjectDialog
                client={client}
                onViewOperation={(operationId) => navigate(`/operations/${operationId}`)}
                onCompleted={(projectId) => {

                    setRestoreOpen(false);
                    void refresh().then(() => navigate(`/projects/${projectId}`));
                }}
                onClose={() => {
                    setRestoreOpen(false);
                    window.requestAnimationFrame(() => addProjectMenuRef.current?.focus());
                }}
                open
            /> : null}
        </section>
    )}</>;
}

function ProjectsTable({
    client,
    descending,
    navigate,
    onChanged,
    onProjectChanged,
    onSort,
    projects,
    sort
}: {
    client: GuiRpcClient;
    descending: boolean;
    navigate(path: string): void;
    onChanged(): void;
    onProjectChanged(project: ProjectSnapshot): void;
    onSort(sort: ProjectListSort): void;
    projects: ProjectSnapshot[];
    sort: ProjectListSort;
}) {
    const { t, locale } = useTranslation();
    const sortableColumn = (header: string, key: ProjectListSort, width?: DesktopListColumn["width"]): DesktopListColumn => {
        const active = sort === key;
        return { key, header, width, onSort: () => onSort(key), sortDirection: active ? (descending ? "descending" : "ascending") : undefined };
    };
    const columns: DesktopListColumn[] = [
        sortableColumn(t("Project"), "name", "100%"),
        sortableColumn(t("Type"), "type", 0),
        sortableColumn(t("Unity"), "unity", 0),
        sortableColumn(t("Added"), "added", 0),
        sortableColumn(t("Last modified"), "modified", 0),
        { key: "actions", header: <span className="visually-hidden">{t("Actions")}</span>, width: 0, className: "projects-column-actions", cellFlow: "wrap" }
    ];
    return <DesktopList className="projects-list" columns={columns} label={t("Projects")} minWidth={720} rows={projects.map((project) => {
        const id = project.projectId;
        return { key: id ?? project.rootPath, cells: [
            <><strong>{projectName(t, locale, project)}</strong><small title={displayPath(project.rootPath)}>{displayPath(project.rootPath)}</small></>,
            <span className="project-type project-type--table"><Icon asset={projectTypeIcon(project.projectType)} /><span>{projectTypeLabel(t, locale, project.projectType)}</span></span>,
            project.unityVersion || t("Unknown"),
            formatRegistered(t, locale, project.registeredAtMs),
            formatModified(t, locale, project.lastModifiedAtMs),
            id === undefined ? <span className="project-unregistered">{t("Unregistered")}</span> : <ProjectRowActions client={client} navigate={navigate} onChanged={onChanged} onProjectChanged={onProjectChanged} project={project} />
        ] };
    })} />;
}

function ProjectRowActions({ client, context = "row", launchAction, navigate, onChanged, onCopyCompleted, onProjectChanged, onRemoved, project }: {
    client: GuiRpcClient;
    context?: "row" | "workspace" | "card";
    launchAction?: ProjectUnityLaunchAction;
    navigate(path: string): void;
    onChanged(): void;
    onCopyCompleted?(projectId: string): void;
    onProjectChanged(project: ProjectSnapshot): void;
    onRemoved?(): void;
    project: ProjectSnapshot;
}) {
    const { t, locale } = useTranslation();
    const current = useActionScope(client);
    const [removalFailure, setRemovalFailure] = useState<{ message: string; error?: RpcError }>();
    const canReadProjects = useCapability(capabilities.projectsRead);
    const canManageProjects = useCapability(capabilities.projectsRegistry);
    const canCopyProjects = useCapability(capabilities.projectsCopy);
    const canDeleteProjects = useCapability(capabilities.projectsDelete);
    const canReadBackups = useCapability(capabilities.backupsRead);
    const canManageUnity = useCapability(capabilities.unityManage);
    const [argumentsOpen, setArgumentsOpen] = useState(false);
    const [copyOpen, setCopyOpen] = useState(false);
    const [openingDirectory, setOpeningDirectory] = useState(false);
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
    const unityLaunch = useProjectUnityLaunch({ client, navigate, onLaunchSettled: onChanged, project });
    const { icon: unityLaunchIcon, label: unityLaunchLabel, ...unityLaunchButton } = unityLaunch.action;

    const observeDeletion = useCallback((next: Operation) => {
        setDeleteOperation(next);
        if (next.state === "succeeded") {
            setConfirmUnregister(false);
            setDeletePlan(undefined);

            if (onRemoved !== undefined) onRemoved();
            else onChanged();
        } else if (["failed", "cancelled"].includes(next.state)) {
            setDeleting(false);
            setRemovalFailure({ message: t("Unable to delete project directory: {value0}", { value0: next.errorCode === undefined ? humanize(t, next.state) : t(requestErrorMessage(next.errorCode)) }), error: next.errorCode === undefined ? undefined : { code: next.errorCode, message: "", diagnosticId: next.diagnosticId } });
        }
    }, [onChanged, onRemoved,  t]);
    if (projectId === undefined) return null;

    const unregister = async () => {
        if (revision === undefined) return;
        setRemovalFailure(undefined);
        setUnregistering(true);
        try {
            await client.projectUnregister(projectId, revision);
            if (!current()) return;
            setConfirmUnregister(false);

            if (onRemoved !== undefined) onRemoved();
            else onChanged();
        } catch (caught: unknown) {
            if (current()) setRemovalFailure({ error: safeError(t, locale, caught), message: t("Unable to unregister project: {value0}", { value0: t(requestErrorMessage(safeError(t, locale, caught).code)) }) });
        } finally {
            setUnregistering(false);
        }
    };

    const openDirectory = async () => {
        setOpeningDirectory(true);
        setMenuOpen(false);
        try {
            await client.openProjectDirectory(projectId);

        } catch {} finally {
            setOpeningDirectory(false);
        }
    };

    const beginCopy = () => {
        setMenuOpen(false);
        setCopyOpen(true);
    };

    const planDelete = async () => {
        if (revision === undefined) return;
        setRemovalFailure(undefined);
        setDeleting(true);
        try {
            const result = await client.projectPlanDeleteDirectory(projectId, revision);
            if (!current()) return;
            setDeletePlan(result.plan);
            setDeleteConfirmation("");
        } catch (caught: unknown) {
            if (current()) setRemovalFailure({ error: safeError(t, locale, caught), message: t("Unable to plan project deletion: {value0}", { value0: t(requestErrorMessage(safeError(t, locale, caught).code)) }) });
        } finally {
            setDeleting(false);
        }
    };

    const beginUnregister = () => {
        setMenuOpen(false);
        setRemovalFailure(undefined);
        setRemovalIntent("unregister");
        setDeletePlan(undefined);
        setConfirmUnregister(true);
    };

    const beginDeleteDirectory = () => {
        setMenuOpen(false);
        setRemovalFailure(undefined);
        setRemovalIntent("delete");
        setDeletePlan(undefined);
        setConfirmUnregister(true);
        void planDelete();
    };

    const applyDelete = async () => {
        if (revision === undefined || deletePlan === undefined || deleteConfirmation !== deletePlan.normalizedLeaf) return;
        setRemovalFailure(undefined);
        setDeleting(true);
        try {
            const result = await client.projectApplyDeleteDirectory(deletePlan.planId, revision);
            const operation = await client.operationGet(result.operationId);
            if (!current()) return;
            observeDeletion(operation);

        } catch (caught: unknown) {
            setDeleting(false);
            if (current()) setRemovalFailure({ error: safeError(t, locale, caught), message: t("Unable to delete project directory: {value0}", { value0: t(requestErrorMessage(safeError(t, locale, caught).code)) }) });
        }
    };

    const menuItems = <>
        {workspace ? <MenuItem disabled={!canManageUnity} label={t("Edit launch arguments")} onClick={() => setArgumentsOpen(true)} title={capabilityUnavailableTitle(canManageUnity, capabilities.unityManage, t)} /> : null}
        <MenuItem disabled={!canReadProjects || openingDirectory} label={t("Open Project Directory")} onClick={() => void openDirectory()} title={capabilityUnavailableTitle(canReadProjects, capabilities.projectsRead, t)} />
        <MenuItem disabled={!canCopyProjects || revision === undefined} label={t("Copy Project")} onClick={beginCopy} title={capabilityUnavailableTitle(canCopyProjects, capabilities.projectsCopy, t)} />
        {workspace ? <>
            <MenuItem disabled={!canReadBackups} label={t("Backups")} onClick={() => navigate(`/projects/${projectId}/backups`)} title={capabilityUnavailableTitle(canReadBackups, capabilities.backupsRead, t)} />
            <MenuItem className="project-actions-menu-item--danger project-actions-menu-item--danger-group" disabled={!canManageProjects || revision === undefined} label={t("Remove from list")} onClick={beginUnregister} title={capabilityUnavailableTitle(canManageProjects, capabilities.projectsRegistry, t)} />
            <MenuItem className="project-actions-menu-item--danger" disabled={!canDeleteProjects || revision === undefined} label={t("Delete Project Directory…")} onClick={beginDeleteDirectory} title={capabilityUnavailableTitle(canDeleteProjects, capabilities.projectsDelete, t)} />
        </> : <MenuItem className="project-actions-menu-item--danger" disabled={!canManageProjects || revision === undefined} label={t("Remove Project")} onClick={beginUnregister} title={capabilityUnavailableTitle(canManageProjects, capabilities.projectsRegistry, t)} />}
    </>;
    return <>{(
        <>
            <div className={workspace ? "project-row-actions project-workspace-more-actions" : context === "card" ? "project-row-actions project-row-actions--card" : "project-row-actions"}>
                {workspace ? null : <ProjectFavoriteButton client={client} onChanged={onProjectChanged} onRefresh={onChanged} project={project} />}
                {workspace ? null : <Button {...unityLaunchButton} className="project-open-unity-action" type="button">
                    {unityLaunchIcon}
                    {unityLaunchLabel}
                </Button>}
                {workspace ? null : <Button disabled={!canReadProjects} onClick={() => navigate(`/projects/${projectId}`)} title={capabilityUnavailableTitle(canReadProjects, capabilities.projectsRead, t)} type="button" variant="tonal">{t("Manage")}</Button>}
                {workspace ? null : <Button disabled={!canReadBackups} onClick={() => navigate(`/projects/${projectId}/backups`)} title={capabilityUnavailableTitle(canReadBackups, capabilities.backupsRead, t)} type="button" variant="tonal">{t("Backups")}</Button>}
                {launchAction === undefined ? <><IconButton className="project-more-actions" label={t("More actions for {value0}", { value0: projectName(t, locale, project) })} onClick={() => setMenuOpen(true)} ref={menuAnchorRef} type="button">
                    <Icon asset={moreVertIcon} size={24} />
                </IconButton>
                <Menu anchorRef={menuAnchorRef} className="project-actions-menu" onClose={() => setMenuOpen(false)} open={menuOpen}>
                    {menuItems}
                </Menu>
                </> : <SplitButton {...launchAction} menuButtonRef={menuAnchorRef} menuLabel={t("More actions for {value0}", { value0: projectName(t, locale, project) })}>{menuItems}</SplitButton>}
            </div>
            {workspace ? null : unityLaunch.dialogs}
            {workspace && argumentsOpen ? <ProjectLaunchArgumentsDialog client={client} onChanged={onChanged} onClose={() => { setArgumentsOpen(false); window.requestAnimationFrame(() => menuAnchorRef.current?.focus()); }}  projectId={projectId} /> : null}
            <Dialog onClose={() => { if (!deleting) { setConfirmUnregister(false); setDeletePlan(undefined); setRemovalIntent("unregister"); } }} open={confirmUnregister} title={deletePlan === undefined ? removalIntent === "delete" ? t("Preparing permanent deletion…") : workspace ? t("Remove this project from the list?") : t("Remove this project?") : t("Permanently delete project directory?")}>
                {removalFailure === undefined ? null : removalFailure.error === undefined ? <p role="alert">{removalFailure.message}</p>
                    : <div role="alert"><p>{t((removalFailure.message) ?? requestErrorMessage((removalFailure.error).code))}</p><code>{(removalFailure.error).code}</code>{removalFailure.error.diagnosticId === undefined ? null : <p><code>{removalFailure.error.diagnosticId}</code></p>}</div>}
                {deletePlan === undefined && removalIntent === "unregister" ? <>
                    <p>{workspace ? t("Remove the ALCOMD registration for this project?") : t("Choose whether to remove only the ALCOMD registration or permanently delete the local Unity project directory.")}</p>
                    <p>{t("Removing from the list does not delete files.")}</p>
                    <div className={workspace ? "dialog-actions" : "dialog-actions dialog-actions--split"}>
                        <DialogCloseButton disabled={unregistering || deleting} variant="text">{t("Cancel")}</DialogCloseButton>
                        <Button disabled={!canManageProjects || unregistering || deleting || revision === undefined} onClick={() => void unregister()} title={capabilityUnavailableTitle(canManageProjects, capabilities.projectsRegistry, t)} type="button" variant="tonal">
                            {t("Remove from list")}
                        </Button>
                        {workspace ? null : <Button className="material-button--danger" disabled={!canDeleteProjects || unregistering || deleting || revision === undefined} onClick={() => { setRemovalIntent("delete"); void planDelete(); }} title={capabilityUnavailableTitle(canDeleteProjects, capabilities.projectsDelete, t)} type="button" variant="text">{t("Delete Project Directory…")}</Button>}
                    </div>
                </> : deletePlan === undefined ? <>
                    <p>{t("Loading the destructive review. No files have been changed.")}</p>
                    <div className="dialog-actions"><DialogCloseButton disabled={deleting} variant="text">{t("Cancel")}</DialogCloseButton></div>
                </> : <>
                    <p><strong>{t("This permanently deletes local files and does not use the Recycle Bin or Trash.")}</strong></p>
                    <p>{t("No automatic backup will be created. This is different from removing the project from the list.")}</p>
                    <p className="project-delete-path"><code>{displayPath(deletePlan.canonicalRootPath)}</code></p>
                    <p>{t("Unity writer observation: ")}<strong>{humanize(t, deletePlan.writerEvidence.state)}</strong>.</p>
                    <TextField aria-label={t("Confirm project directory name")} label={t("Type {value0} to confirm", { value0: deletePlan.normalizedLeaf })} onInput={setDeleteConfirmation} required value={deleteConfirmation} />
                    {deleteOperation === undefined ? null : <>
                        <p>{t("Operation: ")}<code>{deleteOperation.operationId}</code> · {humanize(t, deleteOperation.state)}</p>
                        <OperationFollow client={client} operationId={deleteOperation.operationId} onSnapshot={setDeleteOperation} onTerminal={observeDeletion} onReadError={(error) => {
                            if (error) setRemovalFailure({ error, message: t("Unable to follow project deletion: {value0}", { value0: t(requestErrorMessage(error.code)) }) });
                        }}>{() => null}</OperationFollow>
                    </>}
                    <div className="dialog-actions">
                        <Button disabled={deleting} onClick={() => { setDeletePlan(undefined); setDeleteConfirmation(""); }} type="button" variant="text">{t("Back")}</Button>
                        <Button className="material-button--danger" disabled={deleting || deleteConfirmation !== deletePlan.normalizedLeaf} onClick={() => void applyDelete()} type="button" variant="text">
                            {t("Permanently delete")}
                        </Button>
                    </div>
                </>}
            </Dialog>
            <CopyProjectDialog
                client={client}
                onCompleted={(targetProjectId) => {
                    if (onCopyCompleted !== undefined) onCopyCompleted(targetProjectId);
                    else {

                        onChanged();
                    }
                }}
                onClose={() => setCopyOpen(false)}
                open={copyOpen}
                project={project}
            />
        </>
    )}</>;
}

function CopyProjectDialog({ client, onCompleted, onClose, open, project }: { client: GuiRpcClient; onCompleted(targetProjectId: string): void; onClose(): void; open: boolean; project: ProjectSnapshot }) {
    const { t, locale } = useTranslation();
    const { directory: targetParent, setDirectory: setTargetParent, error: directoryError } = useProjectDirectory(client, open);
    const [targetLeaf, setTargetLeaf] = useState("");
    const [plan, setPlan] = useState<ProjectCopyPlan>();
    const [operation, setOperation] = useState<Operation>();
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState<RpcError>();
    const current = useActionScope(client, open);

    useEffect(() => {
        if (!open) {
            setPlan(undefined);
            setOperation(undefined);
            setBusy(false);
            setError(undefined);
            return;
        }
        setTargetLeaf(t("{value0} Copy", { value0: projectName(t, locale, project) }));
    }, [open, project]);


    const chooseTargetParent = async () => {
        setBusy(true);
        setError(undefined);
        try {
            const selected = await client.selectDirectory();
            if (current() && selected !== undefined) setTargetParent(selected);
        } catch (caught: unknown) {
            if (current()) setError(safeError(t, locale, caught));
        } finally {
            if (current()) setBusy(false);
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
            setError(safeError(t, locale, caught));
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
            setError(safeError(t, locale, caught));
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
            setError(safeError(t, locale, caught));
        } finally {
            setBusy(false);
        }
    };

    const terminal = operation !== undefined && ["succeeded", "failed", "cancelled"].includes(operation.state);
    return (
        <Dialog onClose={onClose} open={open} title={t("Copy project")}>
            <div className="project-copy-review">
                {plan === undefined ? (
                    <>
                        <p>{t("Choose a name and location for the copied project. Existing folders are never overwritten.")}</p>
                        <TextField aria-label={t("Copied project name")} label={t("Project name")} onInput={setTargetLeaf} value={targetLeaf} />
                        <div className="project-copy-location">
                            <TextField aria-label={t("Copy project location")} label={t("Project location")} readOnly value={displayPath(targetParent)} />
                            <Button disabled={busy} onClick={() => void chooseTargetParent()} type="button" variant="tonal">{t("Choose…")}</Button>
                        </div>
                        <div className="dialog-actions">
                            <DialogCloseButton disabled={busy} variant="text">{t("Cancel")}</DialogCloseButton>
                            <Button disabled={busy || targetParent.length === 0 || targetLeaf.trim().length === 0} onClick={() => void createPlan()} type="button">{t("Review copy")}</Button>
                        </div>
                    </>
                ) : operation === undefined ? (
                    <>
                        <p><strong>{t("Source:")}</strong> {projectName(t, locale, project)}</p>
                        <p><strong>{t("Destination:")}</strong> {displayChildPath(plan.targetParentCanonicalPath, plan.normalizedTargetLeaf)}</p>
                        <p><strong>{t("Unity writer:")}</strong> {humanize(t, plan.writerEvidence.state)}</p>
                        <p><strong>{t("Copy profile:")}</strong> {t("v{version}; excludes Logs, Obj, Temp and .git.", { version: plan.profile.version })}</p>
                        <div className="dialog-actions">
                            <Button disabled={busy} onClick={() => setPlan(undefined)} type="button" variant="text">{t("Back")}</Button>
                            <Button disabled={busy} onClick={() => void apply()} type="button">{t("Start copy")}</Button>
                        </div>
                    </>
                ) : (
                    <>
                        <OperationFollow client={client} operationId={operation.operationId} onSnapshot={setOperation} onReadError={setError} onTerminal={(next) => {
                            if (next.state === "succeeded" && plan !== undefined) onCompleted(plan.targetProjectId);
                        }}>{() => null}</OperationFollow>
                        <p role="status">{operation.progress?.phase === undefined ? t("Copy {state}.", { state: humanize(t, operation.state) }) : t("Copy {state}: {phase}.", { state: humanize(t, operation.state), phase: humanize(t, operation.progress.phase) })}</p>
                        <div className="dialog-actions">
                            {!terminal ? <Button disabled={busy} onClick={() => void cancel()} type="button" variant="text">{t("Cancel copy")}</Button> : null}
                            <DialogCloseButton disabled={!terminal}>{t("Close")}</DialogCloseButton>
                        </div>
                    </>
                )}
                {directoryError === undefined ? null : <div role="alert"><strong>{t("Default project location could not be loaded.")}</strong><p>{t(requestErrorMessage((directoryError).code))}</p><code>{(directoryError).code}</code>{directoryError.diagnosticId === undefined ? null : <p><code>{directoryError.diagnosticId}</code></p>}</div>}
                {error === undefined ? null : <div role="alert"><p>{t(t("Copy failed: ") + t(requestErrorMessage(error.code)))}</p><code>{(error).code}</code>{error.diagnosticId === undefined ? null : <p><code>{error.diagnosticId}</code></p>}</div>}
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

function ProjectCard({ client, onChanged, onRefresh, project, navigate }: { client: GuiRpcClient; onChanged(project: ProjectSnapshot): void; onRefresh(): void; project: ProjectSnapshot; navigate(path: string): void }) {
    const { t, locale } = useTranslation();
    const id = project.projectId;
    const name = projectName(t, locale, project);
    const path = displayPath(project.rootPath);
    const type = projectTypeLabel(t, locale, project.projectType);
    const unity = t("Unity {value0}", { value0: project.unityVersion || "unknown" });
    const dates = t("Added {value0} / Last modified {value1}", { value0: formatRegistered(t, locale, project.registeredAtMs), value1: formatModified(t, locale, project.lastModifiedAtMs) });
    return (
        <article aria-label={name} className="project-card">
            <header className="project-card-heading">
                <div className="project-card-identity">
                    <h2 title={name}>{name}</h2>
                    <p className="project-path" title={path}>{path}</p>
                </div>
            </header>
            <div className="project-card-properties">
                <span className="project-type" title={type}><Icon asset={projectTypeIcon(project.projectType)} /><span>{type}</span></span>
                <span className="project-card-unity" title={unity}>{project.unityVersion || t("Unity unknown")}</span>
            </div>
            <p className="project-card-dates" title={dates}>{dates}</p>
            <footer className="project-card-actions">
                {id === undefined ? <span className="project-unregistered">{t("Unregistered")}</span> : <ProjectRowActions client={client} context="card" navigate={navigate} onChanged={onRefresh} onProjectChanged={onChanged} project={project} />}
            </footer>
        </article>
    );
}

function ProjectFavoriteButton({ client, onChanged, onRefresh, project }: { client: GuiRpcClient; onChanged(project: ProjectSnapshot): void; onRefresh(): void; project: ProjectSnapshot }) {
    const { t } = useTranslation();
    const current = useActionScope(client);
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
            if (!current()) return;
            onChanged(result.project);

        } catch (caught: unknown) {
            if (!current()) return;
            if (normalizeRequestError(caught).code === "revision_conflict") onRefresh();
        } finally {
            setBusy(false);
        }
    };
    const label = busy ? t("Updating favorite") : favorite ? t("Remove from favorites") : t("Add to favorites");
    return <IconButton aria-pressed={favorite} className="project-favorite-action" disabled={!canManageProjects || busy || revision === undefined} label={label} onClick={() => void toggle()} title={capabilityUnavailableTitle(canManageProjects, capabilities.projectsRegistry, t) ?? label} type="button"><Icon asset={starIcon} /></IconButton>;
}

function DirectorySelectionDialog({ client, label, onChanged, onClose, onConfirm, open, origin, title }: {
    client: GuiRpcClient;
    label: string;
    onChanged(): void;
    onClose(): void;
    onConfirm(path: string): Promise<unknown>;
    open: boolean;
    origin: DialogOrigin;
    title: string;
}) {
    const { t } = useTranslation();
    const current = useActionScope(client, open);
    const pending = useRef(false);
    const [path, setPath] = useState("");
    const [busy, setBusy] = useState(false);
    const [choosing, setChoosing] = useState(false);
    const [error, setError] = useState<RpcError>();

    useEffect(() => {
        if (open) return;
        setPath("");
        setError(undefined);
    }, [open]);

    const confirm = async () => {
        if (pending.current || choosing || path.trim() === "") return;
        pending.current = true;
        setBusy(true);
        setError(undefined);
        try {
            await onConfirm(path);
            if (!current()) return;
            onChanged();
            onClose();
        } catch (caught) {
            if (current()) setError(normalizeRequestError(caught));
        } finally {
            pending.current = false;
            setBusy(false);
        }
    };

    return <Dialog dismissible={!busy && !choosing} onClose={onClose} open={open} returnFocus={origin} title={title} actions={<>
        <DialogCloseButton disabled={busy || choosing} variant="text">{t("Cancel")}</DialogCloseButton>
        <Button disabled={busy || choosing || path.trim() === ""} onClick={() => void confirm()} type="button">{t("Confirm")}</Button>
    </>}>
        {open ? <>
            <FilePicker client={client} kind="directory" choosePath={() => client.selectDirectory()} disabled={busy}
                label={label} value={path} onChoosingChange={setChoosing}
                onPathChange={(next) => { setPath(next); setError(undefined); }} />
            {error === undefined ? null : <div role="alert"><p>{t(requestErrorMessage(error.code))}</p><code>{error.code}</code>{error.diagnosticId === undefined ? null : <p><code>{error.diagnosticId}</code></p>}</div>}
        </> : null}
    </Dialog>;
}

function projectName(t: Translate, _locale: SupportedLocale, project: ProjectSnapshot): string {
    return project.displayName ?? t("Unity project");
}

function formatModified(t: Translate, locale: SupportedLocale, value: number | undefined): string {
    if (value === undefined || !Number.isFinite(value)) return t("Unknown Date");
    const elapsed = Date.now() - value;
    const absoluteElapsed = Math.abs(elapsed);
    const minute = 60_000;
    const hour = 60 * minute;
    const day = 24 * hour;
    const week = 7 * day;
    const month = 30 * day;
    const year = 365 * day;
    const formatter = new Intl.RelativeTimeFormat(locale, { numeric: "always" });

    if (absoluteElapsed < minute) return elapsed >= 0 ? t("moments ago") : t("in moments");
    if (absoluteElapsed < hour) return formatter.format(-Math.trunc(elapsed / minute), "minute");
    if (absoluteElapsed < day) return formatter.format(-Math.trunc(elapsed / hour), "hour");
    if (absoluteElapsed < week) return formatter.format(-Math.trunc(elapsed / day), "day");
    if (absoluteElapsed < month) return formatter.format(-Math.trunc(elapsed / week), "week");
    if (absoluteElapsed < year) return formatter.format(-Math.trunc(elapsed / month), "month");
    return formatter.format(-Math.trunc(elapsed / year), "year");
}

function formatRegistered(t: Translate, locale: SupportedLocale, value: number | undefined): string {
    if (value === undefined || !Number.isFinite(value)) return t("Unknown");
    const date = new Date(value);
    const year = date.getFullYear().toString().padStart(4, "0");
    const month = (date.getMonth() + 1).toString().padStart(2, "0");
    const day = date.getDate().toString().padStart(2, "0");
    return `${year}-${month}-${day}`;
}

function projectTypeIcon(projectType: string) {
    const normalized = projectType.toLocaleLowerCase();
    if (normalized.includes("avatar")) return accountCircleIcon;
    if (normalized.includes("world")) return publicIcon;
    return helpIcon;
}

function projectTypeLabel(t: Translate, locale: SupportedLocale, projectType: string): string {
    const normalized = projectType.toLocaleLowerCase();
    if (normalized.includes("avatar")) return t("Avatar");
    if (normalized.includes("world")) return t("World");
    return projectType.length === 0 ? t("Unknown") : projectType;
}

export function ProjectDetailPage(props: PageProps & { projectId: string }) {
    return <ProjectPackageWorkspace {...props} />;
}

export function ProjectPackagesPage(props: PageProps & { projectId: string }) {
    return <ProjectDetailPage {...props} />;
}

interface PackageWorkspaceRow {
    evidence?: PackageCandidateSummary;
    displayName: string;
    links?: PackageCandidateSummary["links"];
    installedVersion?: string;
    packageId: string;
    sourceOptions: Array<{ key: string; label: string; selector: PackageSourceSelector }>;
}

function PackageVersionPicker({ client, project, snapshot, source, disabled, canPlan, onSelect, onStale, row }: { client: GuiRpcClient; project: ProjectSnapshot; snapshot?: string; source?: PackageSourceSelector; disabled: boolean; canPlan: boolean; onSelect(version: PackageCandidateEvidence, origin: DialogOrigin): void; onStale(): void; row: PackageWorkspaceRow }) {
    const { t, locale } = useTranslation();
    const [items, setItems] = useState<PackageCandidateEvidence[]>([]);
    const [error, setError] = useState<string>();
    const [busy, setBusy] = useState(false);
    const pending = useRef<{ generation: number; read: Promise<void> } | undefined>(undefined);
    const opened = useRef(false);
    const generation = useRef(0);
    const traversal = useRef<{ pages: number; count: number; cursor?: CandidateCursor; complete: boolean }>({ pages: 0, count: 0, complete: false });
    const anchorRef = useRef<HTMLElement>(null);
    const sourceKey = JSON.stringify(source);
    const installedYanked = row.installedVersion !== undefined && row.evidence?.installedYanked === true;
    const displayedVersion = row.installedVersion === undefined ? t("Not installed") : `${row.installedVersion}${installedYanked ? ` ${t("Yanked")}` : ""}`;
    useEffect(() => {
        generation.current += 1;
        traversal.current = { pages: 0, count: 0, complete: false };
        setItems([]); setError(undefined); setBusy(false);
        return () => { generation.current += 1; opened.current = false; };
    }, [snapshot, sourceKey]);
    const load = (): Promise<void> => {
        const current = generation.current;
        if (pending.current?.generation === current) return pending.current.read;
        if (traversal.current.complete || !snapshot || project.revision === undefined || project.projectId === undefined) return Promise.resolve();
        const projectId = project.projectId;
        const projectRevision = project.revision;
        setBusy(true);
        setError(undefined);
        const read = (async () => {
            try {
                do {
                    const next = traversal.current.cursor;
                    const page = await queryCandidates(client, { projectId, expectedRevision: projectRevision, expectedSnapshot: snapshot, view: { kind: "versions", packageId: row.packageId, ...(source ? { source: source.kind === "repository" ? { kind: "repository" as const, repository_id: source.repositoryId } : { kind: "user_package" as const, user_package_id: source.userPackageId } } : {}) }, limit: 64, ...(next ? { cursor: next } : {}) });
                    if (current !== generation.current) return;
                    validateVersionPage(page, { projectId, projectRevision, packageId: row.packageId, fingerprint: snapshot, ...traversal.current, offset: next?.offset ?? 0 });
                    traversal.current = { pages: traversal.current.pages + 1, count: traversal.current.count + page.items.length, cursor: page.nextCursor ?? undefined, complete: page.nextCursor === null };
                    setItems((existing) => next ? [...existing, ...page.items] : page.items);
                } while (opened.current && !traversal.current.complete);
            } catch (caught) {
                if (current !== generation.current) return;
                const code = safeError(t, locale, caught).code;
                traversal.current = { pages: 0, count: 0, complete: false };
                setItems([]); setError(code);
                if (code === "package_candidate_evidence_stale") onStale();
            } finally {
                if (pending.current?.generation === current) pending.current = undefined;
                // The imperative picker may open as soon as this promise resolves.
                // Commit options before Material measures and focuses its menu.
                if (current === generation.current) flushSync(() => setBusy(false));
            }
        })();
        pending.current = { generation: current, read };
        return read;
    };
    const versions = new Map<string, PackageCandidateEvidence[]>();
    for (const item of traversal.current.complete ? items : []) {
        if (item.classification === "unknown") continue;
        const entries = versions.get(item.version) ?? [];
        entries.push(item);
        versions.set(item.version, entries);
    }
    return <Select
        className={`package-version-picker alcomd-select--constrained${installedYanked ? " package-version-picker--yanked" : ""}`}
        label={t("Version for {value0}: {value1}", { value0: row.displayName, value1: displayedVersion })}
        displayText={displayedVersion}
        prepareOpen={() => { opened.current = true; return load(); }}
        disabled={disabled || !snapshot}
        onOpen={() => { opened.current = true; void load(); }}
        onClose={() => { opened.current = false; }}
        onSelect={(value, origin) => {
            if (value === row.installedVersion) return;
            const matches = versions.get(value);
            const option = matches?.length === 1 ? matches[0] : undefined;
            if (!disabled && canPlan && !busy && option && (actionableVersion(option) || option.manualUnityOverrideAllowed === true)) {
                anchorRef.current = origin.current;
                onSelect(option, anchorRef);
            }
        }}
        options={[
            ...(["compatible", "unknown", "incompatible"] as const).flatMap((group) => {
                const grouped = [...versions].filter(([, entries]) => (entries[0]!.unity === "not_required" ? "compatible" : entries[0]!.unity) === group);
                if (grouped.length === 0) return [];
                return [
                    ...(group === "compatible" ? [] : [
                        { kind: "separator" as const, value: `separator:${group}`, label: "", disabled: true },
                        { kind: "heading" as const, value: `unity-group:${group}`, label: t(group === "incompatible" ? "Incompatible versions" : "Compatibility unknown"), disabled: true }
                    ]),
                    ...grouped.map(([version, entries]) => ({
                        value: version,
                        label: version,
                        title: entries.length > 1 ? t("Select a source to choose this version") : entries[0]!.reasons.map((reason) => reasonText(reason, t)).join(t(", ")),
                        disabled: disabled || busy || (version !== row.installedVersion && (!canPlan || entries.length !== 1 || !(actionableVersion(entries[0]!) || entries[0]!.manualUnityOverrideAllowed === true)))
                    }))
                ];
            }),
            // Material opens before onOpen can commit React's pending state.
            // Prepare this row while idle too, so its first visible frame is
            // never an empty menu. It is not a selectable package version.
            ...(!error && !traversal.current.complete && versions.size === 0 ? [{ kind: "status" as const, value: "loading", label: t("Loading versions…") }] : []),
            ...(error ? [{ value: "error", label: t("Versions unavailable: {value0}", { value0: t(requestErrorMessage(error)) }), disabled: true }] : []),
            ...(!busy && !error && traversal.current.complete && versions.size === 0 ? [{ value: "empty", label: t("No recorded versions"), disabled: true }] : [])
        ]}
        value={row.installedVersion !== undefined && versions.has(row.installedVersion) ? row.installedVersion : ""}
    />;
}

function PackagePrimaryAction({ canPlanV1, canPlanV2, catalogReady, evidence, onSelect, onRemove, row }: { canPlanV1: boolean; canPlanV2: boolean; catalogReady: boolean; evidence?: PackageCandidateSummary; onSelect(version: PackageCandidateEvidence, origin: DialogOrigin): void; onRemove(origin: DialogOrigin): void; row: PackageWorkspaceRow }) {
    const { t } = useTranslation();
    const origin = useRef<HTMLElement>(null);
    const installChoice = evidence?.projectLatest;
    if (row.installedVersion !== undefined) return <IconButton className="package-primary-action package-remove-action" disabled={!canPlanV1} label={t("Remove")} onClick={() => onRemove(origin)} ref={origin} title={t("Remove {value0}", { value0: row.displayName })} type="button"><Icon asset={cancelIcon} /></IconButton>;
    return <IconButton className="package-primary-action" disabled={!canPlanV1 || !canPlanV2 || !catalogReady || installChoice?.kind !== "candidate" || installChoice.candidate.eligibility !== "eligible"} label={t("Install")} onClick={() => { if (installChoice?.kind === "candidate") onSelect(installChoice.candidate, origin); }} ref={origin} title={capabilityUnavailableTitle(canPlanV1 && canPlanV2, capabilities.packagesPlanV2, t) ?? t("Install {value0}", { value0: row.displayName })} type="button"><Icon asset={downloadIcon} /></IconButton>;
}

function PackageSourcePicker({ label, onChange, options, value, disabled }: { label: string; onChange(value: string): void; options: PackageWorkspaceRow["sourceOptions"]; value: string; disabled: boolean }) {
    const { t } = useTranslation();
    const sourceNames = [...new Set(options.map((option) => option.label))];
    const summary = sourceNames.length === 1 ? sourceNames[0]! : t("Multiple sources");
    if (options.length === 1) return <span className="package-source-summary" title={summary}>{summary}</span>;
    return <div className="package-source-summary" title={sourceNames.join(t(", "))}><Select className="package-source-picker alcomd-select--constrained" disabled={disabled} label={label} onChange={onChange} options={[
        { value: "", label: t("Automatic source selection"), displayText: summary },
        ...options.map((option) => ({ value: option.key, label: option.label })),
        ...(value && !options.some((option) => option.key === value) ? [{ value, label: t("Selected source unavailable"), disabled: true }] : [])
    ]} value={value} /></div>;
}

function PackageLatestInfo({ evidence, row, canPlan, onSelect }: { evidence?: PackageCandidateSummary; row: PackageWorkspaceRow; canPlan: boolean; onSelect(version: PackageCandidateEvidence, origin: DialogOrigin): void }) {
    const { t } = useTranslation();
    const origin = useRef<HTMLElement>(null);
    if (evidence === undefined) return <>—</>;
    const candidate = evidence.projectLatest.kind === "candidate" ? evidence.projectLatest.candidate : undefined;
    const update = evidence.update.kind === "target" ? evidence.update.candidate : undefined;
    const incompatible = evidence.latest.kind === "candidate" && evidence.latest.candidate.unity === "incompatible" ? t("Incompatible version: {version}", { version: evidence.latest.candidate.version }) : undefined;
    if (update !== undefined) return <Button aria-label={t("Update")} className="package-latest-update" disabled={!canPlan} onClick={() => onSelect(update, origin)} ref={origin} title={incompatible ?? t("Update {value0}", { value0: row.displayName })} type="button" variant="outlined"><Icon asset={upgradeIcon} slot="icon" />{update.version}</Button>;
    return <span title={incompatible ?? updateReason(evidence.update, t)}>{candidate?.version ?? "—"}</span>;
}

function PackageRowMoreMenu({ canPlanV1, canPlanV2, client, onAction, row }: {
    canPlanV1: boolean;
    canPlanV2: boolean;
    client: GuiRpcClient;
    onAction(action: PackageActionSelection["action"], origin: DialogOrigin): void;
    row: PackageWorkspaceRow;
}) {
    const { t } = useTranslation();
    const [open, setOpen] = useState(false);
    // Keep a first-opened menu connected through Material's closed callbacks.
    const [contentMounted, setContentMounted] = useState(false);
    const anchorRef = useRef<HTMLElement>(null);
    const openLink = async (kind: "documentation" | "changelog") => {
        const url = row.links?.[kind];
        if (url === undefined) return;
        setOpen(false);
        try {
            await client.openPackageLink(url);
        } catch {}
    };
    return <>{(
        <>
            <IconButton className="package-more-actions" label={t("More actions for {value0}", { value0: row.displayName })} onClick={() => { setContentMounted(true); setOpen(true); }} ref={anchorRef} type="button">
                <Icon asset={moreVertIcon} size={24} />
            </IconButton>
            {contentMounted ? <Menu anchorRef={anchorRef} className="package-actions-menu" onClose={() => setOpen(false)} open={open}>
                <MenuItem disabled={row.installedVersion === undefined || !canPlanV2} label={t("Reinstall")} onClick={() => { setOpen(false); onAction("reinstall", anchorRef); }} title={capabilityUnavailableTitle(canPlanV2, capabilities.packagesPlanV2, t)} />
                <MenuItem disabled={row.links?.changelog === undefined} label={t("Changelog")} onClick={() => void openLink("changelog")} />
                <MenuItem disabled={row.links?.documentation === undefined} label={t("Documentation")} onClick={() => void openLink("documentation")} />
            </Menu> : null}
        </>
    )}</>;
}

function PackageWorkspaceMoreMenu({ canPlanV1, canPlanV2, onResolve, onReinstallAll }: { canPlanV1: boolean; canPlanV2: boolean; onResolve(origin: DialogOrigin): void; onReinstallAll(origin: DialogOrigin): void }) {
    const { t, locale } = useTranslation();
    const [open, setOpen] = useState(false);
    const anchorRef = useRef<HTMLElement>(null);
    return (
        <>
            <IconButton label={t("More package actions")} onClick={() => setOpen(true)} ref={anchorRef} type="button">
                <Icon asset={moreVertIcon} size={24} />
            </IconButton>
            <Menu anchorRef={anchorRef} onClose={() => setOpen(false)} open={open}>
                <MenuItem disabled={!canPlanV1} label={t("Resolve Dependencies…")} onClick={() => { setOpen(false); onResolve(anchorRef); }} title={capabilityUnavailableTitle(canPlanV1, capabilities.packagesPlanV1, t)} />
                <MenuItem disabled={!canPlanV2} label={t("Reinstall All Installed Packages…")} onClick={() => { setOpen(false); onReinstallAll(anchorRef); }} title={capabilityUnavailableTitle(canPlanV2, capabilities.packagesPlanV2, t)} />
            </Menu>
        </>
    );
}

interface PackageWorkspaceValue {
    sourceSelectionKey: string;
    candidateRequest?: ProjectCandidateRequest;
    candidateError?: string;
    candidateRead?: Promise<PageReadResult<CandidateSummaryPage>>;
    initialCandidates?: PageReadResult<CandidateSummaryPage>;
    prepared?: {
        first: CandidateSummaryPage;
        candidates: CandidateSummarySet;
        selectedIds: string[];
        bulk: CandidateBulkActions;
        filter?: CandidateFilter;
        filteredItems?: PackageCandidateSummary[];
    };
    launchOptions: UnityLaunchOptionsResult;
    project: ProjectSnapshot;
    settings: SettingsGetResult;
}

function usePackageDisplaySettings(client: GuiRpcClient, settings: SettingsGetResult | undefined, reloadSettings: () => void, onSaved?: () => void) {
    const current = useActionScope(client);
    const [saved, setSaved] = useState<SettingsGetResult>();
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState<RpcError>();
    const saving = useRef(false);
    const snapshot = settings === undefined ? undefined
        : saved !== undefined && saved.revision > settings.revision ? saved : settings;
    const save = async (update: Partial<OfficialSettings["packages"]>) => {
        if (snapshot === undefined || saving.current) return;
        saving.current = true;
        setBusy(true);
        setError(undefined);
        try {
            const next = await client.settingsUpdate(snapshot.revision, { packages: update });
            if (current()) {
                setSaved(next);
                onSaved?.();
            }
        } catch (caught: unknown) {
            if (current()) setError(normalizeRequestError(caught));
        } finally {
            saving.current = false;
            if (current()) setBusy(false);
        }
    };
    return { snapshot, busy, error, save, reload: () => { setError(undefined); reloadSettings(); } };
}

function RepositoryVisibility({ repository, display, labelHidden }: {
    repository: RepositorySnapshot;
    display: ReturnType<typeof usePackageDisplaySettings>;
    labelHidden?: boolean;
}) {
    const { t, locale } = useTranslation();
    const id = repository.repositoryId;
    const hidden = display.snapshot?.settings.packages.hiddenRepositoryIds;
    const checked = id !== undefined && hidden !== undefined && !hidden.includes(id);
    const name = repositoryDisplayName(t, repository, sourceText(t, locale, repository));
    return <Checkbox
        checked={checked}
        disabled={id === undefined || hidden === undefined || display.busy || checked && hidden.length >= 256}
        label={labelHidden ? t("Show packages from {name}", { name }) : name}
        labelHidden={labelHidden}
        onChange={(shown) => {
            if (id === undefined || hidden === undefined || shown === checked) return;
            const hiddenRepositoryIds = shown ? hidden.filter((value) => value !== id) : [...hidden, id];
            void display.save({ hiddenRepositoryIds });
        }}
    />;
}

function PackageFilters({ client, settings, onChanged, filter, setFilter, sourceFilter, setSourceFilter, canUseUserPackages }: {
    client: GuiRpcClient;
    settings: SettingsGetResult;
    onChanged(): void;
    filter: string;
    setFilter(value: "all" | NonNullable<CandidateFilter["status"]>): void;
    sourceFilter: string;
    setSourceFilter(value: "all" | "local" | "remote" | "user-package"): void;
    canUseUserPackages: boolean;
}) {
    const { t } = useTranslation();
    const [open, setOpen] = useState(false);
    const [repositories, setRepositories] = useState<RepositorySnapshot[]>();
    const [inventoryError, setInventoryError] = useState(false);
    useEffect(() => {
        if (!open) return;
        let active = true;
        setInventoryError(false); setRepositories(undefined);
        void listAllRepositories(client).then((items) => { if (active) setRepositories(items); }, () => { if (active) setInventoryError(true); });
        return () => { active = false; };
    }, [client, open, settings.revision]);
    const [confirmPrerelease, setConfirmPrerelease] = useState(false);
    const display = usePackageDisplaySettings(client, settings, onChanged, onChanged);
    const { busy, error, save } = display;
    const snapshot = display.snapshot ?? settings;
    const anchorRef = useRef<HTMLElement>(null);
    const failure = error === undefined ? null : <div role="alert">
        <p>{t(error.code === "revision_conflict" ? "Settings changed elsewhere. Reload filters before trying again." : requestErrorMessage(error.code))}</p>
        <code>{error.code}</code>
        {error.diagnosticId === undefined ? null : <p><code>{error.diagnosticId}</code></p>}
        <Button onClick={display.reload} type="button" variant="text">{t("Reload filters")}</Button>
    </div>;
    return <>
        <Button aria-expanded={open} aria-haspopup="dialog" className="package-filter-trigger" onClick={() => setOpen(!open)} ref={anchorRef} type="button" variant="tonal">{t("Filter packages")}</Button>
        <FilterPopover anchorRef={anchorRef} label={t("Package filters")} onClose={() => setOpen(false)} open={open}>
            <fieldset disabled={busy} aria-busy={busy}>
                <legend>{t("Repositories")}</legend>
                {inventoryError ? <p role="alert">{t("Unable to load repositories")}</p> : repositories === undefined ? <p role="status">{t("Loading…")}</p> : null}
                {(repositories ?? []).map((repository) => repository.repositoryId === undefined ? null
                    : <RepositoryVisibility display={display} key={repository.repositoryId} repository={repository} />)}
                {canUseUserPackages ? <Checkbox checked={!snapshot.settings.packages.hideLocalUserPackages} disabled={busy} label={t("Local User Packages")} onChange={(checked) => void save({ hideLocalUserPackages: !checked })} /> : null}
            </fieldset>
            <fieldset disabled={busy} aria-busy={busy}>
                <legend>{t("Other filters")}</legend>
                <Checkbox checked={snapshot.settings.packages.showPrerelease} disabled={busy} label={t("Show prerelease versions")} onChange={(checked) => { if (checked) { setOpen(false); setConfirmPrerelease(true); } else void save({ showPrerelease: false }); }} />
                <Select disabled={busy} label={t("Status")} onChange={(value) => setFilter(value as "all" | NonNullable<CandidateFilter["status"]>)} options={[{ label: t("All packages"), value: "all" }, { label: t("Installed"), value: "installed" }, { label: t("Available"), value: "available" }, { label: t("Missing source"), value: "missing" }]} value={filter} />
                <Select disabled={busy} label={t("Source")} onChange={(value) => setSourceFilter(value as "all" | "local" | "remote" | "user-package")} options={[{ label: t("All"), value: "all" }, { label: t("Remote"), value: "remote" }, { label: t("Local repository"), value: "local" }, ...(canUseUserPackages ? [{ label: t("User Packages"), value: "user-package" }] : [])]} value={sourceFilter} />
            </fieldset>
            {failure}
        </FilterPopover>
        <Dialog onClose={() => { setConfirmPrerelease(false); setOpen(true); }} open={confirmPrerelease} title={t("Show prerelease versions?")}>
            <p>{t("Prerelease packages may be unstable. Showing them does not install or update any package.")}</p>
            <div className="dialog-actions"><DialogCloseButton variant="text">{t("Cancel")}</DialogCloseButton><DialogCloseButton afterClose={() => { setConfirmPrerelease(false); setOpen(true); void save({ showPrerelease: true }); }}>{t("Show prerelease versions")}</DialogCloseButton></div>
        </Dialog>
    </>;
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

const packageWorkspaceChanges = ["projects", "repositories", "user-packages", "unity"] as const;

export function primePackageWorkspaceRead(client: GuiRpcClient, events: CoreEvents, scope: ReturnType<typeof createPageRead>, projectId: string, canQuery: boolean): void {
    void primePageRead(scope, "primary", () => events.consistentRead(packageWorkspaceChanges,
        () => loadPackageWorkspace(client, projectId, canQuery, {}, scope.deferUntil)));
}

async function loadPackageWorkspace(client: GuiRpcClient, projectId: string, canQuery: boolean, sourceSelections: Record<string, string>, deferUntil?: number): Promise<PackageWorkspaceValue> {
    const project = (await client.projectGet(projectId)).project;
    if (project.revision === undefined) throw { code: "project_not_registered" };
    const sources = Object.entries(sourceSelections).filter(([, key]) => key.length > 0).map(([packageId, key]) => ({ packageId, source: key.startsWith("repository:") ? { kind: "repository" as const, repository_id: key.slice(11) } : { kind: "user_package" as const, user_package_id: key.slice(13) } }));
    const candidateRequest: ProjectCandidateRequest = { projectId, expectedRevision: project.revision, view: { kind: "summary", sources } };
    const [settings, launchOptions] = await Promise.all([
        client.settingsGet(),
        client.unityLaunchOptions(projectId, project.revision, true)
    ]);
    if (!canQuery) return { project, launchOptions, settings, sourceSelectionKey: JSON.stringify(sourceSelections), candidateError: "capability_required" };
    // The initial route may wait only for the remainder of its 100ms budget.
    // Keep the same settled read after timeout so mounting never duplicates it.
    const candidateRead = readCandidateSummaryPage(client, candidateRequest).then(
        (first): PageReadResult<CandidateSummaryPage> => first.snapshot.configRevision === settings.revision
            ? { kind: "ready", value: first } : { kind: "error", error: { code: "package_candidate_evidence_stale" } },
        (error: unknown): PageReadResult<CandidateSummaryPage> => ({ kind: "error", error })
    );
    const remaining = (deferUntil ?? 0) - performance.now();
    let initialCandidates: PageReadResult<CandidateSummaryPage> | undefined;
    if (remaining > 0) {
        let timer: number | undefined;
        try {
            initialCandidates = await Promise.race([candidateRead, new Promise<undefined>((resolve) => { timer = window.setTimeout(resolve, remaining); })]);
        } finally {
            window.clearTimeout(timer);
        }
    }
    return { project, launchOptions, settings, sourceSelectionKey: JSON.stringify(sourceSelections), candidateRequest, candidateRead, initialCandidates };
}

async function reloadPackageWorkspace(client: GuiRpcClient, projectId: string, canQuery: boolean, sources: Record<string, string>, previous: PackageWorkspaceValue, selectedIds: string[], filter?: CandidateFilter): Promise<PackageWorkspaceValue> {
    const value = await loadPackageWorkspace(client, projectId, canQuery, sources);
    // An explicit visibility/source change must still withdraw old provenance
    // immediately. Ordinary background refresh replaces the whole view at once.
    if (value.settings.revision !== previous.settings.revision || value.sourceSelectionKey !== previous.sourceSelectionKey || !value.candidateRead || !value.candidateRequest) return value;
    const result = await value.candidateRead;
    if (result.kind === "error") throw result.error;
    const first = result.value;
    const request = { ...value.candidateRequest, expectedSnapshot: first.snapshot.fingerprint };
    const [candidates, bulk, filtered] = await Promise.all([
        finishCandidateSummary(client, request, first, () => true),
        queryCandidates(client, { ...request, view: { ...request.view, kind: "summary", bulkPackageIds: selectedIds }, limit: 64 }).then((page) => {
            if (page.view !== "summary" || !page.bulk || page.projectId !== projectId || page.snapshot.fingerprint !== first.snapshot.fingerprint) throw { code: "package_candidate_evidence_stale" };
            return page.bulk;
        }),
        filter === undefined ? undefined : (async () => {
            const filteredRequest: ProjectCandidateRequest = { ...request, view: { ...request.view, kind: "summary", filter } };
            return finishCandidateSummary(client, filteredRequest, await readCandidateSummaryPage(client, filteredRequest), () => true);
        })()
    ]);
    if (!candidates) throw { code: "package_candidate_evidence_stale" };
    return { ...value, prepared: { first, candidates, bulk, selectedIds, filter, filteredItems: filtered?.items } };
}

type PackageCandidateReadState = { value: PackageWorkspaceValue; first?: CandidateSummaryPage; candidates?: CandidateSummarySet; error?: string };

function initialPackageCandidates(value: PackageWorkspaceValue): PackageCandidateReadState | undefined {
    if (value.prepared) return { value, first: value.prepared.first, candidates: value.prepared.candidates };
    const initial = value.initialCandidates;
    return initial?.kind === "ready" ? { value, first: initial.value, ...(initial.value.nextCursor === null ? { candidates: initial.value } : {}) }
        : initial?.kind === "error" ? { value, error: normalizeRequestError(initial.error).code } : undefined;
}

function PackageWorkspaceData({ client, value, filter, selectedIds, children }: {
    client: GuiRpcClient;
    value: PackageWorkspaceValue;
    filter?: CandidateFilter;
    selectedIds: string[];
    children(data: {
        candidates: CandidateSummarySet | undefined;
        displayedItems: PackageCandidateSummary[] | undefined;
        candidateError: string | undefined;
        candidatesPending: boolean;
        filteredItems: PackageCandidateSummary[] | undefined;
        filterPending: boolean;
        filterError: string | undefined;
        allBulk: CandidateBulkActions | undefined;
        selectedBulk: CandidateBulkActions | undefined;
        bulkError: string | undefined;
    }): ReactNode;
}) {
    const [complete, setComplete] = useState<PackageCandidateReadState | undefined>(() => initialPackageCandidates(value));
    const [filtered, setFiltered] = useState<{ value: PackageWorkspaceValue; filter: CandidateFilter; items?: PackageCandidateSummary[]; error?: string }>();
    const [bulk, setBulk] = useState<{ value: PackageWorkspaceValue; selectedIds: string[]; result?: CandidateBulkActions; error?: string }>();
    const resolved = complete?.value === value ? complete : initialPackageCandidates(value);
    const first = resolved?.first;
    const candidates = resolved?.error === undefined ? resolved?.candidates ?? first : undefined;
    const previousCandidates = complete !== undefined && complete.value.project.projectId === value.project.projectId
        && complete.value.settings.revision === value.settings.revision ? complete.candidates ?? complete.first : undefined;
    const displayedItems = (candidates ?? previousCandidates)?.items;
    const candidatesPending = value.candidateRequest !== undefined && resolved?.error === undefined && resolved?.candidates === undefined;
    useEffect(() => {
        const { candidateRequest, candidateRead } = value;
        if (!candidateRequest || !candidateRead) return;
        if (value.prepared || value.initialCandidates?.kind === "error" || (value.initialCandidates?.kind === "ready" && value.initialCandidates.value.nextCursor === null)) {
            setComplete((previous) => previous?.value === value ? previous : initialPackageCandidates(value));
            return;
        }
        let active = true;
        void candidateRead.then(async (result) => {
            if (!active) return;
            if (result.kind === "error") throw result.error;
            const first = result.value;
            if (first.nextCursor === null) {
                setComplete({ value, first, candidates: first });
                return;
            }
            setComplete({ value, first });
            const candidates = await finishCandidateSummary(client, candidateRequest, first, () => active);
            if (active && candidates !== undefined) setComplete({ value, first, candidates });
        }).catch((caught: unknown) => {
            if (active) setComplete({ value, error: normalizeRequestError(caught).code });
        });
        return () => { active = false; };
    }, [client, value]);
    useEffect(() => {
        const { candidateRequest } = value;
        if (!filter || !first || candidateRequest?.view.kind !== "summary" || value.prepared?.filter === filter) return;
        const request: ProjectCandidateRequest = {
            ...candidateRequest,
            expectedSnapshot: first.snapshot.fingerprint,
            view: { ...candidateRequest.view, filter }
        };
        let active = true;
        const timeout = window.setTimeout(() => {
            void readCandidateSummaryPage(client, request).then(async (page) => {
                if (!active) return;
                const result = page.nextCursor === null
                    ? page : await finishCandidateSummary(client, request, page, () => active);
                if (active && result) setFiltered({ value, filter, items: result.items });
            }).catch((caught: unknown) => {
                if (active) setFiltered({ value, filter, error: normalizeRequestError(caught).code });
            });
        }, 120);
        return () => { active = false; window.clearTimeout(timeout); };
    }, [client, filter, first, value]);
    useEffect(() => {
        const { candidateRequest } = value;
        if (!first || candidateRequest?.view.kind !== "summary" || value.prepared?.selectedIds === selectedIds) return;
        const request: ProjectCandidateRequest = {
            ...candidateRequest,
            expectedSnapshot: first.snapshot.fingerprint,
            view: { ...candidateRequest.view, bulkPackageIds: selectedIds }
        };
        let active = true;
        const timer = window.setTimeout(() => {
            void queryCandidates(client, request).then((page) => {
                if (!active) return;
                if (page.view !== "summary" || !page.bulk || page.snapshot.fingerprint !== first.snapshot.fingerprint) {
                    throw { code: "package_candidate_evidence_stale" };
                }
                setBulk({ value, selectedIds, result: page.bulk });
            }).catch((cause: unknown) => {
                if (active) setBulk({ value, selectedIds, error: normalizeRequestError(cause).code });
            });
        }, selectedIds.length ? 80 : 0);
        return () => { active = false; window.clearTimeout(timer); };
    }, [client, selectedIds, first, value]);
    const visible = filtered?.value === value && filtered.filter === filter ? filtered
        : value.prepared !== undefined && value.prepared.filter === filter && filter !== undefined ? { items: value.prepared.filteredItems, error: undefined } : undefined;
    const currentBulk = bulk?.value === value ? bulk
        : value.prepared ? { selectedIds: value.prepared.selectedIds, result: value.prepared.bulk, error: undefined } : undefined;
    return <>{children({
        candidates,
        displayedItems,
        candidateError: resolved?.error ?? value.candidateError,
        candidatesPending,
        filteredItems: visible?.items,
        filterPending: filter !== undefined && value.candidateRequest !== undefined && resolved?.error === undefined && visible === undefined,
        filterError: visible?.error,
        allBulk: currentBulk?.result,
        selectedBulk: currentBulk?.selectedIds === selectedIds ? currentBulk?.result : undefined,
        bulkError: currentBulk?.selectedIds === selectedIds ? currentBulk?.error : undefined
    })}</>;
}

function ProjectPackageWorkspace({ client, navigate, projectId }: PageProps & { projectId: string }) {
    const { t, locale } = useTranslation();
    const current = useActionScope(client);
    const pageRead = useContext(PageReadContext);

    const canManageRepositories = useCapability(capabilities.repositoriesRegistry);
    const canPlanPackagesV1 = useCapability(capabilities.packagesPlanV1);
    const canPlanPackagesV2 = useCapability(capabilities.packagesPlanV2);
    const canUseUserPackages = useCapability(capabilities.packagesUserPackages);
    const canQueryCandidates = useCapability(capabilities.packagesCandidates);
    const [search, setSearch] = useState("");
    const [filter, setFilter] = useState<"all" | NonNullable<CandidateFilter["status"]>>("all");
    const [sourceFilter, setSourceFilter] = useState<"all" | "local" | "remote" | "user-package">("all");
    const candidateFilter = useMemo<CandidateFilter | undefined>(() => {
        if (search === "" && filter === "all" && sourceFilter === "all") return undefined;
        return {
            ...(search === "" ? {} : { search }),
            ...(filter === "all" ? {} : { status: filter }),
            ...(sourceFilter === "all" ? {} : { source: sourceFilter })
        };
    }, [search, filter, sourceFilter]);
    const [sourceSelections, setSourceSelections] = useState<Record<string, string>>({});
    const [bulkSelection, setBulkSelection] = useState<string[]>([]);
    const readView = useRef({ selectedIds: bulkSelection, filter: candidateFilter });
    useEffect(() => { readView.current = { selectedIds: bulkSelection, filter: candidateFilter }; }, [bulkSelection, candidateFilter]);
    const load = useCallback(() => loadPackageWorkspace(client, projectId, canQueryCandidates, sourceSelections, pageRead?.deferUntil), [canQueryCandidates, client, projectId, sourceSelections, pageRead]);
    const reload = useCallback((previous: PackageWorkspaceValue) => reloadPackageWorkspace(client, projectId, canQueryCandidates, sourceSelections, previous, readView.current.selectedIds, readView.current.filter), [canQueryCandidates, client, projectId, sourceSelections]);
    const [evidenceInvalid, setEvidenceInvalid] = useState(false);
    const [packageMutationBusy, setPackageMutationBusy] = useState(false);
    useEffect(() => setEvidenceInvalid(false), [load]);
    const selectionAnchor = useRef<string | undefined>(undefined);
    const [repositoriesRefreshing, setRepositoriesRefreshing] = useState(false);
    const [repositoryRefreshError, setRepositoryRefreshError] = useState<RpcError>();
    const repositoryRefreshPending = useRef(false);
    useEffect(() => { selectionAnchor.current = undefined; }, [search, filter, sourceFilter]);
    useEffect(() => { if (bulkSelection.length === 0) selectionAnchor.current = undefined; }, [bulkSelection.length]);
    const [selection, setSelection] = useState<PackageActionSelection>();
    const selectedSource = (row: PackageWorkspaceRow): PackageSourceSelector | undefined => {
        const selectedKey = sourceSelections[row.packageId];
        if (selectedKey?.startsWith("repository:")) return { kind: "repository", repositoryId: selectedKey.slice(11) };
        if (selectedKey?.startsWith("user-package:")) return { kind: "user_package", userPackageId: selectedKey.slice(13) };
        return undefined;
    };
    const selectAction = (action: PackageActionSelection["action"], origin: DialogOrigin, row?: PackageWorkspaceRow, version?: string) => {
        const packageId = row?.packageId ?? "";
        const source = row === undefined ? undefined : selectedSource(row);
        setSelection({ action, key: (selection?.key ?? 0) + 1, origin, packageId, ...(version === undefined ? {} : { version }), ...(source === undefined ? {} : { source }) });
    };
    const refreshPackages = async (reload: () => void) => {
        if (repositoryRefreshPending.current) return;
        repositoryRefreshPending.current = true;
        setRepositoriesRefreshing(true);
        setRepositoryRefreshError(undefined);
        try {
            const { items } = await client.repositoriesRefreshAll();
            if (current()) {
                const failure = items.find((item) => item.status === "failed");
                if (failure?.status === "failed") setRepositoryRefreshError(normalizeRequestError({ code: failure.code }));
                reload();
            }
        } catch (caught: unknown) {
            if (current()) setRepositoryRefreshError(normalizeRequestError(caught));
        } finally {
            repositoryRefreshPending.current = false;
            setRepositoriesRefreshing(false);
        }
    };
    return (
        <ResourcePage load={load} reload={reload} changes={packageWorkspaceChanges}>{(value, refresh, _refreshing, refreshError) => <PackageWorkspaceData client={client} filter={candidateFilter} selectedIds={bulkSelection} value={value}>{({ candidates, displayedItems, candidateError, candidatesPending, filteredItems, filterPending, filterError, allBulk, selectedBulk, bulkError }) => {
            const { project, launchOptions, settings, sourceSelectionKey } = value;
            const workspaceRows = candidatesPending && displayedItems === undefined ? [] : packageWorkspaceRows(t, project, displayedItems);
            const currentEvidence = candidates !== undefined && sourceSelectionKey === JSON.stringify(sourceSelections) && !evidenceInvalid;
            const catalogReady = currentEvidence && !candidatesPending && candidates.catalogComplete && !packageMutationBusy;
            const reloadEvidence = () => { setEvidenceInvalid(false); refresh(); };
            const readError = filterError ?? candidateError;
            const evidenceError = readError ?? bulkError;
            const evidenceFailure = evidenceError ? t(requestErrorMessage(evidenceError)) : evidenceInvalid ? t("Candidate evidence is stale.") : undefined;
            const repositoryRefreshFailure = repositoryRefreshError === undefined ? undefined : t("Refresh failed: ") + t(requestErrorMessage(repositoryRefreshError.code));
            const chooseCandidate = (row: PackageWorkspaceRow, option: PackageCandidateEvidence, origin: DialogOrigin) => {
                if (!catalogReady || !canPlanPackagesV1 || !canPlanPackagesV2 || !(actionableVersion(option) || option.manualUnityOverrideAllowed === true)) return;
                setSourceSelections((current) => ({ ...current, [row.packageId]: candidateSourceKey(option.source) }));
                setSelection({ action: option.relation === "older" ? "downgrade" : option.relation === "newer" ? "upgrade" : "install", key: (selection?.key ?? 0) + 1, origin, packageId: row.packageId, version: option.relation === "older" ? option.version : `=${option.version}`, source: candidateSource(option.source), includePrerelease: option.classification === "prerelease", ...(option.manualUnityOverrideAllowed ? { allowUnityIncompatible: true } : {}), candidateSnapshot: candidates.snapshot.fingerprint });
            };
            const bulkReady = (intents: CandidateBulkIntent[] | undefined) => intents?.length && intents.every((intent) => intent.kind === "remove") ? currentEvidence && !candidatesPending && !packageMutationBusy : catalogReady;
            const bulkDisabled = (intents: CandidateBulkIntent[] | undefined) => !canPlanPackagesV2 || !bulkReady(intents) || !intents?.length;
            const submitBulk = (intents: CandidateBulkIntent[] | undefined, origin: DialogOrigin, action?: CandidateBulkAction) => {
                if (!candidates || !bulkReady(intents) || !canPlanPackagesV2 || !intents?.length) return;
                const names = new Map(workspaceRows.map((row) => [row.packageId, row.displayName]));
                const exclusions = action?.exclusions.map(({ packageId, update }) => `${names.get(packageId) ?? packageId}: ${updateReason(update, t)}`) ?? [];
                if (action?.omittedExclusions) exclusions.push(t("{count} more packages were not included.", { count: action.omittedExclusions }));
                setSelection({ action: "bulk-candidates", key: (selection?.key ?? 0) + 1, origin, packageId: "", intents: intents.map(bulkPlanIntent), exclusions, candidateSnapshot: candidates.snapshot.fingerprint });
            };
            const allLatest = allBulk?.updateAll;
            const allStable = allBulk?.updateAllStable;
            const selectedLatest = selectedBulk?.installUpdate;
            const selectedStable = selectedBulk?.installUpdateStable;
            const rows = candidateFilter === undefined ? workspaceRows : packageWorkspaceRows(t, project, filteredItems ?? []);
            return (
                <section className="project-workspace">
                    <PageHeader
                        className="project-workspace-header"
                        leading={<Button className="project-back-action" onClick={() => navigate("/projects")} type="button" variant="text"><Icon asset={arrowBackIcon} slot="icon" />{t("Back")}</Button>}
                        subtitle={<span title={displayPath(project.rootPath)}>{displayPath(project.rootPath)}</span>}
                        title={projectName(t, locale, project)}
                    >
                        <nav aria-label={t("Project actions")} className="project-workspace-action-cluster">
                            <ProjectUnityWorkspaceActions client={client} launchOptions={launchOptions} navigate={navigate} onChanged={refresh} project={project} renderLaunch={(launchAction) => <ProjectRowActions
                                client={client}
                                context="workspace"
                                launchAction={launchAction}
                                navigate={navigate}
                                onChanged={refresh}
                                onCopyCompleted={(targetProjectId) => navigate(`/projects/${targetProjectId}`)}
                                onProjectChanged={() => refresh()}
                                onRemoved={() => navigate("/projects")}
                                project={project}
                            />} />
                        </nav>
                    </PageHeader>
                    <section aria-labelledby="packages-heading" className="package-workspace-surface">
                        <header className="alcomd-filter-bar package-workspace-toolbar">
                            <h2 id="packages-heading">{t("Manage packages")}</h2>
                             <RefreshButton className="package-refresh-action" disabled={(evidenceFailure ? !canQueryCandidates : !refreshError && !canManageRepositories) || repositoriesRefreshing} label={evidenceFailure || refreshError ? t("Retry") : undefined} onClick={() => evidenceFailure ? reloadEvidence() : refreshError ? refresh() : void refreshPackages(refresh)} refreshing={repositoriesRefreshing} refreshingLabel={t("Refreshing packages")} title={evidenceFailure ?? capabilityUnavailableTitle(canManageRepositories, capabilities.repositoriesRegistry, t) ?? repositoryRefreshFailure ?? t("Refresh packages")} />
                             <SearchField className="package-workspace-search" label={t("Search packages")} onInput={setSearch} placeholder={t("Search...")} value={search} />
                             {allLatest ? <>
                                 <Button disabled={bulkDisabled(allLatest?.intents)} onClick={(event) => submitBulk(allLatest?.intents, { current: event.currentTarget }, allLatest)} type="button" variant="tonal">{t("Update All ({count})", { count: allLatest?.targetCount ?? 0 })}</Button>
                                 {allStable ? <Button disabled={bulkDisabled(allStable.intents)} onClick={(event) => submitBulk(allStable.intents, { current: event.currentTarget }, allStable)} type="button" variant="text">{t("Update All Stable ({count})", { count: allStable.targetCount })}</Button> : null}
                             </> : null}
                             <div className="package-workspace-secondary-actions">
                                <span className="visually-hidden" role="status" aria-live="polite">{t(rows.length === 1 ? "{count} package" : "{count} packages", { count: rows.length })}</span>
                                <PackageWorkspaceMoreMenu canPlanV1={canPlanPackagesV1 && catalogReady} canPlanV2={canPlanPackagesV2 && catalogReady} onResolve={(origin) => selectAction("resolve", origin)} onReinstallAll={(origin) => selectAction("reinstall-all", origin)} />
                                <PackageFilters canUseUserPackages={canUseUserPackages} client={client} filter={filter} onChanged={refresh} setFilter={setFilter} setSourceFilter={setSourceFilter} settings={settings} sourceFilter={sourceFilter} />
                            </div>
                         </header>
                        {bulkSelection.length === 0 ? null : (
                            <div aria-label={t("Selected package actions")} className="package-bulk-bar" role="region">
                                <strong>{t("{count} selected", { count: bulkSelection.length })}</strong>
                                <Button disabled={bulkDisabled(selectedLatest?.intents)} onClick={(event) => submitBulk(selectedLatest?.intents, { current: event.currentTarget })} type="button" variant="tonal">{t("Install / Update selected")}</Button>
                                {selectedStable ? <Button disabled={bulkDisabled(selectedStable.intents)} onClick={(event) => submitBulk(selectedStable.intents, { current: event.currentTarget })} type="button" variant="text">{t("Install / Update selected stable")}</Button> : null}
                                <Button disabled={bulkDisabled(selectedBulk?.reinstall)} onClick={(event) => submitBulk(selectedBulk?.reinstall, { current: event.currentTarget })} title={capabilityUnavailableTitle(canPlanPackagesV2, capabilities.packagesPlanV2, t)} type="button" variant="tonal">{t("Reinstall selected")}</Button>
                                <Button disabled={bulkDisabled(selectedBulk?.remove)} onClick={(event) => submitBulk(selectedBulk?.remove, { current: event.currentTarget })} title={capabilityUnavailableTitle(canPlanPackagesV2, capabilities.packagesPlanV2, t)} type="button" variant="text">{t("Remove selected")}</Button>
                                <Button onClick={() => setBulkSelection([])} type="button" variant="text">{t("Clear selection")}</Button>
                            </div>
                        )}
                        <div aria-busy={candidatesPending || filterPending} className="package-workspace-list-region">
                            <DesktopList
                                className="package-workspace-list"
                                emptyMessage={candidatesPending || filterPending ? <LoadingState title={t("Loading")} variant="bar" /> : filterError ? <p role="alert">{evidenceFailure}</p> : readError ? undefined : <section className="projects-empty" role="status"><h3>{workspaceRows.length === 0 ? t("No packages") : t("No matching packages")}</h3><p>{workspaceRows.length === 0 ? t("This project has no packages to manage.") : t("Change the search or package filter.")}</p></section>}
                                columns={[
                                    { key: "select", header: <Checkbox checked={rows.length > 0 && rows.every((row) => bulkSelection.includes(row.packageId))} disabled={rows.length === 0} label={t("Select all visible packages")} onChange={(checked) => setBulkSelection((current) => checked ? [...new Set([...current, ...rows.map((row) => row.packageId)])] : current.filter((id) => !rows.some((row) => row.packageId === id)))} />, width: 48, className: "package-column-select" },
                                    { key: "package", header: t("Package"), width: "100%", className: "package-column-name" },
                                    { key: "installed", header: t("Installed"), width: 220, className: "package-column-installed" },
                                    { key: "latest", header: t("Latest"), width: 160, className: "package-column-latest" },
                                    { key: "source", header: t("Source"), width: 210, className: "package-column-source" },
                                    { key: "actions", header: <span className="visually-hidden">{t("Actions")}</span>, width: 112, className: "package-column-actions" }
                                ]}
                                label={t("Packages")}
                                minWidth={880}
                                sizing="fixed"
                                rows={rows.map((row) => {
                                    const evidence = currentEvidence ? row.evidence : undefined;
                                    const selectRow = (checked: boolean, shiftKey: boolean) => {
                                        const anchor = rows.findIndex((item) => item.packageId === selectionAnchor.current);
                                        const end = rows.findIndex((item) => item.packageId === row.packageId);
                                        const ids = shiftKey && anchor >= 0 ? rows.slice(Math.min(anchor, end), Math.max(anchor, end) + 1).map((item) => item.packageId) : [row.packageId];
                                        setBulkSelection((current) => checked ? [...new Set([...current, ...ids])] : current.filter((id) => !ids.includes(id)));
                                        selectionAnchor.current = row.packageId;
                                    };
                                    return { key: row.packageId, className: row.installedVersion === undefined ? "package-row--available" : "package-row--installed", cells: [
                                        <Checkbox checked={bulkSelection.includes(row.packageId)} label={t("Select {value0}", { value0: row.displayName })} onChange={selectRow} />,
                                        <><strong title={evidence?.description}>{row.displayName}</strong><small>{row.packageId}</small></>,
                                        <PackageVersionPicker client={client} project={project} source={selectedSource(row)} snapshot={candidates?.snapshot.fingerprint} disabled={!canQueryCandidates || !currentEvidence} canPlan={catalogReady && canPlanPackagesV1 && canPlanPackagesV2} onStale={() => setEvidenceInvalid(true)} onSelect={(option, origin) => chooseCandidate(row, option, origin)} row={row} />,
                                        <PackageLatestInfo canPlan={catalogReady && canPlanPackagesV1 && canPlanPackagesV2} evidence={evidence} onSelect={(option, origin) => chooseCandidate(row, option, origin)} row={row} />,
                                        row.sourceOptions.length === 0 ? <span>{evidence === undefined ? "—" : t("Unknown source")}</span> : <PackageSourcePicker disabled={!currentEvidence} label={t("Source for {value0}", { value0: row.displayName })} onChange={(key) => setSourceSelections((current) => ({ ...current, [row.packageId]: key }))} options={row.sourceOptions} value={sourceSelections[row.packageId] ?? ""} />,
                                        <div className="package-row-actions"><PackagePrimaryAction canPlanV1={canPlanPackagesV1 && !packageMutationBusy && !evidenceInvalid} canPlanV2={canPlanPackagesV2} catalogReady={catalogReady} evidence={evidence} onRemove={(origin) => selectAction("remove", origin, row)} onSelect={(option, origin) => chooseCandidate(row, option, origin)} row={row} /><PackageRowMoreMenu canPlanV1={canPlanPackagesV1 && !packageMutationBusy && !evidenceInvalid} canPlanV2={canPlanPackagesV2 && catalogReady} client={client} onAction={(action, origin) => selectAction(action, origin, row)} row={row} /></div>
                                    ] };
                                })}
                            />
                        </div>
                    </section>
                    <PackageActions client={client} onChanged={reloadEvidence} onBusyChanged={setPackageMutationBusy} onEvidenceStale={() => setEvidenceInvalid(true)} project={project} returnFocusReady={currentEvidence && !candidatesPending && !packageMutationBusy} selection={selection === undefined ? undefined : { ...selection, candidateSnapshot: selection.candidateSnapshot ?? candidates?.snapshot.fingerprint }} />
                </section>
            );
        }}</PackageWorkspaceData>}</ResourcePage>
    );
}

function packageWorkspaceRows(t: Translate, project: ProjectSnapshot, summaries: PackageCandidateSummary[] | undefined): PackageWorkspaceRow[] {
    const installed = new Map(project.lockedDependencies.map((item) => [item.packageId, item.value]));
    const evidenceById = new Map(summaries?.map((item) => [item.packageId, item]));
    const packageIds = summaries?.map((item) => item.packageId) ?? [...new Set([...installed.keys(), ...project.directDependencies.map((item) => item.packageId)])];
    return orderPackageWorkspaceRows(packageIds.map((packageId) => {
        const evidence = evidenceById.get(packageId);
        const installedVersion = evidence === undefined ? installed.get(packageId) : evidence.installed.kind === "locked" ? evidence.installed.version : evidence.installed.kind === "unknown" ? t("Unknown") : undefined;
        const sourceOptions = (evidence?.providers ?? []).map((provider) => { const key = candidateSourceKey(provider.source); return { key, label: provider.displayName ?? (provider.builtin === "official" ? t("Official") : provider.builtin === "curated" ? t("Curated") : provider.name ?? t(provider.sourceKind === "user-package" ? "Local / User Package" : "Unnamed")), selector: candidateSource(provider.source) }; });
        return {
            ...(evidence ? { evidence } : {}),
            displayName: evidence?.displayName ?? packageId,
            ...(evidence?.links === undefined ? {} : { links: evidence.links }),
            ...(installedVersion === undefined ? {} : { installedVersion }),
            packageId,
            sourceOptions
        };
    }));
}

export function RepositoriesPage({ client, navigate }: PageProps) {
    const { t } = useTranslation();
    const load = useCallback(async () => {
        const [repositories, displaySettings] = await Promise.all([
            listAllRepositories(client),
            client.settingsGet().then((settings) => ({ settings }), (caught: unknown) => ({ error: normalizeRequestError(caught) }))
        ]);
        return { repositories, ...displaySettings };
    }, [client]);
    return <UtilityWorkspace changes={["repositories"]} title={t(navigationLabels.resources)} load={load} navigation={<ResourceNavigation current="/repositories" navigate={navigate} />} tools={(_, refresh) => <RepositoryToolbar client={client} onChanged={refresh} />}>
        {(value, refresh) => <RepositoryTable client={client} onChanged={refresh} {...value} />}
    </UtilityWorkspace>;
}

function RepositoryTable({ client, onChanged, repositories, settings, error }: {
    client: GuiRpcClient;
    onChanged(): void;
    repositories: RepositorySnapshot[];
    settings?: SettingsGetResult;
    error?: RpcError;
}) {
    const { t, locale } = useTranslation();
    const display = usePackageDisplaySettings(client, settings, onChanged);
    const [selectedId, setSelectedId] = useState<string>();
    const selected = selectedId === undefined ? undefined : repositories.find((repository) => repository.repositoryId === selectedId);
    const failure = display.error ?? error;
    return <>
        {failure === undefined ? null : <>
            <Button onClick={display.reload} type="button" variant="text">{t("Reload filters")}</Button>
        </>}
        {repositories.length === 0 ? <RouteState kind="empty" title={t("No repositories registered")} /> : <DesktopList
            columns={[
                { key: "visibility", header: <span className="visually-hidden">{t("Enabled")}</span>, width: 48 },
                { key: "repository", header: t("Repository"), width: "34%" },
                { key: "source", header: t("Source") },
                { key: "actions", header: t("Actions"), width: 144 }
            ]}
            label={t("Repositories")}
            minWidth={640}
            rows={repositories.map((repository) => {
                const source = sourceText(t, locale, repository);
                return {
                    key: repository.repositoryId ?? source,
                    cells: [
                        <RepositoryVisibility display={display} labelHidden repository={repository} />,
                        <strong>{repositoryDisplayName(t, repository, t("Repository"))}</strong>,
                        <span title={source}>{source}</span>,
                        <div className="card-actions">
                            <IconButton disabled={repository.repositoryId === undefined} label={t("Browse packages")} onClick={() => setSelectedId(repository.repositoryId)} title={t("Browse packages")} type="button"><Icon asset={packagesIcon} /></IconButton>
                            <RepositoryActions client={client} onChanged={onChanged} repository={repository} />
                        </div>
                    ]
                };
            })}
        />}
        {selected === undefined || selectedId === undefined ? null : <Dialog wide open title={repositoryDisplayName(t, selected, t("Repository packages"))} onClose={() => setSelectedId(undefined)}>
            {isBuiltinRepository(selected) && selected.refreshedAtMs === 0 ? <p>{t("Pending first sync")}</p> : <RepositoryPackageList client={client} repositoryId={selectedId} />}
            <div className="dialog-actions"><DialogCloseButton variant="text">{t("Close")}</DialogCloseButton></div>
        </Dialog>}
    </>;
}

function RepositoryPackageList({ client, repositoryId }: { client: GuiRpcClient; repositoryId: string }) {
    const { t, locale } = useTranslation();
    const load = useCallback(() => client.repositoryPackages(repositoryId), [client, repositoryId]);
    return <ResourcePage load={load}>{(catalog) => <PagedItems initialItems={catalog.packages} initialCursor={catalog.nextCursor} loadMore={async (cursor) => { const next = await client.repositoryPackages(repositoryId, cursor); return { items: next.packages, nextCursor: next.nextCursor }; }}>
        {(items) => <DataTable headers={[t("Package"), t("Version"), t("Status")]} rows={items.map((item) => [item.displayName ?? item.packageId, item.version, item.yanked ? t("Yanked") : t("Available")])} />}
    </PagedItems>}</ResourcePage>;
}

function ResourceNavigation({ current, navigate }: { current: string; navigate(path: string): void }) {
    const { t, locale } = useTranslation();
    const canUseUserPackages = useCapability(capabilities.packagesUserPackages);
    return <nav aria-label={t("Resources")} className="resource-navigation"><SegmentedButtons label={t("Resource sections")} value={current} onChange={navigate} options={[{ value: "/repositories", label: t("Repositories") }, { value: "/user-packages", label: t("User Packages"), disabled: !canUseUserPackages, title: capabilityUnavailableTitle(canUseUserPackages, capabilities.packagesUserPackages, t) }, { value: "/templates", label: t("Templates") }]} /></nav>;
}

function UtilityNavigation({ current, kind, navigate }: { current: string; kind: "settings"; navigate(path: string): void }) {
    const { t, locale } = useTranslation();
    const unity = useCapability(capabilities.unityRead);
    const items = [{ path: "/settings", label: t("Preferences") }, { path: "/unity", label: t("Unity installations") }];
    return <nav aria-label={t("Settings sections")} className="resource-navigation"><SegmentedButtons label={t("Settings sections")} value={current} onChange={navigate} options={items.map(({ path, label }) => ({ value: path, label, disabled: path === "/unity" && !unity }))} /></nav>;
}

function ActionDisclosure({ children, title }: { children: ReactNode; title: string }) {
    const [open, setOpen] = useState(false);
    return <section className="utility-disclosure"><Button aria-haspopup="dialog" onClick={() => setOpen(true)} type="button" variant="tonal">{title}</Button>{open ? <UtilityActionDialog title={title} onClose={() => setOpen(false)}>{children}</UtilityActionDialog> : null}</section>;
}

function BackTo({ label, navigate, path }: { label: string; navigate(path: string): void; path: string }) {
    return <Button onClick={() => navigate(path)} type="button" variant="text"><Icon asset={arrowBackIcon} slot="icon" />{label}</Button>;
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
    const { t, locale } = useTranslation();
    const capabilityState = useCapabilityState(capabilities.packagesUserPackages);
    const load = useCallback(() => listAllUserPackages(client), [client]);
    if (capabilityState === "checking") return <Page title={t(navigationLabels.resources)}><ResourceNavigation current="/user-packages" navigate={navigate} /><RouteState kind="loading" title={t("Checking capability")} /></Page>;
    if (capabilityState === "unavailable") return <Page title={t(navigationLabels.resources)}><ResourceNavigation current="/user-packages" navigate={navigate} /><RouteState kind="error" title={t("Feature unavailable")} detail={t("{capability} was not negotiated by the connected daemon.", { capability: capabilities.packagesUserPackages })} /></Page>;
    return <UtilityWorkspace changes={["user-packages"]} title={t(navigationLabels.resources)} load={load} navigation={<ResourceNavigation current="/user-packages" navigate={navigate} />} tools={(_, refresh) => <UserPackageEnroll client={client} onChanged={refresh} />}>{(packages, refresh) => <UserPackageManager client={client} onChanged={refresh} packages={packages} />}</UtilityWorkspace>;
}

function UserPackageEnroll({ client, onChanged }: { client: GuiRpcClient; onChanged(): void }) {
    const { t } = useTranslation();
    const [open, setOpen] = useState(false);
    const origin = useRef<HTMLElement>(null);
    return <>
        <Button aria-haspopup="dialog" onClick={() => setOpen(true)} ref={origin} type="button">{t("Enroll folder")}</Button>
        <DirectorySelectionDialog client={client} label={t("Source folder")} onChanged={onChanged} onClose={() => setOpen(false)}
            onConfirm={(path) => client.userPackageEnroll(path)} open={open} origin={origin} title={t("Add user package")} />
    </>;
}

function UserPackageManager({ client, onChanged, packages }: { client: GuiRpcClient; onChanged(): void; packages: UserPackageRecord[] }) {
    const { t, locale } = useTranslation();

    const current = useActionScope(client);
    const [busy, setBusy] = useState<string>();
    const pending = useRef(false);
    const [removing, setRemoving] = useState<UserPackageRecord>();
    const [selected, setSelected] = useState<string>();
    const detailsOrigin = useRef<HTMLElement>(null);
    const [error, setError] = useState<RpcError>();
    const refresh = async (item: UserPackageRecord) => {
        if (pending.current) return;
        pending.current = true;

        setBusy(`refresh:${item.userPackageId}`);
        setError(undefined);
        try {
            await client.userPackageRefresh(item.userPackageId, item.revision);
            if (!current()) return;

            onChanged();
        } catch (caught: unknown) {
            if (current() && normalizeRequestError(caught).code === "revision_conflict") onChanged();
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
            if (!current()) return;
            setRemoving(undefined);

            onChanged();
        } catch (caught: unknown) {
            if (current()) setError(safeError(t, locale, caught));
        } finally {
            setBusy(undefined);
            pending.current = false;
        }
    };
    return <section className="user-packages">{packages.length === 0 ? <RouteState kind="empty" title={t("No User Packages enrolled")} detail={t("Enroll a package folder to make it available as a source. Removing enrollment never deletes the source folder.")} /> : <UtilityTable label={t("User Packages")} headers={[t("Package"), t("Version"), t("Source"), t("Actions")]} rows={packages.map((item) => ({ key: item.userPackageId, cells: [<><strong>{item.displayName ?? item.packageId}</strong><small>{item.packageId}</small></>, item.version, <span title={item.userPackageId}>{t("Local / User Package")}<small>{t("revision ")}{item.revision}</small></span>, <div className="card-actions"><Button disabled={busy !== undefined} onClick={() => {  detailsOrigin.current = document.activeElement instanceof HTMLElement ? document.activeElement : null; setSelected(item.userPackageId); }} type="button" variant="text">{t("Details")}</Button><RefreshButton disabled={busy !== undefined} onClick={() => void refresh(item)} refreshing={busy === "refresh:" + item.userPackageId} /><IconButton disabled={busy !== undefined} label={busy === "remove:" + item.userPackageId ? t("Removing…") : t("Remove enrollment")} onClick={() => {  setError(undefined); setRemoving(item); }} title={t("Remove enrollment")} type="button"><Icon asset={closeIcon} /></IconButton></div>] }))} />}
        <Dialog onClose={() => setSelected(undefined)} open={selected !== undefined} returnFocus={detailsOrigin} title={t("User Package details")}>
            {selected === undefined ? null : <UserPackageDetails client={client} userPackageId={selected} />}
            <div className="dialog-actions"><DialogCloseButton variant="text">{t("Close")}</DialogCloseButton></div>
        </Dialog>
        {removing === undefined ? null : <Dialog open dismissible={busy === undefined} title={t("Remove User Package?")} onClose={() => { if (busy === undefined) { setRemoving(undefined); setError(undefined); } }}>
        <p><Translated message="Remove {name} ({version}) from your package sources?" values={{ name: <strong>{removing.displayName ?? removing.packageId}</strong>, version: removing.version }} /></p><p>{t("The source folder will stay on disk.")}</p>
        {error === undefined ? null : <div role="alert"><p>{t(requestErrorMessage((error).code))}</p><code>{(error).code}</code>{error.diagnosticId === undefined ? null : <p><code>{error.diagnosticId}</code></p>}</div>}
        <div className="dialog-actions"><DialogCloseButton disabled={busy !== undefined} variant="text">{t("Cancel")}</DialogCloseButton><Button disabled={busy !== undefined} onClick={() => void remove(removing)} type="button">{t("Remove enrollment")}</Button></div>
    </Dialog>}</section>;
}

function UserPackageDetails({ client, userPackageId }: { client: GuiRpcClient; userPackageId: string }) {
    const { t } = useTranslation();
    const load = useCallback(() => client.userPackageGet(userPackageId), [client, userPackageId]);
    return <ResourcePage load={load}>{({ userPackage }) => <dl className="dialog-summary">
        <Detail label={t("Name")} value={userPackage.displayName ?? userPackage.packageId} />
        <Detail label={t("Package")} value={userPackage.packageId} />
        <Detail label={t("Version")} value={userPackage.version} />
        <Detail label={t("Source folder")} value={displayPath(userPackage.sourceRootPath)} />
        <Detail label={t("Revision")} value={String(userPackage.revision)} />
        <Detail label={t("Archive SHA-256")} value={shortHash(userPackage.archiveSha256)} />
    </dl>}</ResourcePage>;
}

export function RepositoryDetailPage({ client, navigate, repositoryId }: PageProps & { repositoryId: string }) {
    const { t, locale } = useTranslation();
    const load = useCallback(async () => Promise.all([client.repositoryGet(repositoryId), client.repositoryPackages(repositoryId)]), [client, repositoryId]);
    return <UtilityWorkspace changes={["repositories"]} title={t("Repository")} back={<BackTo label={t("Repositories")} navigate={navigate} path="/repositories" />} load={load}>{([detail, catalog], refresh) => <>
        <dl className="detail-grid">
            <Detail label={t("Name")} value={repositoryDisplayName(t, detail.repository, t("Unnamed"))} />
            <Detail label={t("Source")} value={sourceText(t, locale, detail.repository)} />
            <Detail label={t("Revision")} value={String(detail.repository.revision ?? "—")} />
            <Detail label={t("Issues")} value={String(detail.repository.issues.length)} />
        </dl>
        <h2>{t("Packages")}</h2>
        <PagedItems initialItems={catalog.packages} initialCursor={catalog.nextCursor} loadMore={async (cursor) => { const next = await client.repositoryPackages(repositoryId, cursor); return { items: next.packages, nextCursor: next.nextCursor }; }}>{(items) => <DataTable headers={[t("Package"), t("Version"), t("Status")]} rows={items.map((item) => [item.displayName ?? item.packageId, item.version, item.yanked ? t("Yanked") : t("Available")])} />}</PagedItems>
        <ActionDisclosure title={t("Manage repository")}><RepositoryActions client={client} onChanged={refresh} repository={detail.repository} /></ActionDisclosure>
    </>}</UtilityWorkspace>;
}

export function TemplatesPage({ client, navigate }: PageProps) {
    const { t } = useTranslation();
    const available = useCapability(capabilities.projectTemplates);
    const navigation = <ResourceNavigation current="/templates" navigate={navigate} />;
    if (!available) return <section className="utility-workspace page-surface--utility" aria-labelledby="route-title">
        <PageHeader className="utility-workspace-toolbar" title={t(navigationLabels.resources)}>{navigation}</PageHeader>
    </section>;
    return <ProjectTemplatesWorkspace client={client} navigation={navigation} />;
}

export function UnityPage({ client, navigate }: PageProps) {
    const { t, locale } = useTranslation();
    const load = useCallback(() => loadAllUnityInstallations(client), [client]);
    return <UtilityWorkspace changes={["unity"]} title={t(navigationLabels.settings)} load={load} navigation={<UtilityNavigation current="/unity" kind="settings" navigate={navigate} />} action={{ label: t("Manage installations"), render: (installations, refresh) => <UnityRegistryActions client={client} installations={installations} onChanged={refresh} /> }}>{(installations) => <>{installations.length === 0 ? <RouteState kind="empty" title={t("No Unity installations registered")} /> : <UtilityTable label={t("Unity installations")} headers={[t("Version"), t("Architecture"), t("Source"), t("Installation")]} rows={installations.map((item) => ({ key: item.installationId, cells: [item.unityVersion, item.architecture, humanize(t, item.sourceKind), item.installationId] }))} />}</>}</UtilityWorkspace>;
}

export function ProjectUnityPage({ afterMigrationOpen = false, client, navigate, projectId }: PageProps & { afterMigrationOpen?: boolean; projectId: string }) {
    const { t, locale } = useTranslation();
    const load = useCallback(async () => {
        const project = (await client.projectGet(projectId)).project;
        if (project.revision === undefined) throw { code: "project_not_registered" };
        const [launchConfig, launchOptions, writer] = await Promise.all([
            client.unityProjectLaunchConfigGet(projectId),
            client.unityLaunchOptions(projectId, project.revision, true),
            client.unityWriterState(projectId).catch(() => undefined)
        ]);
        return { project, launchConfig, launchOptions, writer };
    }, [client, projectId]);
    return <UtilityWorkspace changes={["projects", "unity"]} title={t("Project Unity")} back={<BackTo label={t("Project packages")} navigate={navigate} path={`/projects/${projectId}/packages`} />} load={load}>{({ project, launchConfig, launchOptions, writer }, refresh) => <><dl className="detail-grid"><Detail label={t("Project Unity version")} value={project.unityVersion} /><Detail label={t("Exact installations")} value={String(launchOptions.exactMatchingInstallations.length)} /><Detail label={t("Writer observation")} value={writer?.state ?? t("Unknown")} /><Detail label={t("Arguments")} value={launchConfig.config.revision === 0 ? t("Use global defaults") : t("{count} arguments", { count: launchConfig.effectiveArguments.length })} /><Detail label={t("Observed")} value={writer === undefined ? "—" : formatTime(t, locale, writer.checkedAtMs)} /></dl><ProjectUnityActions afterMigrationOpen={afterMigrationOpen} client={client} launchConfig={launchConfig} launchOptions={launchOptions} onChanged={refresh} project={project} /></>}</UtilityWorkspace>;
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
    const { t, locale } = useTranslation();
    const [accepted, setAccepted] = useState<BackupCreateResult>();
    const load = useCallback(async () => ({ backups: await client.backupsList(projectId), project: (await client.projectGet(projectId)).project }), [client, projectId]);
    return <UtilityWorkspace changes={["backups"]} title={t("Backups")} back={<BackTo label={t("Project packages")} navigate={navigate} path={"/projects/" + projectId + "/packages"} />} load={load}
        action={{ label: t("Create backup"), render: (value) => <BackupCreatePanel client={client} navigate={navigate} onAccepted={setAccepted} project={value.project} /> }}>
        {(value, refresh) => <>
            <p>{projectName(t, locale, value.project)}</p>
            {accepted === undefined ? null : <OperationFollow client={client} key={accepted.operationId} onTerminal={refresh} operationId={accepted.operationId} background />}
            <PagedItems initialItems={value.backups.backups} initialCursor={value.backups.nextCursor} loadMore={async (cursor) => { const page = await client.backupsList(projectId, cursor); return { items: page.backups, nextCursor: page.nextCursor }; }}>
                {(backups) => backups.length === 0 ? <RouteState kind="empty" title={t("No backups for this project")} /> : <UtilityTable label={t("Backups")} headers={[t("Created"), t("Size"), t("Packages"), t("Actions")]} rows={backups.map((backup) => ({ key: backup.backupId, cells: [formatTime(t, locale, backup.createdAtMs), formatBytes(t, locale, backup.archiveBytes), backup.excludeVpmPackages ? t("VPM packages excluded") : t("VPM packages included"), <Button onClick={() => navigate("/backups/" + backup.backupId)} type="button" variant="text">{t("View backup")}</Button>] }))} />}
            </PagedItems>
        </>}
    </UtilityWorkspace>;
}

export function BackupDetailPage({ client, navigate, backupId }: PageProps & { backupId: string }) {
    const { t, locale } = useTranslation();
    const canRestore = useCapability(capabilities.backupsRestore);
    const [restoreOpen, setRestoreOpen] = useState(false);
    const load = useCallback(() => client.backupGet(backupId), [client, backupId]);
    return <UtilityWorkspace changes={["backups"]} title={t("Backup detail")} load={load} tools={() => <Button disabled={!canRestore} onClick={() => setRestoreOpen(true)} title={capabilityUnavailableTitle(canRestore, capabilities.backupsRestore, t)} type="button">{t("Restore backup")}</Button>}>
        {(backup) => <><BackTo label={t("Project backups")} navigate={navigate} path={`/projects/${backup.sourceProjectId}/backups`} /><dl className="detail-grid"><Detail label={t("Source project")} value={backup.sourceProjectId} /><Detail label={t("Created")} value={formatTime(t, locale, backup.createdAtMs)} /><Detail label={t("Archive size")} value={formatBytes(t, locale, backup.archiveBytes)} /><Detail label={t("Format")} value={`v${backup.formatVersion}`} /><Detail label={t("Compression")} value={backupCompressionLabel(t, backup.compressionMode)} /><Detail label={t("Integrity")} value={shortHash(backup.archiveSha256)} /></dl>
            {restoreOpen ? <RestoreProjectDialog backup={backup} client={client} onClose={() => setRestoreOpen(false)} onCompleted={(projectId) => navigate(`/projects/${projectId}`)} onViewOperation={(operationId) => navigate(`/operations/${operationId}`)} open /> : null}
        </>}
    </UtilityWorkspace>;
}

export function OperationsPage({ client, navigate }: PageProps) {
    const { t, locale } = useTranslation();
    const load = useCallback(() => client.operationsList(), [client]);
    return <UtilityWorkspace title={t(navigationLabels.taskCenter)} load={load}>{(value) => <OperationItems client={client} initial={value}>
        {(operations) => operations.length === 0 ? <RouteState kind="empty" title={t("No operations yet")} /> : <UtilityTable label={t("Tasks")} headers={[t("Task"), t("Status"), t("Updated"), t("Actions")]} rows={operations.map((operation) => ({ key: operation.operationId, cells: [humanize(t, operation.kind), <span className={"state-chip state-chip--" + operation.state}>{humanize(t, operation.state)}</span>, formatTime(t, locale, operation.updatedAtMs), <Button onClick={() => navigate("/operations/" + operation.operationId)} type="button" variant="text">{t("View operation")}</Button>] }))} />}
    </OperationItems>}</UtilityWorkspace>;
}

export function OperationDetailPage({ client, navigate, operationId }: PageProps & { operationId: string }) {
    const { t, locale } = useTranslation();
    const load = useCallback(() => client.operationGet(operationId), [client, operationId]);
    return <UtilityWorkspace title={t("Operation detail")} back={<BackTo label={t(navigationLabels.taskCenter)} navigate={navigate} path="/operations" />} load={load}>{(initialOperation, refresh) => <OperationFollow client={client} initialOperation={initialOperation} operationId={operationId}>
        {(operation, error) => operation === undefined ? <OperationProgress error={error} /> : <>
            <OperationProgress operation={operation} />
            <dl className="detail-grid"><Detail label={t("Kind")} value={humanize(t, operation.kind)} /><Detail label={t("Updated")} value={formatTime(t, locale, operation.updatedAtMs)} /></dl>
            <OperationResultLinks navigate={navigate} operation={operation} />
            <OperationActions client={client} key={operation.operationId} onChanged={refresh} operation={operation} />
        </>}
    </OperationFollow>}</UtilityWorkspace>;
}

function OperationResultLinks({ navigate, operation }: { navigate(path: string): void; operation: Operation }) {
    const { t } = useTranslation();
    if (operation.state !== "succeeded" || typeof operation.result !== "object" || operation.result === null) return null;
    const result = operation.result as Record<string, unknown>;
    const id = (value: unknown): value is string => typeof value === "string" && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(value);
    return <div className="action-row">
        {!id(result.projectId) ? null : <Button onClick={() => navigate(`/projects/${result.projectId}`)} type="button" variant="tonal">{t("Open project")}</Button>}
        {!id(result.backupId) ? null : <Button onClick={() => navigate(`/backups/${result.backupId}`)} type="button" variant="tonal">{t("View backup")}</Button>}
    </div>;
}

export function ExtensionsPage({ client, navigate }: PageProps) {
    const { t } = useTranslation();
    const load = useCallback(() => client.extensionsList(), [client]);
    const canManage = useCapability(capabilities.extensionsLifecycle);
    const canOpen = useCapability(capabilities.extensionsPortableUi);
    const current = useActionScope(client);
    const pending = useRef(false);
    const [busy, setBusy] = useState(false);
    const [operationId, setOperationId] = useState<string>();
    const [operationTerminal, setOperationTerminal] = useState(false);
    const toggle = async (id: string, revision: number, enabled: boolean, refresh: () => void) => {
        if (!canManage || pending.current) return;
        pending.current = true;
        setBusy(true);
        try {
            await (enabled ? client.extensionEnable(id, revision) : client.extensionDisable(id, revision));
            if (!current()) return;

            refresh();
        } catch {} finally {
            pending.current = false;
            if (current()) setBusy(false);
        }
    };
    return <><UtilityWorkspace changes={["extensions"]} title={t(navigationLabels.extensions)} load={load} action={{ label: t("Install extension"), disabled: operationId !== undefined && !operationTerminal, render: () => <ExtensionInstallPanel client={client} onAccepted={(id) => { setOperationId(id); setOperationTerminal(false); }} /> }}>{(value, refresh) => <>
        {operationId === undefined ? null : <OperationFollow client={client} key={operationId} onTerminal={() => { setOperationTerminal(true); refresh(); }} operationId={operationId} background />}
        <PagedItems initialItems={value.extensions} initialCursor={value.nextCursor} loadMore={async (cursor) => { const page = await client.extensionsList(cursor); return { items: page.extensions, nextCursor: page.nextCursor }; }}>{(extensions) => (
        <section className="extension-management" aria-labelledby="extension-management-title">
            <header className="extension-management-heading"><h2 id="extension-management-title">{t("Manage extensions")}</h2></header>
            <div className="extension-section-heading"><h3>{t("Installed")}</h3><span>{extensions.length}</span></div>
            {extensions.length === 0 ? <RouteState kind="empty" title={t("No extensions installed")} /> : <div className="extension-cards">{extensions.map((extension) => {
                const enabled = extension.desiredState === "enabled";
                const uninstalling = extension.desiredState === "uninstalling";
                const quarantined = extension.quarantineState === "quarantined";
                return <article className="extension-card" key={extension.extensionId} aria-label={extension.extensionId}>
                    <div className="extension-card-heading"><div><h4>{extension.extensionId}</h4><p>{t("Version ")}{extension.version}</p></div>{extension.ui?.protocol !== "portable-v1" ? null : <Button disabled={!canOpen || !enabled || uninstalling || quarantined || busy} onClick={() => navigate(`/extensions/${encodeURIComponent(extension.extensionId)}/ui`)} title={capabilityUnavailableTitle(canOpen, capabilities.extensionsPortableUi, t)} type="button" variant="tonal">{t("Open")}</Button>}</div>
                    <p className="extension-card-status">{humanize(t, extension.runtimeState)} · {humanize(t, extension.trustDecision)}{extension.quarantineState === "clear" ? "" : ` · ${humanize(t, extension.quarantineState)}`}</p>
                    <div className="extension-card-actions"><Button onClick={() => navigate(`/extensions/${encodeURIComponent(extension.extensionId)}`)} type="button" variant="text">{t("Manage extension")}</Button><Switch disabled={!canManage || busy || uninstalling || quarantined} label={t("Enabled")} selected={enabled} onChange={(next) => void toggle(extension.extensionId, extension.revision, next, refresh)} /></div>
                </article>;
            })}</div>}
        </section>
        )}</PagedItems>
    </>}</UtilityWorkspace></>;
}

export function ExtensionDetailPage({ client, navigate, extensionId }: PageProps & { extensionId: string }) {
    const { t } = useTranslation();
    const canUsePortableUi = useCapability(capabilities.extensionsPortableUi);
    const [operationId, setOperationId] = useState<string>();
    const [operationTerminal, setOperationTerminal] = useState(false);
    const load = useCallback(() => client.extensionGet(extensionId), [client, extensionId]);
    return <UtilityWorkspace changes={["extensions"]} title={t("Extension detail")} back={<BackTo label={t("Extensions")} navigate={navigate} path="/extensions" />} load={load}>{({ extension }, refresh) => <>
        <h2 className="extension-detail-identity">{extension.extensionId}</h2>
        <dl className="detail-grid"><Detail label={t("Version")} value={extension.version} /><Detail label={t("Publisher")} value={shortHash(extension.publisherFingerprint)} /><Detail label={t("Trust")} value={humanize(t, extension.trustDecision)} /><Detail label={t("Desired state")} value={humanize(t, extension.desiredState)} /><Detail label={t("Runtime")} value={humanize(t, extension.runtimeState)} /><Detail label={t("Quarantine")} value={humanize(t, extension.quarantineState)} /></dl>
        {extension.ui?.protocol === "portable-v1" ? <Button disabled={!canUsePortableUi || extension.desiredState !== "enabled" || extension.quarantineState === "quarantined" || (operationId !== undefined && !operationTerminal)} onClick={() => navigate(`/extensions/${encodeURIComponent(extensionId)}/ui`)} title={capabilityUnavailableTitle(canUsePortableUi, capabilities.extensionsPortableUi, t)} type="button">{t("Open Portable UI")}</Button> : null}
        <ExtensionActions client={client} extension={extension} onChanged={refresh} onAccepted={(id) => { setOperationId(id); setOperationTerminal(false); }} operationActive={operationId !== undefined && !operationTerminal} />
        {operationId === undefined ? null : <OperationFollow client={client} key={operationId} onTerminal={(operation) => { if (operation.state === "succeeded") navigate("/extensions"); else { setOperationTerminal(true); refresh(); } }} operationId={operationId} background />}
    </>}</UtilityWorkspace>;
}

export function AboutPage({ client }: PageProps) {
    const { t } = useTranslation();
    const load = useCallback(() => client.systemStatus(), [client]);
    return <section className="utility-workspace page-surface--utility" aria-labelledby="route-title">
        <PageHeader title={t(navigationLabels.about)} />
        <ScrollArea className="utility-workspace-body">
            <dl className="detail-grid"><Detail label={t("Product")} value={productFamily} /><Detail label={t("GUI version")} value={guiVersion} /></dl>
            <LocalPageReadContext.Provider value={true}>
                <ResourcePage load={load}>{(status) => <dl className="detail-grid"><Detail label={t("Daemon")} value={status.daemonVersion} /><Detail label="RPC" value={`v${status.rpcVersion}`} /><Detail label={t("State")} value={humanize(t, status.state)} /></dl>}</ResourcePage>
            </LocalPageReadContext.Provider>
            <section className="notice-card"><h2>{t("Licenses")}</h2><p>{t("ALCOMD source is licensed under AGPL-3.0-only.")}</p><p><Translated message="The canonical third-party dependency notice source is {file} in the product distribution." values={{ file: <code>THIRD_PARTY_NOTICES.md</code> }} /></p></section>
        </ScrollArea>
    </section>;
}

export function SettingsPage(props: PageProps & { settingsWriter: import("./settings-autosave").SettingsAutosave; onApplied(value: SettingsGetResult): void; }) {
    return <SettingsWorkspace {...props} />;
}

export function Page({ actions, children, title }: { actions?: ReactNode; children: ReactNode; title: string }) {
    return <section className="page-surface page-surface--utility" aria-labelledby="route-title"><PageHeader title={title}><div className="page-header-actions">{actions}</div></PageHeader>{children}</section>;
}

function Detail({ label, value }: { label: string; value: string }) { return <div><dt>{label}</dt><dd>{value}</dd></div>; }

function UtilityTable({ headers, label, rows }: { headers: string[]; label: string; rows: { key: string; cells: ReactNode[] }[] }) {
    return <DesktopList columns={headers.map((header, index) => ({ key: `${index}-${header}`, header }))} label={label} minWidth={640} rows={rows} />;
}

function DataTable({ headers, rows }: { headers: string[]; rows: string[][] }) {
    const { t, locale } = useTranslation();
    if (rows.length === 0) return <RouteState kind="empty" title={t("No matching items")} />;
    return <DesktopList columns={headers.map((header, index) => ({ key: `${index}-${header}`, header }))} label={t("Repository packages")} minWidth={480} rows={rows.map((cells, rowIndex) => ({ key: `${rowIndex}-${cells[0]}`, cells }))} />;
}

function sourceText(t: Translate, locale: SupportedLocale, repository: RepositorySnapshot): string { return repository.source.kind === "local" ? t("Local repository") : repository.source.url; }
function formatTime(t: Translate, locale: SupportedLocale, value: number): string { return new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeStyle: "short", timeZone: "UTC" }).format(new Date(value)) + " UTC"; }
function formatBytes(t: Translate, locale: SupportedLocale, value: number): string { return value < 1024 * 1024 ? t("{value0} KiB", { value0: Math.ceil(value / 1024) }) : t("{value0} MiB", { value0: (value / (1024 * 1024)).toFixed(1) }); }
function shortHash(value: string): string { return value.length <= 24 ? value : `${value.slice(0, 16)}…${value.slice(-8)}`; }
function humanize(t: Translate, value: string): string {
    const labels: Readonly<Record<string, string>> = {
        "projects.create": "Create project",
        "projects.delete-directory": "Delete project directory",
        "extensions.permission.change": "Change extension permissions",
        "extensions.disable": "Disable extension",
        "extensions.enable": "Enable extension",
        "user-package.remove": "Remove user package registration",
        "user-package.refresh": "Read user package content",
        "user-package.add": "Add user package",
        "projectTemplates.remove": "Remove template",
        "projectTemplates.save": "Save template",
        "projectTemplates.export": "Export template",
        "unity.launch": "Request Unity launch",
        "unity.launch-config.change": "Change Unity launch arguments",
        "unity.installation.remove": "Remove Unity installation",
        "unity.installation.add": "Add Unity installation",
        "repository.remove": "Remove repository",
        "repository.fetch": "Fetch repository",
        "repository.add": "Add repository",
        "project.remove": "Remove project registration",
        "project.favorite": "Change project favorite",
        "project.refresh": "Read project content",
        "project.add": "Add project",
        "packages.bulk": "Change packages",
        "packages.reinstall": "Reinstall packages",
        "packages.resolve": "Resolve packages",
        "packages.downgrade": "Downgrade packages",
        "packages.upgrade": "Upgrade packages",
        "packages.remove": "Remove packages",
        "packages.install": "Install packages",
        "queued": "queued",
        "running": "running",
        "recovering": "recovering",
        "cancelling": "cancelling",
        "interrupted": "interrupted",
        "cancelled": "cancelled",
        "succeeded": "succeeded",
        "failed": "failed",
        "completed": "completed",
        "opening": "opening",
        "open": "open",
        "confirmed": "confirmed",
        "suspected": "suspected",
        "not_observed": "not observed",
        "unknown": "unknown",
        "official": "official",
        "user_approved_for_extension": "user approved for extension",
        "installed_disabled": "installed disabled",
        "enabled": "enabled",
        "uninstalling": "uninstalling",
        "clear": "clear",
        "quarantined": "quarantined",
        "stopped": "stopped",
        "starting": "starting",
        "stopping": "stopping",
        "crashed": "crashed",
        "ready": "ready",
        "warning": "warning",
        "error": "error",
        "operation": "operation",
        "event": "event",
        "project": "project",
        "package": "package",
        "repository": "repository",
        "extension": "extension",
        "builtin": "builtin",
        "local": "local",
        "remote": "remote",
        "packages.apply": "Change packages",
        "projects.copy": "Copy project",
        "projects.delete": "projects delete",
        "projects.unity-migration": "Change project Unity version",
        "backups.create": "Create backup",
        "backups.restore": "Restore backup",
        "state.check": "Check state",
        "extensions.install": "Install extension",
        "extensions.uninstall": "Uninstall extension"
    };
    const label = labels[value];
    const english = value.replaceAll("_", " ").replaceAll(".", " ");
    if (label !== undefined) return t(label);
    if (value.startsWith("operation.")) {
        const end = value.lastIndexOf(".");
        const kind = value.slice("operation.".length, end);
        const state = value.slice(end + 1);
        if (labels[kind] !== undefined && labels[state] !== undefined) return t("operation {kind} {state}", { kind: humanize(t, kind), state: humanize(t, state) });
    }
    return english;
}

function safeError(_t: Translate, _locale: SupportedLocale, caught: unknown): RpcError {
    return normalizeRequestError(caught);
}
