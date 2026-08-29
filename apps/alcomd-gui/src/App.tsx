import type {
    ExtensionRecord,
    RpcError,
    UiAction,
    UiSnapshot
} from "@alcomd/sdk";
import {
    applyAppearance,
    defaultAppearance,
    productFamily,
    type AppearanceSettings
} from "@alcomd/ui";
import {
    extensionIcon,
    infoIcon,
    logIcon,
    packagesIcon,
    projectsIcon,
    settingsIcon,
    taskCenterIcon,
    type IconAsset
} from "@alcomd/ui/icons";
import type { ReactNode } from "react";
import { useCallback, useEffect, useRef, useState } from "react";

import {
    AboutPage,
    ActivityPage,
    BackupDetailPage,
    DiagnosticsPage,
    ExtensionDetailPage,
    ExtensionsPage,
    OperationDetailPage,
    OperationsPage,
    ProjectBackupsPage,
    ProjectDetailPage,
    ProjectPackagesPage,
    ProjectsPage,
    ProjectUnityPage,
    RepositoriesPage,
    RepositoryDetailPage,
    RouteState,
    SettingsPage,
    TemplateDetailPage,
    TemplatesPage,
    UnityPage,
    UserPackagesPage
} from "./CorePages";
import { PortableUiRenderer } from "./PortableUiRenderer";
import {
    PortableUiConsumerError,
    acceptSnapshot
} from "./portable-ui";
import { Button, Dialog as MaterialDialog, Icon, NavigationList, NavigationListItem } from "./Material";
import { guiRpcClient, type GuiRpcClient } from "./rpc";
import type { SettingsGetResult } from "./core-models";

const DISCARD_MESSAGE = "Discard the unsaved changes?";

type Route =
    | { kind: "projects" }
    | { kind: "project-detail"; projectId: string }
    | { kind: "project-packages"; projectId: string }
    | { kind: "project-unity"; projectId: string }
    | { kind: "project-backups"; projectId: string }
    | { kind: "repositories" }
    | { kind: "repository-detail"; repositoryId: string }
    | { kind: "templates" }
    | { kind: "user-packages" }
    | { kind: "template-detail"; templateId: string }
    | { kind: "unity" }
    | { kind: "backup-detail"; backupId: string }
    | { kind: "operations" }
    | { kind: "operation-detail"; operationId: string }
    | { kind: "extensions" }
    | { kind: "extension-detail"; extensionId: string }
    | { kind: "extension-ui"; extensionId: string }
    | { kind: "activity" }
    | { kind: "diagnostics" }
    | { kind: "settings" }
    | { kind: "about" }
    | { kind: "not-found" };

interface AppProps {
    client?: GuiRpcClient;
}

export function App({ client = guiRpcClient }: AppProps) {
    const [route, setRoute] = useState<Route>(() => readRoute(window.location.pathname));
    const [pendingRoute, setPendingRoute] = useState<Route>();
    const [appearance, setAppearance] = useState<AppearanceSettings>(defaultAppearance);
    const [locale, setLocale] = useState(() => preferredLocale(navigator.language));
    const dirtyRef = useRef(false);
    const handleDirtyChange = useCallback((dirty: boolean) => {
        dirtyRef.current = dirty;
    }, []);

    useEffect(() => {
        applyAppearance(document.documentElement, appearance);
    }, [appearance]);

    const applySettings = useCallback((value: SettingsGetResult) => {
        const sourceColor = sourceColorName(value.settings.appearance.sourceColor);
        setAppearance({
            mode: value.settings.appearance.mode,
            sourceColor,
            density: value.settings.appearance.density === "compact" ? "compact" : "comfortable"
        });
        const nextLocale = value.settings.locale === "system"
            ? preferredLocale(navigator.language)
            : value.settings.locale;
        setLocale(nextLocale);
        document.documentElement.lang = nextLocale;
        document.documentElement.dataset.motion = value.settings.appearance.motion;
    }, []);

    useEffect(() => {
        let active = true;
        void client.settingsGet().then((value) => {
            if (active) applySettings(value);
        }).catch(() => {
            // The durable Settings page exposes reconnect/error state; shell defaults remain safe.
        });
        return () => { active = false; };
    }, [applySettings, client]);

    useEffect(() => {
        const onPopState = () => {
            const next = readRoute(window.location.pathname);
            if (dirtyRef.current) {
                window.history.pushState(null, "", routePath(route));
                setPendingRoute(next);
                return;
            }
            dirtyRef.current = false;
            setRoute(next);
        };
        window.addEventListener("popstate", onPopState);
        return () => window.removeEventListener("popstate", onPopState);
    }, [route]);

    useEffect(() => {
        const beforeUnload = (event: BeforeUnloadEvent) => {
            if (dirtyRef.current) {
                event.preventDefault();
            }
        };
        window.addEventListener("beforeunload", beforeUnload);
        return () => window.removeEventListener("beforeunload", beforeUnload);
    }, []);

    useEffect(() => {
        window.requestAnimationFrame(() => {
            document.querySelector<HTMLElement>("#route-title, #extension-ui-title")?.focus();
        });
    }, [route]);

    const navigateRoute = (next: Route) => {
        if (dirtyRef.current) {
            setPendingRoute(next);
            return;
        }
        dirtyRef.current = false;
        window.history.pushState(null, "", routePath(next));
        setRoute(next);
    };

    const discardAndNavigate = () => {
        if (pendingRoute === undefined) return;
        dirtyRef.current = false;
        window.history.pushState(null, "", routePath(pendingRoute));
        setRoute(pendingRoute);
        setPendingRoute(undefined);
    };

    const navigate = (path: string) => navigateRoute(readRoute(path));

    return (
        <div className="app-shell">
            <div className="app-body">
                <PrimaryNavigation
                    client={client}
                    current={route}
                    navigate={navigate}
                />
                <main className="main-content" id="main-content">
                    <RouteContent
                        client={client}
                        locale={locale}
                        navigate={navigate}
                        onDirtyChange={handleDirtyChange}
                        onSettingsApplied={applySettings}
                        route={route}
                    />
                </main>
            </div>
            <MaterialDialog onClose={() => setPendingRoute(undefined)} open={pendingRoute !== undefined} title="Discard unsaved changes?">
                <p>{DISCARD_MESSAGE}</p>
                <div className="dialog-actions">
                    <Button onClick={() => setPendingRoute(undefined)} type="button" variant="text">Keep editing</Button>
                    <Button onClick={discardAndNavigate} type="button">Discard changes</Button>
                </div>
            </MaterialDialog>
        </div>
    );
}

function PrimaryNavigation({ client, current, navigate }: { client: GuiRpcClient; current: Route; navigate(path: string): void }) {
    const items: readonly NavigationItem[] = [
        { icon: projectsIcon, label: "Projects", path: "/projects", section: "projects" },
        { icon: packagesIcon, label: "Resources", path: "/repositories", section: "packages" },
        { icon: settingsIcon, label: "Settings", path: "/settings", section: "settings" },
        { icon: logIcon, label: "Logs", path: "/activity", section: "log" }
    ];
    return (
        <aside className="primary-navigation" id="primary-navigation">
            <a className="skip-link" href="#main-content">Skip to content</a>
            <nav aria-label="Primary">
                <NavigationList className="navigation-list">
                    {items.map((item) => {
                        const selected = routeSection(current) === item.section;
                        return (
                            <NavigationListItem
                                aria-current={selected ? "page" : undefined}
                                className="navigation-item"
                                key={item.path}
                                onClick={() => navigate(item.path)}
                                selected={selected}
                            >
                                <Icon asset={item.icon} size={24} slot="start" />
                                <span className="navigation-item-label" slot="headline">{item.label}</span>
                            </NavigationListItem>
                        );
                    })}
                </NavigationList>
            </nav>
            <div className="navigation-spacer" />
            <NavigationFooter client={client} current={current} navigate={navigate} />
        </aside>
    );
}

interface NavigationItem {
    readonly icon: IconAsset;
    readonly label: string;
    readonly path: string;
    readonly section: "projects" | "packages" | "settings" | "log";
}

function NavigationFooter({ client, current, navigate }: { client: GuiRpcClient; current: Route; navigate(path: string): void }) {
    const [summary, setSummary] = useState<{ activeTasks: number; daemonVersion?: string }>({ activeTasks: 0 });
    useEffect(() => {
        let active = true;
        void Promise.allSettled([client.operationsList(), client.systemStatus()]).then(([operations, status]) => {
            if (!active) return;
            setSummary({
                activeTasks: operations.status === "fulfilled"
                    ? operations.value.operations.filter((operation) => !["succeeded", "failed", "cancelled"].includes(operation.state)).length
                    : 0,
                ...(status.status === "fulfilled" ? { daemonVersion: status.value.daemonVersion } : {})
            });
        });
        return () => { active = false; };
    }, [client, current.kind]);
    return (
        <footer className="navigation-footer">
            <nav aria-label="Utilities">
                <NavigationList className="navigation-list navigation-list--footer">
                    <NavigationListItem
                        aria-current={routeSection(current) === "extensions" ? "page" : undefined}
                        className="navigation-item"
                        onClick={() => navigate("/extensions")}
                        selected={routeSection(current) === "extensions"}
                    >
                        <Icon asset={extensionIcon} size={24} slot="start" />
                        <span className="navigation-item-label" slot="headline">Extensions</span>
                    </NavigationListItem>
                    <NavigationListItem
                        aria-label={summary.activeTasks === 0 ? "Task Center" : `Task Center ${summary.activeTasks} active`}
                        aria-current={routeSection(current) === "tasks" ? "page" : undefined}
                        className="navigation-item"
                        onClick={() => navigate("/operations")}
                        selected={routeSection(current) === "tasks"}
                    >
                        <Icon asset={taskCenterIcon} size={24} slot="start" />
                        <span className="navigation-item-label" slot="headline">Task Center</span>
                        {summary.activeTasks === 0 ? null : <span aria-hidden="true" className="navigation-item-meta" slot="end">{summary.activeTasks}</span>}
                    </NavigationListItem>
                    <NavigationListItem
                        aria-label={summary.daemonVersion === undefined ? "About" : `About ${summary.daemonVersion}`}
                        aria-current={routeSection(current) === "about" ? "page" : undefined}
                        className="navigation-item"
                        onClick={() => navigate("/about")}
                        selected={routeSection(current) === "about"}
                    >
                        <Icon asset={infoIcon} size={24} slot="start" />
                    <span className="navigation-item-label" slot="headline">About</span>
                    {summary.daemonVersion === undefined ? null : <span aria-hidden="true" className="navigation-item-meta" slot="end">{summary.daemonVersion}</span>}
                    </NavigationListItem>
                </NavigationList>
            </nav>
        </footer>
    );
}

function RouteContent({
    client,
    locale,
    navigate,
    onDirtyChange,
    onSettingsApplied,
    route
}: {
    client: GuiRpcClient;
    locale: string;
    navigate(path: string): void;
    onDirtyChange(dirty: boolean): void;
    onSettingsApplied(value: SettingsGetResult): void;
    route: Route;
}) {
    const props = { client, navigate };
    switch (route.kind) {
        case "projects": return <ProjectsPage {...props} />;
        case "project-detail": return <ProjectDetailPage {...props} projectId={route.projectId} />;
        case "project-packages": return <ProjectPackagesPage {...props} projectId={route.projectId} />;
        case "project-unity": return <ProjectUnityPage {...props} projectId={route.projectId} />;
        case "project-backups": return <ProjectBackupsPage {...props} projectId={route.projectId} />;
        case "repositories": return <RepositoriesPage {...props} />;
        case "repository-detail": return <RepositoryDetailPage {...props} repositoryId={route.repositoryId} />;
        case "templates": return <TemplatesPage {...props} />;
        case "user-packages": return <UserPackagesPage {...props} />;
        case "template-detail": return <TemplateDetailPage {...props} templateId={route.templateId} />;
        case "unity": return <UnityPage {...props} />;
        case "backup-detail": return <BackupDetailPage {...props} backupId={route.backupId} />;
        case "operations": return <OperationsPage {...props} />;
        case "operation-detail": return <OperationDetailPage {...props} operationId={route.operationId} />;
        case "extensions": return <ExtensionsPage {...props} />;
        case "extension-detail": return <ExtensionDetailPage {...props} extensionId={route.extensionId} />;
        case "extension-ui": return <ExtensionUiPage client={client} extensionId={route.extensionId} locale={locale} onDirtyChange={onDirtyChange} />;
        case "activity": return <ActivityPage {...props} />;
        case "diagnostics": return <DiagnosticsPage {...props} />;
        case "settings": return <SettingsPage {...props} onApplied={onSettingsApplied} onDirtyChange={onDirtyChange} />;
        case "about": return <AboutPage {...props} />;
        case "not-found": return <RouteState kind="error" title="Page not found" detail="Use the primary navigation to continue." />;
    }
}

interface ExtensionUiPageProps {
    client: GuiRpcClient;
    extensionId: string;
    locale: string;
    onDirtyChange(dirty: boolean): void;
}

function ExtensionUiPage({ client, extensionId, locale, onDirtyChange }: ExtensionUiPageProps) {
    const [extension, setExtension] = useState<ExtensionRecord>();
    const [snapshot, setSnapshot] = useState<UiSnapshot>();
    const [error, setError] = useState<RpcError>();
    const [pendingDiscardAction, setPendingDiscardAction] = useState<"reconnect" | "refresh">();
    const [busy, setBusy] = useState(false);
    const [dirty, setDirty] = useState(false);
    const [generation, setGeneration] = useState(0);
    const sessionIdRef = useRef<string | undefined>(undefined);
    const sequenceRef = useRef(1);
    const headingRef = useRef<HTMLHeadingElement>(null);

    const updateDirty = useCallback((next: boolean) => {
        setDirty(next);
        onDirtyChange(next);
    }, [onDirtyChange]);

    useEffect(() => {
        let active = true;
        let openedSessionId: string | undefined;
        setExtension(undefined);
        setSnapshot(undefined);
        setError(undefined);
        setBusy(true);
        updateDirty(false);
        sequenceRef.current = 1;

        const open = async () => {
            try {
                const extensionResult = await client.extensionGet(extensionId);
                if (extensionResult.extension.ui?.protocol !== "portable-v1") {
                    throw rpcError("extension_ui_not_available", "This extension does not expose Portable UI v1.");
                }
                const opened = await client.extensionUiOpen({ extensionId, locale });
                openedSessionId = opened.session.sessionId;
                if (opened.session.extensionId !== extensionId
                    || opened.session.locale !== locale
                    || opened.snapshot.sessionId !== opened.session.sessionId) {
                    throw rpcError("extension_ui_document_invalid", "The extension UI session was invalid.");
                }
                const accepted = acceptSnapshot(undefined, opened.snapshot, false);
                if (!active) {
                    await closeBestEffort(client, opened.session.sessionId);
                    return;
                }
                sessionIdRef.current = opened.session.sessionId;
                let currentExtension = extensionResult.extension;
                try {
                    currentExtension = (await client.extensionGet(extensionId)).extension;
                } catch {
                    // Identity remains the verified record read immediately before opening.
                }
                setExtension(currentExtension);
                setSnapshot(accepted);
                setBusy(false);
                window.requestAnimationFrame(() => headingRef.current?.focus());
            } catch (caught: unknown) {
                if (active) {
                    if (openedSessionId !== undefined) {
                        await closeBestEffort(client, openedSessionId);
                        openedSessionId = undefined;
                    }
                    setBusy(false);
                    setError(safeError(caught));
                }
            }
        };
        void open();

        return () => {
            active = false;
            updateDirty(false);
            if (sessionIdRef.current === openedSessionId) {
                sessionIdRef.current = undefined;
            }
            if (openedSessionId !== undefined) {
                void closeBestEffort(client, openedSessionId);
            }
        };
    }, [client, extensionId, generation, locale, updateDirty]);

    const performRefresh = async () => {
        if (snapshot === undefined || sessionIdRef.current === undefined || busy) {
            return;
        }
        setBusy(true);
        setError(undefined);
        try {
            const result = await client.extensionUiRefresh({
                sessionId: sessionIdRef.current,
                expectedSnapshotRevision: snapshot.snapshotRevision
            });
            setSnapshot(acceptSnapshot(snapshot, result.snapshot, false));
            updateDirty(false);
        } catch (caught: unknown) {
            handleSessionError(caught, setError, setSnapshot, updateDirty);
        } finally {
            setBusy(false);
        }
    };

    const refresh = async () => {
        if (dirty) {
            setPendingDiscardAction("refresh");
            return;
        }
        await performRefresh();
    };

    const dispatch = async (action: UiAction) => {
        if (snapshot === undefined || sessionIdRef.current === undefined || busy) {
            return;
        }
        const sequence = sequenceRef.current;
        setBusy(true);
        setError(undefined);
        try {
            const result = await client.extensionUiDispatch({
                sessionId: sessionIdRef.current,
                expectedSnapshotRevision: snapshot.snapshotRevision,
                sequence,
                requestId: crypto.randomUUID(),
                action
            });
            setSnapshot(acceptSnapshot(snapshot, result.snapshot, result.replayed));
            sequenceRef.current = sequence + 1;
            updateDirty(false);
        } catch (caught: unknown) {
            handleSessionError(caught, setError, setSnapshot, updateDirty);
        } finally {
            setBusy(false);
        }
    };

    const reconnect = () => {
        if (dirty) {
            setPendingDiscardAction("reconnect");
            return;
        }
        updateDirty(false);
        setGeneration((current) => current + 1);
    };

    const discardAndContinue = async () => {
        const action = pendingDiscardAction;
        setPendingDiscardAction(undefined);
        updateDirty(false);
        if (action === "refresh") {
            await performRefresh();
        } else if (action === "reconnect") {
            setGeneration((current) => current + 1);
        }
    };

    return (
        <section className="extension-surface" aria-labelledby="extension-ui-title">
            <header className="extension-chrome">
                <div>
                    <p className="eyebrow">Extension-provided content</p>
                    <h1 id="extension-ui-title" ref={headingRef} tabIndex={-1}>Extension UI</h1>
                    <p className="extension-id"><code>{extensionId}</code></p>
                </div>
                <Button disabled={busy || snapshot === undefined} onClick={() => void refresh()} type="button" variant="tonal">
                    {busy && snapshot !== undefined ? "Working…" : "Refresh"}
                </Button>
            </header>
            {extension === undefined ? null : <ExtensionIdentity extension={extension} />}
            {busy && snapshot === undefined ? (
                <StatePanel kind="loading" title="Opening extension UI" detail="Connecting to the ALCOMD core and extension host…" />
            ) : null}
            {error === undefined ? null : (
                <StatePanel
                    action={<Button onClick={reconnect} type="button">Reconnect</Button>}
                    detail={error.diagnosticId === undefined ? undefined : `Diagnostic ID: ${error.diagnosticId}`}
                    kind={error.code === "daemon_unavailable" ? "disconnected" : "error"}
                    title={errorTitle(error.code)}
                />
            )}
            <MaterialDialog onClose={() => setPendingDiscardAction(undefined)} open={pendingDiscardAction !== undefined} title="Discard unsaved changes?">
                <p>{DISCARD_MESSAGE}</p>
                <div className="dialog-actions">
                    <Button onClick={() => setPendingDiscardAction(undefined)} type="button" variant="text">Keep editing</Button>
                    <Button onClick={() => void discardAndContinue()} type="button">Discard changes</Button>
                </div>
            </MaterialDialog>
            {snapshot === undefined ? null : (
                <PortableUiRenderer
                    busy={busy}
                    onAction={dispatch}
                    onDirtyChange={updateDirty}
                    snapshot={snapshot}
                />
            )}
        </section>
    );
}

function ExtensionIdentity({ extension }: { extension: ExtensionRecord }) {
    return (
        <dl className="extension-identity" aria-label="Host-verified extension identity">
            <div><dt>Version</dt><dd>{extension.version}</dd></div>
            <div><dt>Publisher</dt><dd><code>{shortFingerprint(extension.publisherFingerprint)}</code></dd></div>
            <div><dt>Trust</dt><dd>{humanize(extension.trustDecision)}</dd></div>
            <div><dt>Desired state</dt><dd>{humanize(extension.desiredState)}</dd></div>
            <div><dt>Runtime</dt><dd>{humanize(extension.runtimeState)}</dd></div>
            <div><dt>Quarantine</dt><dd>{humanize(extension.quarantineState)}</dd></div>
        </dl>
    );
}

interface StatePanelProps {
    kind: "loading" | "error" | "disconnected";
    title: string;
    detail?: string;
    action?: ReactNode;
}

function StatePanel({ kind, title, detail, action }: StatePanelProps) {
    return (
        <section className={`state-panel state-panel--${kind}`} aria-live="polite" role={kind === "error" ? "alert" : "status"}>
            <h2>{title}</h2>
            {detail === undefined ? null : <p>{detail}</p>}
            {action}
        </section>
    );
}

function handleSessionError(
    caught: unknown,
    setError: (error: RpcError) => void,
    setSnapshot: (snapshot: UiSnapshot | undefined) => void,
    updateDirty: (dirty: boolean) => void
): void {
    const error = safeError(caught);
    setError(error);
    if (error.code === "daemon_unavailable"
        || error.code === "extension_ui_session_not_found"
        || error.code === "extension_ui_session_stale"
        || error.code === "extension_ui_snapshot_stale") {
        setSnapshot(undefined);
        updateDirty(false);
    }
}

async function closeBestEffort(client: GuiRpcClient, sessionId: string): Promise<void> {
    try {
        await client.extensionUiClose({ sessionId });
    } catch {
        // Closing is best effort. Raw transport or UI payload details are intentionally discarded.
    }
}

function safeError(caught: unknown): RpcError {
    if (caught instanceof PortableUiConsumerError) {
        return rpcError(caught.code, "The extension UI response could not be displayed safely.");
    }
    if (typeof caught === "object" && caught !== null && "code" in caught && typeof caught.code === "string") {
        return {
            code: caught.code,
            message: "The request could not be completed.",
            ...("diagnosticId" in caught && typeof caught.diagnosticId === "string"
                ? { diagnosticId: caught.diagnosticId }
                : {})
        };
    }
    return rpcError("internal_error", "The request could not be completed.");
}

function rpcError(code: string, message: string): RpcError {
    return { code, message };
}

function errorTitle(code: string): string {
    switch (code) {
        case "daemon_unavailable": return "ALCOMD core disconnected";
        case "extension_not_enabled": return "Extension is disabled";
        case "extension_quarantined": return "Extension is quarantined";
        case "extension_ui_not_available": return "Portable UI is unavailable";
        case "extension_permission_denied": return "Extension UI access denied";
        case "extension_ui_session_stale":
        case "extension_ui_session_not_found":
        case "extension_ui_snapshot_stale": return "Extension UI session ended";
        default: return "Extension UI request failed";
    }
}

function shortFingerprint(value: string): string {
    return value.length <= 28 ? value : `${value.slice(0, 20)}…${value.slice(-8)}`;
}

function humanize(value: string): string {
    return value.replaceAll("_", " ");
}

function preferredLocale(value: string): string {
    const canonical = value.replace("_", "-");
    return /^(?:en-US|zh-CN|ja-JP)$/.test(canonical) ? canonical : "en-US";
}

function sourceColorName(value: string | null): AppearanceSettings["sourceColor"] {
    switch (value) {
        case "#315DA8": return "blue";
        case "#006A60": return "teal";
        default: return "violet";
    }
}

function readRoute(pathname: string): Route {
    const segments = pathname.split("/").filter(Boolean).map((segment) => decodeURIComponent(segment));
    if (segments.length === 0) return { kind: "projects" };
    if (segments.length === 1) {
        switch (segments[0]) {
            case "projects": return { kind: "projects" };
            case "repositories": return { kind: "repositories" };
            case "templates": return { kind: "templates" };
            case "user-packages": return { kind: "user-packages" };
            case "unity": return { kind: "unity" };
            case "operations": return { kind: "operations" };
            case "extensions": return { kind: "extensions" };
            case "activity": return { kind: "activity" };
            case "diagnostics": return { kind: "diagnostics" };
            case "settings": return { kind: "settings" };
            case "about": return { kind: "about" };
            default: return { kind: "not-found" };
        }
    }
    if (segments[0] === "projects" && segments[1] !== undefined) {
        if (segments.length === 2) return { kind: "project-detail", projectId: segments[1] };
        if (segments.length === 3 && segments[2] === "packages") return { kind: "project-packages", projectId: segments[1] };
        if (segments.length === 3 && segments[2] === "unity") return { kind: "project-unity", projectId: segments[1] };
        if (segments.length === 3 && segments[2] === "backups") return { kind: "project-backups", projectId: segments[1] };
    }
    if (segments[0] === "repositories" && segments.length === 2 && segments[1] !== undefined) return { kind: "repository-detail", repositoryId: segments[1] };
    if (segments[0] === "templates" && segments.length === 2 && segments[1] !== undefined) return { kind: "template-detail", templateId: segments[1] };
    if (segments[0] === "backups" && segments.length === 2 && segments[1] !== undefined) return { kind: "backup-detail", backupId: segments[1] };
    if (segments[0] === "operations" && segments.length === 2 && segments[1] !== undefined) return { kind: "operation-detail", operationId: segments[1] };
    if (segments[0] === "extensions" && segments[1] !== undefined) {
        if (segments.length === 2) return { kind: "extension-detail", extensionId: segments[1] };
        if (segments.length === 3 && segments[2] === "ui") return { kind: "extension-ui", extensionId: segments[1] };
    }
    return { kind: "not-found" };
}

function routePath(route: Route): string {
    switch (route.kind) {
        case "projects": return "/projects";
        case "project-detail": return `/projects/${encodeURIComponent(route.projectId)}`;
        case "project-packages": return `/projects/${encodeURIComponent(route.projectId)}/packages`;
        case "project-unity": return `/projects/${encodeURIComponent(route.projectId)}/unity`;
        case "project-backups": return `/projects/${encodeURIComponent(route.projectId)}/backups`;
        case "repositories": return "/repositories";
        case "repository-detail": return `/repositories/${encodeURIComponent(route.repositoryId)}`;
        case "templates": return "/templates";
        case "user-packages": return "/user-packages";
        case "template-detail": return `/templates/${encodeURIComponent(route.templateId)}`;
        case "unity": return "/unity";
        case "backup-detail": return `/backups/${encodeURIComponent(route.backupId)}`;
        case "operations": return "/operations";
        case "operation-detail": return `/operations/${encodeURIComponent(route.operationId)}`;
        case "extensions": return "/extensions";
        case "extension-detail": return `/extensions/${encodeURIComponent(route.extensionId)}`;
        case "extension-ui": return `/extensions/${encodeURIComponent(route.extensionId)}/ui`;
        case "activity": return "/activity";
        case "diagnostics": return "/diagnostics";
        case "settings": return "/settings";
        case "about": return "/about";
        case "not-found": return window.location.pathname;
    }
}

function routeSection(route: Route): string {
    switch (route.kind) {
        case "project-detail":
        case "project-packages":
        case "project-unity":
        case "project-backups": return "projects";
        case "repositories":
        case "repository-detail":
        case "templates":
        case "user-packages":
        case "template-detail": return "packages";
        case "backup-detail": return "projects";
        case "operations":
        case "operation-detail": return "tasks";
        case "extension-detail":
        case "extension-ui": return "extensions";
        case "activity":
        case "diagnostics": return "log";
        case "unity":
        case "settings": return "settings";
        case "about": return "about";
        default: return route.kind;
    }
}

function sectionLabel(section: string): string {
    switch (section) {
        case "projects": return "Projects";
        case "packages": return "Resources";
        case "extensions": return "Extensions";
        case "settings": return "Settings";
        case "log": return "Logs";
        case "tasks": return "Task Center";
        case "about": return "About";
        default: return productFamily;
    }
}
