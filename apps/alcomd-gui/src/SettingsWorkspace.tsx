import { useCallback, useEffect, useRef, useState, type FormEvent, type ReactNode } from "react";

import type { OfficialSettings, RepositorySnapshot, SettingsGetResult, SettingsLocale, UnityInstallation } from "./core-models";
import type { GuiRpcClient } from "./rpc";
import { UnityRegistryActions } from "./CoreActions";
import { capabilities, useCapability } from "./capabilities";
import { DataTableHeader, MaterialDataTable } from "./DataTable";
import { Button, Checkbox, Select } from "./Material";
import { UtilityActionDialog } from "./UtilityActionDialog";
import { UtilityWorkspace } from "./UtilityWorkspace";

interface SettingsWorkspaceProps {
    client: GuiRpcClient;
    navigate(path: string): void;
    onApplied(value: SettingsGetResult): void;
    onDirtyChange(dirty: boolean): void;
}

export function SettingsWorkspace({ client, navigate, onApplied, onDirtyChange }: SettingsWorkspaceProps) {
    const load = useCallback(() => client.settingsGet(), [client]);
    const [dirty, setDirty] = useState(false);
    const dirtyChanged = useCallback((value: boolean) => { setDirty(value); onDirtyChange(value); }, [onDirtyChange]);
    return (
        <UtilityWorkspace title="Settings" load={load} refreshDisabled={dirty}>
            {(snapshot, refresh) => (
                <div className="settings-workspace">
                    <UnitySettingsSection client={client} />
                    <SettingsEditor client={client} key={snapshot.revision} snapshot={snapshot}
                        onApplied={(next) => { onApplied(next); refresh(); }} onDirtyChange={dirtyChanged} />
                    <SettingsSection title="System information">
                        <div className="action-row">
                            <Button onClick={() => navigate("/about")} type="button" variant="tonal">About ALCOMD</Button>
                            <Button onClick={() => navigate("/diagnostics")} type="button" variant="text">Diagnostics</Button>
                        </div>
                    </SettingsSection>
                </div>
            )}
        </UtilityWorkspace>
    );
}

function SettingsSection({ title, children, actions }: { title: string; children: ReactNode; actions?: ReactNode }) {
    return (
        <section className="settings-section" aria-label={title}>
            <header className="settings-section-heading"><h2>{title}</h2>{actions}</header>
            <div className="settings-section-content">{children}</div>
        </section>
    );
}

function UnitySettingsSection({ client }: { client: GuiRpcClient }) {
    const available = useCapability(capabilities.unityRead);
    const manage = useCapability(capabilities.unityManage);
    const [items, setItems] = useState<UnityInstallation[]>([]);
    const [cursor, setCursor] = useState<string>();
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState<string>();
    const [open, setOpen] = useState(false);
    const [generation, setGeneration] = useState(0);
    const requestRef = useRef(0);
    const busyRef = useRef(false);

    const read = useCallback(async (next?: string) => {
        if (busyRef.current) return;
        const request = ++requestRef.current;
        busyRef.current = true;
        setBusy(true);
        setError(undefined);
        try {
            const page = await client.unityInstallationsList(next);
            if (request !== requestRef.current) return;
            setItems((current) => next === undefined ? page.installations : [...current, ...page.installations]);
            setCursor(page.nextCursor);
        } catch {
            if (request === requestRef.current) setError("Unity installations could not be loaded.");
        } finally {
            if (request === requestRef.current) { busyRef.current = false; setBusy(false); }
        }
    }, [client]);

    useEffect(() => {
        if (available) void read();
        return () => { requestRef.current += 1; busyRef.current = false; };
    }, [available, read, generation]);

    return (
        <SettingsSection title="Unity installations" actions={
            <div className="action-row">
                <Button disabled={!available || busy} onClick={() => setGeneration((current) => current + 1)} type="button" variant="tonal">Refresh</Button>
                <Button disabled={!manage} onClick={() => setOpen(true)} type="button">Manage installations</Button>
            </div>
        }>
            {!available ? <p className="field-hint">Unity installations are unavailable on this connection.</p> : (
                <>
                    {error === undefined ? null : <p className="form-error" role="alert">{error}</p>}
                    <MaterialDataTable label="Unity installations" minWidth={600}>
                        <thead><tr><DataTableHeader>Version</DataTableHeader><DataTableHeader>Path</DataTableHeader><DataTableHeader>Source</DataTableHeader></tr></thead>
                        <tbody>{items.map((item) => <tr key={item.installationId}><td>{item.unityVersion}</td><td className="settings-path">{item.executablePath}</td><td>{item.sourceKind.replaceAll("_", " ")}</td></tr>)}</tbody>
                    </MaterialDataTable>
                    {items.length === 0 && error === undefined ? <p role="status">{busy ? "Loading Unity installations…" : "No Unity installations found."}</p> : null}
                    {cursor === undefined ? null : <Button disabled={busy} onClick={() => void read(cursor)} type="button" variant="text">Load more installations</Button>}
                </>
            )}
            {open ? <UtilityActionDialog title="Manage Unity installations" onClose={() => setOpen(false)}>
                <UnityRegistryActions client={client} installations={items} onChanged={() => setGeneration((current) => current + 1)} />
            </UtilityActionDialog> : null}
        </SettingsSection>
    );
}

function SettingsEditor({ client, snapshot, onApplied, onDirtyChange }: Omit<SettingsWorkspaceProps, "navigate"> & { snapshot: SettingsGetResult }) {
    const [settings, setSettings] = useState<OfficialSettings>(snapshot.settings);
    const [busy, setBusy] = useState(false);
    const busyRef = useRef(false);
    const [error, setError] = useState<string>();
    const [repositories, setRepositories] = useState<RepositorySnapshot[]>([]);
    const [repositoryCursor, setRepositoryCursor] = useState<Parameters<GuiRpcClient["repositoriesList"]>[0]>();
    const [repositoryError, setRepositoryError] = useState(false);
    const [repositoryBusy, setRepositoryBusy] = useState(false);
    const repositoryBusyRef = useRef(false);
    const repositoryRequestRef = useRef(0);
    const canReadRepositories = useCapability(capabilities.repositoriesRead);
    const dirty = JSON.stringify(settings) !== JSON.stringify(snapshot.settings);

    useEffect(() => {
        onDirtyChange(dirty);
        return () => onDirtyChange(false);
    }, [dirty, onDirtyChange]);

    const readRepositories = useCallback(async (cursor?: Parameters<GuiRpcClient["repositoriesList"]>[0]) => {
        if (repositoryBusyRef.current) return;
        const request = ++repositoryRequestRef.current;
        repositoryBusyRef.current = true;
        setRepositoryBusy(true);
        setRepositoryError(false);
        try {
            const page = await client.repositoriesList(cursor);
            if (request !== repositoryRequestRef.current) return;
            setRepositories((current) => cursor === undefined ? page.repositories : [...current, ...page.repositories]);
            setRepositoryCursor(page.nextCursor);
        } catch {
            if (request === repositoryRequestRef.current) setRepositoryError(true);
        } finally {
            if (request === repositoryRequestRef.current) { repositoryBusyRef.current = false; setRepositoryBusy(false); }
        }
    }, [client]);

    useEffect(() => {
        if (canReadRepositories) void readRepositories();
        return () => { repositoryRequestRef.current += 1; repositoryBusyRef.current = false; };
    }, [canReadRepositories, readRepositories]);

    const appearance = <K extends keyof OfficialSettings["appearance"]>(key: K, value: OfficialSettings["appearance"][K]) => {
        setSettings((current) => ({ ...current, appearance: { ...current.appearance, [key]: value } }));
    };
    const packages = <K extends keyof OfficialSettings["packages"]>(key: K, value: OfficialSettings["packages"][K]) => {
        setSettings((current) => ({ ...current, packages: { ...current.packages, [key]: value } }));
    };
    const save = async (event: FormEvent) => {
        event.preventDefault();
        if (!dirty || busyRef.current) return;
        busyRef.current = true;
        setBusy(true);
        setError(undefined);
        try {
            const next = await client.settingsUpdate(snapshot.revision, settings);
            onDirtyChange(false);
            onApplied(next);
        } catch (caught: unknown) {
            const conflict = typeof caught === "object" && caught !== null && "code" in caught && caught.code === "revision_conflict";
            setError(conflict ? "Settings changed elsewhere. Discard these changes and refresh before trying again." : "Settings could not be saved. Your changes are still here.");
        } finally {
            busyRef.current = false;
            setBusy(false);
        }
    };
    const colors = [{ label: "Product default", value: "" }, { label: "Violet", value: "#6750A4" }, { label: "Blue", value: "#315DA8" }, { label: "Teal", value: "#006A60" }];
    if (settings.appearance.sourceColor !== null && !colors.some((color) => color.value === settings.appearance.sourceColor)) {
        colors.push({ label: settings.appearance.sourceColor, value: settings.appearance.sourceColor });
    }

    return (
        <form className="settings-editor" onSubmit={(event) => void save(event)}>
            <SettingsSection title="Packages">
                <div className="settings-toggle-row">
                    <Checkbox checked={settings.packages.showPrerelease} disabled={busy} label="Show prerelease package versions" onChange={(checked) => packages("showPrerelease", checked)} />
                    <p className="field-hint">Include prerelease versions in package lists.</p>
                </div>
                <div className="settings-toggle-row">
                    <Checkbox checked={settings.packages.hideLocalUserPackages} disabled={busy} label="Hide local User Packages" onChange={(checked) => packages("hideLocalUserPackages", checked)} />
                </div>
                <div className="settings-source-options">
                    {repositories.map((repository) => repository.repositoryId === undefined ? null : (
                        <Checkbox key={repository.repositoryId} checked={settings.packages.hiddenRepositoryIds.includes(repository.repositoryId)} disabled={busy}
                            label={`Hide ${repository.name ?? repository.declaredId ?? repository.repositoryId}`}
                            onChange={(checked) => {
                                const hidden = new Set(settings.packages.hiddenRepositoryIds);
                                if (checked) hidden.add(repository.repositoryId as string);
                                else hidden.delete(repository.repositoryId as string);
                                packages("hiddenRepositoryIds", [...hidden].sort());
                            }} />
                    ))}
                    {repositoryError ? <div role="alert"><p>Repository visibility options could not be loaded.</p><Button disabled={repositoryBusy} onClick={() => void readRepositories(repositoryCursor)} type="button" variant="text">Retry repository options</Button></div> : null}
                    {repositoryCursor === undefined ? null : <Button disabled={repositoryBusy} onClick={() => void readRepositories(repositoryCursor)} type="button" variant="text">Load more repository options</Button>}
                </div>
            </SettingsSection>
            <SettingsSection title="Appearance">
                <div className="settings-row">
                    <Select id="settings-locale" disabled={busy} label="Language" onChange={(value) => setSettings((current) => ({ ...current, locale: value as SettingsLocale }))}
                        options={[{ label: "System language", value: "system" }, { label: "English", value: "en-US" }, { label: "简体中文", value: "zh-CN" }, { label: "日本語", value: "ja-JP" }]} value={settings.locale} />
                </div>
                <div className="settings-toggle-row">
                    <Checkbox checked={settings.appearance.motion === "system"} disabled={busy} label="GUI animation" onChange={(checked) => appearance("motion", checked ? "system" : "reduced")} />
                    <p className="field-hint">Enable interface animation while respecting your system’s reduced-motion preference.</p>
                </div>
                <div className="settings-toggle-row">
                    <Checkbox checked={settings.appearance.density === "compact"} disabled={busy} label="Compact GUI" onChange={(checked) => appearance("density", checked ? "compact" : "default")} />
                    <p className="field-hint">Reduce spacing to show more content.</p>
                </div>
            </SettingsSection>
            <SettingsSection title="Theme">
                <div className="settings-row"><Select id="settings-theme" disabled={busy} label="Theme" onChange={(value) => appearance("mode", value as OfficialSettings["appearance"]["mode"])} options={[{ label: "System", value: "system" }, { label: "Light", value: "light" }, { label: "Dark", value: "dark" }]} value={settings.appearance.mode} /></div>
                <div className="settings-row"><Select id="settings-color" disabled={busy} label="Source color" supportingText="Choose the accent color used throughout the app." onChange={(value) => appearance("sourceColor", value || null)} options={colors} value={settings.appearance.sourceColor ?? ""} /></div>
            </SettingsSection>
            <div className="settings-save-bar">
                {error === undefined ? null : <p className="form-error" role="alert">{error}</p>}
                <div className="action-row">
                    <Button disabled={!dirty || busy} type="submit">{busy ? "Saving…" : "Save settings"}</Button>
                    <Button disabled={!dirty || busy} onClick={() => { setSettings(snapshot.settings); setError(undefined); }} type="button" variant="tonal">Discard changes</Button>
                    <span className="field-hint" role="status">{dirty ? "Unsaved changes" : "All changes saved"}</span>
                </div>
            </div>
        </form>
    );
}
