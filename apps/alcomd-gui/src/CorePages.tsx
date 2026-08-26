import type { ExtensionRecord, RpcError } from "@alcomd/sdk";
import { useCallback, useEffect, useState, type FormEvent, type ReactNode } from "react";

import type {
    ActivityItem,
    BackupRecord,
    DiagnosticItem,
    OfficialSettings,
    Operation,
    ProjectSnapshot,
    RepositoryPackageVersion,
    RepositorySnapshot,
    TemplateRecord,
    UnityInstallation,
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
    ProjectActions,
    ProjectUnityActions,
    RegisterProjectPanel,
    RegisterRepositoryPanel,
    RepositoryActions,
    TemplateActions,
    TemplateImportPanel,
    UnityRegistryActions
} from "./CoreActions";
import type { GuiRpcClient } from "./rpc";
import { Button } from "./Material";

interface PageProps {
    client: GuiRpcClient;
    navigate(path: string): void;
}

interface ResourcePageProps<T> {
    load(): Promise<T>;
    children(value: T, refresh: () => void, refreshing: boolean): ReactNode;
    empty?(value: T): boolean;
    emptyTitle?: string;
}

function ResourcePage<T>({ load, children, empty, emptyTitle = "Nothing here yet" }: ResourcePageProps<T>) {
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
                <RefreshBar error={state.error} refresh={refresh} refreshing={state.refreshing} />
                <RouteState kind="empty" title={emptyTitle} />
            </>
        );
    }
    return (
        <>
            <RefreshBar error={state.error} refresh={refresh} refreshing={state.refreshing} />
            {children(state.value, refresh, state.refreshing)}
        </>
    );
}

function RefreshBar({ error, refresh, refreshing }: { error?: RpcError; refresh(): void; refreshing: boolean }) {
    return (
        <div className="refresh-bar" role="status" aria-live="polite">
            <span>{refreshing ? "Refreshing while keeping the last result…" : "Current daemon state"}</span>
            {error === undefined ? null : <span className="inline-error">Refresh failed: {error.code}</span>}
            <button className="button button--tonal" disabled={refreshing} onClick={refresh} type="button">
                {refreshing ? "Refreshing…" : "Refresh"}
            </button>
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
            <button className="button button--filled" onClick={retry} type="button">Reconnect and retry</button>
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
                    <article className="summary-card"><h2>Get started</h2><button className="text-link" onClick={() => navigate("/projects")} type="button">Open projects</button></article>
                </div>
            )}</ResourcePage>
        </Page>
    );
}

export function ProjectsPage({ client, navigate }: PageProps) {
    const load = useCallback(() => client.projectsList(), [client]);
    return (
        <Page title="Projects" eyebrow="Registered Unity projects">
            <ResourcePage load={load}>
                {(value, refresh) => <>{value.projects.length === 0 ? <RouteState kind="empty" title="No registered projects" /> : <CardList>{value.projects.map((project) => <ProjectCard key={project.projectId ?? project.rootPath} project={project} navigate={navigate} />)}</CardList>}<RegisterProjectPanel client={client} onChanged={refresh} /></>}
            </ResourcePage>
        </Page>
    );
}

function ProjectCard({ project, navigate }: { project: ProjectSnapshot; navigate(path: string): void }) {
    const id = project.projectId;
    return (
        <article className="resource-card">
            <h2>{project.rootPath.split(/[\\/]/).at(-1) ?? "Unity project"}</h2>
            <p>{project.projectType} · Unity {project.unityVersion || "unknown"}</p>
            <p>{project.directDependencies.length} direct packages · revision {project.revision ?? "unregistered"}</p>
            {id === undefined ? null : <Button onClick={() => navigate(`/projects/${id}`)} variant="text">View project</Button>}
        </article>
    );
}

export function ProjectDetailPage({ client, navigate, projectId }: PageProps & { projectId: string }) {
    const load = useCallback(() => client.projectGet(projectId), [client, projectId]);
    return (
        <Page title="Project detail" eyebrow={projectId}>
            <ResourcePage load={load}>{({ project }, refresh) => (
                <>
                    <dl className="detail-grid">
                        <Detail label="Type" value={project.projectType} /><Detail label="Unity" value={project.unityVersion || "Unknown"} />
                        <Detail label="Revision" value={String(project.revision ?? "—")} /><Detail label="VPM manifest" value={project.vpmManifest} />
                    </dl>
                    <nav className="subnav" aria-label="Project sections">
                        <button onClick={() => navigate(`/projects/${projectId}/packages`)} type="button">Packages</button>
                        <button onClick={() => navigate(`/projects/${projectId}/unity`)} type="button">Unity</button>
                        <button onClick={() => navigate(`/projects/${projectId}/backups`)} type="button">Backups</button>
                    </nav>
                    <h2>Direct packages</h2>
                    <DataTable headers={["Package", "Requested"]} rows={project.directDependencies.map((item) => [item.packageId, item.value])} />
                    <ProjectActions client={client} onChanged={refresh} project={project} />
                </>
            )}</ResourcePage>
        </Page>
    );
}

export function ProjectPackagesPage({ client, projectId }: PageProps & { projectId: string }) {
    const load = useCallback(async () => {
        const [project, repositories] = await Promise.all([client.projectGet(projectId), client.repositoriesList()]);
        const repository = repositories.repositories[0];
        const catalog = repository?.repositoryId === undefined
            ? { packages: [] as RepositoryPackageVersion[] }
            : await client.repositoryPackages(repository.repositoryId);
        return { project: project.project, catalog: catalog.packages };
    }, [client, projectId]);
    return (
        <Page title="Packages" eyebrow={`Project ${projectId}`}>
            <ResourcePage load={load}>{({ project, catalog }, refresh) => (
                <><div className="two-column">
                    <section><h2>Project state</h2><DataTable headers={["Package", "Locked version"]} rows={project.lockedDependencies.map((item) => [item.packageId, item.value])} /></section>
                    <section><h2>Repository catalog</h2><DataTable headers={["Package", "Version", "Unity"]} rows={catalog.map((item) => [item.displayName ?? item.packageId, item.version, item.unity ?? "Any"])} /></section>
                </div><PackageActions client={client} onChanged={refresh} project={project} /></>
            )}</ResourcePage>
        </Page>
    );
}

export function RepositoriesPage({ client, navigate }: PageProps) {
    const load = useCallback(() => client.repositoriesList(), [client]);
    return <Page title="Repositories" eyebrow="VPM sources"><ResourcePage load={load}>{(value, refresh) => <>{value.repositories.length === 0 ? <RouteState kind="empty" title="No repositories registered" /> : <CardList>{value.repositories.map((repository) => <RepositoryCard key={repository.repositoryId ?? sourceText(repository)} repository={repository} navigate={navigate} />)}</CardList>}<RegisterRepositoryPanel client={client} onChanged={refresh} /></>}</ResourcePage></Page>;
}

function RepositoryCard({ repository, navigate }: { repository: RepositorySnapshot; navigate(path: string): void }) {
    return <article className="resource-card"><h2>{repository.name ?? repository.declaredId ?? "Repository"}</h2><p>{sourceText(repository)}</p><p>Revision {repository.revision ?? "unregistered"}</p>{repository.repositoryId === undefined ? null : <button className="text-link" onClick={() => navigate(`/repositories/${repository.repositoryId}`)} type="button">Browse packages</button>}</article>;
}

export function RepositoryDetailPage({ client, repositoryId }: PageProps & { repositoryId: string }) {
    const load = useCallback(async () => Promise.all([client.repositoryGet(repositoryId), client.repositoryPackages(repositoryId)]), [client, repositoryId]);
    return <Page title="Repository" eyebrow={repositoryId}><ResourcePage load={load}>{([detail, catalog], refresh) => <><dl className="detail-grid"><Detail label="Name" value={detail.repository.name ?? "Unnamed"} /><Detail label="Source" value={sourceText(detail.repository)} /><Detail label="Revision" value={String(detail.repository.revision ?? "—")} /><Detail label="Issues" value={String(detail.repository.issues.length)} /></dl><h2>Packages</h2><DataTable headers={["Package", "Version", "Status"]} rows={catalog.packages.map((item) => [item.displayName ?? item.packageId, item.version, item.yanked ? "Yanked" : "Available"])} /><RepositoryActions client={client} onChanged={refresh} repository={detail.repository} /></>}</ResourcePage></Page>;
}

export function TemplatesPage({ client, navigate }: PageProps) {
    const load = useCallback(() => client.templatesList(), [client]);
    return <Page title="Templates" eyebrow="Project starters"><ResourcePage load={load}>{(value, refresh) => <>{value.templates.length === 0 ? <RouteState kind="empty" title="No templates available" /> : <CardList>{value.templates.map((template) => <TemplateCard key={template.templateId} template={template} navigate={navigate} />)}</CardList>}<TemplateImportPanel client={client} onChanged={refresh} /></>}</ResourcePage></Page>;
}

function TemplateCard({ template, navigate }: { template: TemplateRecord; navigate(path: string): void }) {
    return <article className="resource-card"><h2>{template.displayName}</h2><p>{template.description ?? "No description"}</p><p>{template.sourceKind} · v{template.templateVersion}{template.favorite ? " · Favorite" : ""}</p><button className="text-link" onClick={() => navigate(`/templates/${template.templateId}`)} type="button">View template</button></article>;
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
        const [project, installations, ...results] = await Promise.all([client.projectGet(projectId), client.unityInstallationsList(), Promise.allSettled([client.unityProjectEditorGet(projectId), client.unityWriterState(projectId)])]);
        return { project: project.project, installations: installations.installations, preference: results[0][0].status === "fulfilled" ? results[0][0].value.preference : undefined, writer: results[0][1].status === "fulfilled" ? results[0][1].value : undefined };
    }, [client, projectId]);
    return <Page title="Project Unity" eyebrow={projectId}><ResourcePage load={load}>{({ project, installations, preference, writer }, refresh) => <><dl className="detail-grid"><Detail label="Selected editor" value={preference?.installationId ?? "Not selected"} /><Detail label="Writer observation" value={writer?.state ?? "Unknown"} /><Detail label="Arguments" value={preference?.arguments.length === 0 ? "Default" : `${preference?.arguments.length ?? 0} configured arguments`} /><Detail label="Observed" value={writer === undefined ? "—" : formatTime(writer.checkedAtMs)} /></dl><ProjectUnityActions client={client} installations={installations} onChanged={refresh} preferenceRevision={preference?.revision ?? 0} project={project} /></>}</ResourcePage></Page>;
}

export function ProjectBackupsPage({ client, navigate, projectId }: PageProps & { projectId: string }) {
    const load = useCallback(async () => ({ backups: await client.backupsList(projectId), project: (await client.projectGet(projectId)).project }), [client, projectId]);
    return <Page title="Backups" eyebrow={`Project ${projectId}`}><ResourcePage load={load}>{(value, refresh) => <>{value.backups.backups.length === 0 ? <RouteState kind="empty" title="No backups for this project" /> : <CardList>{value.backups.backups.map((backup) => <BackupCard backup={backup} key={backup.backupId} navigate={navigate} />)}</CardList>}<BackupCreatePanel client={client} onChanged={refresh} project={value.project} /></>}</ResourcePage></Page>;
}

function BackupCard({ backup, navigate }: { backup: BackupRecord; navigate(path: string): void }) {
    return <article className="resource-card"><h2>{formatTime(backup.createdAtMs)}</h2><p>{formatBytes(backup.archiveBytes)} · {backup.compressionMode}</p><p>{backup.excludeVpmPackages ? "VPM packages excluded" : "VPM packages included"}</p><button className="text-link" onClick={() => navigate(`/backups/${backup.backupId}`)} type="button">View backup</button></article>;
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
    return <article className="resource-card"><h2>{humanize(operation.kind)}</h2><p className={`state-chip state-chip--${operation.state}`}>{humanize(operation.state)}</p><p>{operation.progress?.phase === undefined ? "No phase reported" : humanize(operation.progress.phase)}</p><button className="text-link" onClick={() => navigate(`/operations/${operation.operationId}`)} type="button">View operation</button></article>;
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
    return <article className="resource-card"><h2>{extension.extensionId}</h2><p>v{extension.version} · {humanize(extension.trustDecision)}</p><p>{humanize(extension.desiredState)} · {humanize(extension.runtimeState)}</p><button className="text-link" onClick={() => navigate(`/extensions/${extension.extensionId}`)} type="button">Manage extension</button></article>;
}

export function ExtensionDetailPage({ client, navigate, extensionId }: PageProps & { extensionId: string }) {
    const load = useCallback(() => client.extensionGet(extensionId), [client, extensionId]);
    return <Page title="Extension detail" eyebrow={extensionId}><ResourcePage load={load}>{({ extension }, refresh) => <><dl className="detail-grid"><Detail label="Version" value={extension.version} /><Detail label="Publisher" value={shortHash(extension.publisherFingerprint)} /><Detail label="Trust" value={humanize(extension.trustDecision)} /><Detail label="Desired state" value={humanize(extension.desiredState)} /><Detail label="Runtime" value={humanize(extension.runtimeState)} /><Detail label="Quarantine" value={humanize(extension.quarantineState)} /><Detail label="Grant revision" value={String(extension.grantRevision)} /><Detail label="Record revision" value={String(extension.revision)} /></dl>{extension.ui?.protocol === "portable-v1" ? <button className="button button--filled" onClick={() => navigate(`/extensions/${extensionId}/ui`)} type="button">Open Portable UI</button> : <RouteState kind="empty" title="This extension has no Portable UI" />}<ExtensionActions client={client} extension={extension} onChanged={refresh} /></>}</ResourcePage></Page>;
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
            {item.operationId === undefined ? null : <button className="text-link" onClick={() => navigate(`/operations/${item.operationId}`)} type="button">View operation</button>}
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
                <button className="button button--filled" disabled={checking} onClick={() => void runStateCheck()} type="button">{checking ? "Starting…" : "Run state check"}</button>
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
    const dirty = JSON.stringify(settings) !== JSON.stringify(snapshot.settings);

    useEffect(() => {
        onDirtyChange(dirty);
        return () => onDirtyChange(false);
    }, [dirty, onDirtyChange]);

    const updateAppearance = <Key extends keyof OfficialSettings["appearance"]>(
        key: Key,
        value: OfficialSettings["appearance"][Key]
    ) => setSettings((current) => ({
        ...current,
        appearance: { ...current.appearance, [key]: value }
    }));

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
            <label htmlFor="settings-theme">Theme</label>
            <select id="settings-theme" value={settings.appearance.mode} onChange={(event) => updateAppearance("mode", event.currentTarget.value as OfficialSettings["appearance"]["mode"])}>
                <option value="system">System</option><option value="light">Light</option><option value="dark">Dark</option>
            </select>
            <label htmlFor="settings-color">Source color</label>
            <select aria-describedby="settings-color-hint" id="settings-color" value={settings.appearance.sourceColor ?? ""} onChange={(event) => updateAppearance("sourceColor", event.currentTarget.value || null)}>
                <option value="">Product default</option><option value="#6750A4">Violet</option><option value="#315DA8">Blue</option><option value="#006A60">Teal</option>
            </select>
            <p className="field-hint" id="settings-color-hint">Saved as a canonical #RRGGBB value; extensions never receive this preference.</p>
            <label htmlFor="settings-density">Density</label>
            <select id="settings-density" value={settings.appearance.density} onChange={(event) => updateAppearance("density", event.currentTarget.value as OfficialSettings["appearance"]["density"])}>
                <option value="default">Default</option><option value="compact">Compact</option>
            </select>
            <label htmlFor="settings-motion">Motion</label>
            <select id="settings-motion" value={settings.appearance.motion} onChange={(event) => updateAppearance("motion", event.currentTarget.value as OfficialSettings["appearance"]["motion"])}>
                <option value="system">System preference</option><option value="reduced">Reduce motion</option>
            </select>
            <label htmlFor="settings-locale">Language</label>
            <select id="settings-locale" value={settings.locale} onChange={(event) => {
                const locale = event.currentTarget.value as SettingsLocale;
                setSettings((current) => ({ ...current, locale }));
            }}>
                <option value="system">System language</option><option value="en-US">English</option><option value="zh-CN">简体中文</option><option value="ja-JP">日本語</option>
            </select>
            {error === undefined ? null : <p className="form-error" role="alert">{error.code === "revision_conflict" ? "Settings changed elsewhere. Reload and try again." : "Settings could not be saved."}</p>}
            <div className="action-row">
                <button className="button button--filled" disabled={!dirty || busy} type="submit">{busy ? "Saving…" : "Save settings"}</button>
                <button className="button button--tonal" disabled={!dirty || busy} onClick={() => setSettings(snapshot.settings)} type="button">Discard changes</button>
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
