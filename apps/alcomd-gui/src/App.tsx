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
    type AppearanceMode,
    type AppearanceSettings,
    type InterfaceDensity,
    type SourceColor
} from "@alcomd/ui";
import type { ReactNode } from "react";
import { useCallback, useEffect, useRef, useState } from "react";

import { PortableUiRenderer } from "./PortableUiRenderer";
import {
    PortableUiConsumerError,
    acceptSnapshot
} from "./portable-ui";
import { guiRpcClient, type GuiRpcClient } from "./rpc";

const DISCARD_MESSAGE = "Discard the unsaved extension form changes?";

type Route = { kind: "home" } | { kind: "extension-ui"; extensionId: string };

export function App() {
    const [route, setRoute] = useState<Route>(() => readRoute(window.location.pathname));
    const [appearance, setAppearance] = useState<AppearanceSettings>(defaultAppearance);
    const [locale, setLocale] = useState(() => preferredLocale(navigator.language));
    const dirtyRef = useRef(false);
    const handleDirtyChange = useCallback((dirty: boolean) => {
        dirtyRef.current = dirty;
    }, []);

    useEffect(() => {
        applyAppearance(document.documentElement, appearance);
    }, [appearance]);

    useEffect(() => {
        const onPopState = () => {
            const next = readRoute(window.location.pathname);
            if (dirtyRef.current && !window.confirm(DISCARD_MESSAGE)) {
                window.history.pushState(null, "", routePath(route));
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

    const navigate = (next: Route) => {
        if (dirtyRef.current && !window.confirm(DISCARD_MESSAGE)) {
            return;
        }
        dirtyRef.current = false;
        window.history.pushState(null, "", routePath(next));
        setRoute(next);
    };

    const changeLocale = (next: string) => {
        if (next === locale) {
            return;
        }
        if (dirtyRef.current && !window.confirm(DISCARD_MESSAGE)) {
            return;
        }
        dirtyRef.current = false;
        setLocale(next);
    };

    return (
        <div className="app-shell">
            <header className="top-app-bar">
                <button className="brand-button" onClick={() => navigate({ kind: "home" })} type="button">
                    <span className="brand-mark" aria-hidden="true">A</span>
                    <span><strong>ALCOMD3</strong><small>{productFamily} platform</small></span>
                </button>
                <AppearanceControls
                    appearance={appearance}
                    locale={locale}
                    onAppearanceChange={setAppearance}
                    onLocaleChange={changeLocale}
                />
            </header>
            <main className="main-content">
                {route.kind === "home" ? (
                    <HomePage onOpen={(extensionId) => navigate({ kind: "extension-ui", extensionId })} />
                ) : (
                    <ExtensionUiPage
                        client={guiRpcClient}
                        extensionId={route.extensionId}
                        locale={locale}
                        onDirtyChange={handleDirtyChange}
                    />
                )}
            </main>
        </div>
    );
}

function HomePage({ onOpen }: { onOpen(extensionId: string): void }) {
    const [extensionId, setExtensionId] = useState("");
    const valid = /^[a-z][a-z0-9._-]{0,63}$/.test(extensionId);
    return (
        <section className="home-card" aria-labelledby="home-title">
            <p className="eyebrow">Official desktop client</p>
            <h1 id="home-title">ALCOMD3</h1>
            <p className="supporting-text">
                Open a Portable UI exposed by an enabled extension. Extension content remains inside the
                official renderer and cannot provide its own HTML, CSS, script, or Tauri authority.
            </p>
            <form
                className="open-extension-form"
                onSubmit={(event) => {
                    event.preventDefault();
                    if (valid) {
                        onOpen(extensionId);
                    }
                }}
            >
                <label>
                    <span>Extension ID</span>
                    <input
                        aria-describedby="extension-id-hint"
                        autoComplete="off"
                        maxLength={64}
                        onChange={(event) => setExtensionId(event.currentTarget.value)}
                        pattern="[a-z][a-z0-9._-]{0,63}"
                        required
                        spellCheck={false}
                        value={extensionId}
                    />
                </label>
                <p className="field-hint" id="extension-id-hint">Use the installed extension's stable ID.</p>
                <button className="button button--filled" disabled={!valid} type="submit">Open extension UI</button>
            </form>
        </section>
    );
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

    const refresh = async () => {
        if (snapshot === undefined || sessionIdRef.current === undefined || busy) {
            return;
        }
        if (dirty && !window.confirm(DISCARD_MESSAGE)) {
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
        if (dirty && !window.confirm(DISCARD_MESSAGE)) {
            return;
        }
        updateDirty(false);
        setGeneration((current) => current + 1);
    };

    return (
        <section className="extension-surface" aria-labelledby="extension-ui-title">
            <header className="extension-chrome">
                <div>
                    <p className="eyebrow">Extension-provided content</p>
                    <h1 id="extension-ui-title" ref={headingRef} tabIndex={-1}>Extension UI</h1>
                    <p className="extension-id"><code>{extensionId}</code></p>
                </div>
                <button className="button button--tonal" disabled={busy || snapshot === undefined} onClick={() => void refresh()} type="button">
                    {busy && snapshot !== undefined ? "Working…" : "Refresh"}
                </button>
            </header>
            {extension === undefined ? null : <ExtensionIdentity extension={extension} />}
            {busy && snapshot === undefined ? (
                <StatePanel kind="loading" title="Opening extension UI" detail="Connecting to the ALCOMD core and extension host…" />
            ) : null}
            {error === undefined ? null : (
                <StatePanel
                    action={<button className="button button--filled" onClick={reconnect} type="button">Reconnect</button>}
                    detail={error.diagnosticId === undefined ? undefined : `Diagnostic ID: ${error.diagnosticId}`}
                    kind={error.code === "daemon_unavailable" ? "disconnected" : "error"}
                    title={errorTitle(error.code)}
                />
            )}
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

interface AppearanceControlsProps {
    appearance: AppearanceSettings;
    locale: string;
    onAppearanceChange(settings: AppearanceSettings): void;
    onLocaleChange(locale: string): void;
}

function AppearanceControls({
    appearance,
    locale,
    onAppearanceChange,
    onLocaleChange
}: AppearanceControlsProps) {
    const set = <Key extends keyof AppearanceSettings>(key: Key, value: AppearanceSettings[Key]) => {
        onAppearanceChange({ ...appearance, [key]: value });
    };
    return (
        <details className="appearance-menu">
            <summary>Appearance and language</summary>
            <div className="appearance-grid">
                <label>Theme
                    <select value={appearance.mode} onChange={(event) => set("mode", event.currentTarget.value as AppearanceMode)}>
                        <option value="system">System</option><option value="light">Light</option><option value="dark">Dark</option>
                    </select>
                </label>
                <label>Color
                    <select value={appearance.sourceColor} onChange={(event) => set("sourceColor", event.currentTarget.value as SourceColor)}>
                        <option value="violet">Violet</option><option value="blue">Blue</option><option value="teal">Teal</option>
                    </select>
                </label>
                <label>Density
                    <select value={appearance.density} onChange={(event) => set("density", event.currentTarget.value as InterfaceDensity)}>
                        <option value="comfortable">Comfortable</option><option value="compact">Compact</option>
                    </select>
                </label>
                <label>Language
                    <select value={locale} onChange={(event) => onLocaleChange(event.currentTarget.value)}>
                        <option value="en-US">English</option><option value="zh-CN">简体中文</option><option value="ja-JP">日本語</option>
                    </select>
                </label>
            </div>
            <p className="field-hint">Appearance remains host-owned and is never sent to extensions.</p>
        </details>
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

function readRoute(pathname: string): Route {
    const match = /^\/extensions\/([a-z][a-z0-9._-]{0,63})\/ui\/?$/.exec(pathname);
    return match?.[1] === undefined ? { kind: "home" } : { kind: "extension-ui", extensionId: match[1] };
}

function routePath(route: Route): string {
    return route.kind === "home" ? "/" : `/extensions/${route.extensionId}/ui`;
}
