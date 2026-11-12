import { useEffect, useMemo, useRef, useState } from "react";
import type { PackageCursor, RepositoryPackagesResult, RepositoryPackageVersion, RepositorySnapshot } from "./core-models";
import type { RepositoryPackageSettings, RpcError } from "@alcomd/sdk";
import { useActionScope } from "./CoreActions";
import { capabilities, useCapability } from "./capabilities";
import { DesktopList, type DesktopListColumn } from "./DesktopList";
import type { Translate } from "./i18n-types";
import { useTranslation } from "./i18n";
import { LoadingState } from "./LoadingState";
import { Button, Checkbox, RefreshButton, Select } from "./Material";
import { PagedItems } from "./PagedItems";
import { normalizeRequestError } from "./request-errors";
import type { GuiRpcClient } from "./rpc";

export function RepositoryRefresh({ client, repository, onChanged }: { client: GuiRpcClient; repository: RepositorySnapshot; onChanged(): void }) {
    const { t } = useTranslation();
    const available = useCapability(capabilities.repositoriesRegistry);
    const current = useActionScope(client);
    const latest = useRef(repository);
    if (latest.current.repositoryId !== repository.repositoryId || (repository.revision ?? 0) > (latest.current.revision ?? 0)) latest.current = repository;
    const pending = useRef(false);
    const [busy, setBusy] = useState(false);
    const refresh = async () => {
        const { repositoryId, revision } = latest.current;
        if (!available || pending.current || repositoryId === undefined || revision === undefined) return;
        pending.current = true;
        setBusy(true);
        try {
            const result = await client.repositoryRefresh(repositoryId, revision);
            if (!current()) return;
            latest.current = result.repository;
            onChanged();
        } catch (error) {
            if (current() && normalizeRequestError(error).code === "revision_conflict") onChanged();
        } finally {
            pending.current = false;
            if (current()) setBusy(false);
        }
    };
    return <RefreshButton disabled={!available || repository.repositoryId === undefined || repository.revision === undefined || busy}
        label={t("Refresh repository")} refreshingLabel={t("Refreshing repository")} refreshing={busy} onClick={() => refresh()} />;
}

interface PackageVisibility {
    groups?: RepositoryPackageSettings[];
    busy: boolean;
    save(groups: RepositoryPackageSettings[]): void;
}

export function repositoryPackageColumns(t: Translate): DesktopListColumn[] {
    return [{ key: "enabled", header: t("Enabled"), width: "64px" }, { key: "name", header: t("Package") }];
}

export function RepositoryPackageCatalog({ client, repositoryId, initial, onSelect, visibility }: { client: GuiRpcClient; repositoryId: string; initial: RepositoryPackagesResult; onSelect(id: string | undefined): void; visibility: PackageVisibility }) {
    const [selectedId, setSelectedId] = useState<string>();
    const [version, setVersion] = useState<string>();
    const select = (id: string | undefined) => { setSelectedId(id); setVersion(undefined); onSelect(id); };
    return <PagedItems initialItems={initial.packages} initialCursor={initial.nextCursor} showPagination={selectedId === undefined}
        loadMore={async (cursor) => { const result = await client.repositoryPackages(repositoryId, cursor); return { items: result.packages, nextCursor: result.nextCursor }; }}>
        {(items, pagination) => <CatalogContents client={client} repositoryId={repositoryId} visibility={visibility} items={items} selectedId={selectedId} select={select} version={version} setVersion={setVersion} pagination={pagination} />}
    </PagedItems>;
}

function CatalogContents({ client, items, selectedId, select, pagination, version, setVersion, repositoryId, visibility }: {
    repositoryId: string; visibility: PackageVisibility;
    client: GuiRpcClient; items: RepositoryPackageVersion[]; selectedId?: string; select(id: string | undefined): void;
    pagination: { cursor?: PackageCursor; loading: boolean; error?: RpcError; next(): Promise<void> };
    version?: string; setVersion(version: string): void;
}) {
    const { t } = useTranslation();
    const origins = useRef(new Map<string, HTMLElement>());
    const returnTo = useRef<string | undefined>(undefined);
    const heading = useRef<HTMLHeadingElement>(null);
    const toggle = (id: string, name: string, labelHidden = false) => {
        const groups = visibility.groups;
        const group = groups?.find((item) => item.repositoryId === repositoryId);
        const checked = groups !== undefined && !group?.packageIds.includes(id);
        const count = groups?.reduce((total, item) => total + item.packageIds.length, 0) ?? 0;
        return <Checkbox checked={checked} label={t("Enable {name} from this repository", { name })} labelHidden={labelHidden}
            disabled={groups === undefined || visibility.busy || checked && (count >= 4096 || group === undefined && groups.length >= 256)}
            onChange={(enabled) => {
                if (groups === undefined || enabled === checked) return;
                const ids = enabled ? group?.packageIds.filter((item) => item !== id) ?? [] : [...group?.packageIds ?? [], id];
                const next = groups.filter((item) => item.repositoryId !== repositoryId);
                if (ids.length) next.push({ repositoryId, packageIds: ids });
                visibility.save(next);
            }} />;
    };
    const groups = useMemo(() => {
        const result = new Map<string, RepositoryPackageVersion[]>();
        const seen = new Set<string>();
        for (const item of items) {
            const key = JSON.stringify([item.packageId, item.version]);
            if (seen.has(key)) continue;
            seen.add(key);
            const group = result.get(item.packageId) ?? [];
            group.push(item);
            result.set(item.packageId, group);
        }
        return result;
    }, [items]);
    const versions = selectedId === undefined ? undefined : groups.get(selectedId);
    // Raw identity pagination keeps each package's versions contiguous. Read only
    // the selected trailing group to its boundary, without scanning the rest.
    const incomplete = versions !== undefined && pagination.cursor?.packageId === selectedId;
    useEffect(() => {
        if (selectedId !== undefined && versions === undefined) select(undefined);
        if (incomplete && !pagination.loading && !pagination.error) void pagination.next();
    }, [selectedId, versions, incomplete, pagination.cursor, pagination.loading, pagination.error, pagination.next, select]);
    useEffect(() => {
        if (selectedId !== undefined || returnTo.current === undefined) return;
        const origin = origins.current.get(returnTo.current) as (HTMLElement & { updateComplete?: Promise<unknown> }) | undefined;
        let active = true;
        // The name button is mounted again after leaving details. Material must
        // finish creating its focusable shadow control before focus can return.
        void Promise.resolve(origin?.updateComplete).then(() => {
            if (active && origin?.isConnected) { origin.focus(); returnTo.current = undefined; }
        });
        return () => { active = false; };
    }, [selectedId]);
    useEffect(() => { if (selectedId !== undefined) heading.current?.focus(); }, [selectedId]);
    if (versions !== undefined && selectedId !== undefined) return <section className="repository-package-detail">
        <Button variant="text" type="button" onClick={() => { returnTo.current = selectedId; select(undefined); }}>{t("Back to packages")}</Button>
        <h2 ref={heading} tabIndex={-1}>{versions.find((item) => item.displayName)?.displayName ?? selectedId}</h2>
        {toggle(selectedId, versions.find((item) => item.displayName)?.displayName ?? selectedId)}
        {incomplete ? <>
            <dl className="detail-grid"><div><dt>{t("Package ID")}</dt><dd>{selectedId}</dd></div></dl>
            {pagination.error ? <Button variant="text" type="button" disabled={pagination.loading} onClick={() => void pagination.next()}>{t("Retry loading more")}</Button> : <LoadingState title={t("Loading versions…")} variant="bar" />}
        </> : <PackageDetails client={client} versions={versions} version={version} setVersion={setVersion} />}
    </section>;
    return <DesktopList className="repository-package-list" label={t("Repository packages")} minWidth={320}
        columns={repositoryPackageColumns(t)} emptyMessage={t("No matching items")} rows={[...groups].map(([id, group]) => ({ key: id, cells: [
            toggle(id, group.find((item) => item.displayName)?.displayName ?? id, true),
            <Button className="repository-package-name" ref={(element) => { if (element) origins.current.set(id, element); else origins.current.delete(id); }}
                variant="text" type="button" title={group.find((item) => item.displayName)?.displayName ?? id} onClick={() => select(id)}>{group.find((item) => item.displayName)?.displayName ?? id}</Button>
        ] }))} />;
}

function PackageDetails({ client, versions, version, setVersion }: { client: GuiRpcClient; versions: RepositoryPackageVersion[]; version?: string; setVersion(version: string): void }) {
    const { t } = useTranslation();
    const selected = versions.find((item) => item.version === version) ?? versions[0]!;
    const open = async (url: string | undefined) => {
        if (url === undefined) return;
        try { await client.openPackageLink(url); } catch {}
    };
    return <>
        <div className="repository-package-version"><span>{t("Version")}</span><Select label={t("Version")} value={selected.version} onChange={setVersion}
            options={versions.map((item) => ({ value: item.version, label: item.version }))} /></div>
        <dl className="detail-grid">
            <div><dt>{t("Package ID")}</dt><dd>{selected.packageId}</dd></div>
            <div><dt>{t("Unity")}</dt><dd>{selected.unity ?? t("Unknown")}</dd></div>
            <div><dt>{t("Status")}</dt><dd>{selected.yanked ? t("Yanked") : selected.prerelease ? t("Prerelease") : t("Available")}</dd></div>
        </dl>
        {selected.description === undefined ? null : <p className="repository-package-description">{selected.description}</p>}
        <div className="action-row">
            <Button variant="text" type="button" disabled={selected.links?.documentation === undefined} onClick={() => void open(selected.links?.documentation?.url)}>{t("Documentation")}</Button>
            <Button variant="text" type="button" disabled={selected.links?.changelog === undefined} onClick={() => void open(selected.links?.changelog?.url)}>{t("Changelog")}</Button>
        </div>
    </>;
}
