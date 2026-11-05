import { useEffect, useRef, useState } from "react";
import type { ExtensionPermissionGrant, ExtensionPermissionsListResult, RpcError } from "@alcomd/sdk";
import type { ProjectSnapshot } from "./core-models";
import type { GuiRpcClient } from "./rpc";
import { Button, Select } from "./Material";
import { Reminder, RequestReminder } from "./Reminder";
import { LoadingState } from "./LoadingState";
import { capabilities, useCapability } from "./capabilities";
import { displayPath } from "./display-path";
import { useTranslation, type Translate } from "./i18n";
import { useCoreEvents, useCoreInvalidation } from "./core-events-ui";

export function hasPermissionRequests(value: { requiredPermissions?: unknown; optionalPermissions?: unknown }): boolean {
    return [value.requiredPermissions, value.optionalPermissions].every((items) => Array.isArray(items)
        && items.length <= 64 && items.every((item: unknown) => typeof item === "string" && item.length > 0 && item.length <= 128));
}

function permissionName(permission: string, t: Translate): string {
    if (permission === "background.run") return t("Run in background");
    if (permission === "projects.read") return t("Read projects");
    return t("Unknown permission: {permission}", { permission });
}

export function ExtensionPermissionRequests({ required, optional }: { required: string[]; optional: string[] }) {
    const { t } = useTranslation();
    return <section aria-label={t("Requested permissions")}>
        <h3>{t("Requested permissions")}</h3>
        {required.length === 0 && optional.length === 0 ? <p>{t("No permissions requested")}</p> : <dl className="dialog-summary">
            {required.length === 0 ? null : <div><dt>{t("Required permissions")}</dt><dd>{required.map((permission) => permissionName(permission, t)).join(" · ")}</dd></div>}
            {optional.length === 0 ? null : <div><dt>{t("Optional permissions")}</dt><dd>{optional.map((permission) => permissionName(permission, t)).join(" · ")}</dd></div>}
        </dl>}
    </section>;
}

export function ExtensionPermissions({ client, extensionId, onBusyChange, onChanged, onClose }: {
    client: GuiRpcClient;
    extensionId: string;
    onBusyChange(busy: boolean): void;
    onChanged?(): void;
    onClose(): void;
}) {
    const { t } = useTranslation();
    const canRead = useCapability(capabilities.extensionsPermissionsRead);
    const canWrite = useCapability(capabilities.extensionsPermissions);
    const canReadProjects = useCapability(capabilities.projectsRead);

    const events = useCoreEvents();
    const readVersion = useRef(0);
    const projectReadVersion = useRef(0);
    const [generation, setGeneration] = useState(0);
    const [value, setValue] = useState<ExtensionPermissionsListResult>();
    const [projects, setProjects] = useState<ProjectSnapshot[]>([]);
    const [projectsError, setProjectsError] = useState<RpcError>();
    const [projectsLoading, setProjectsLoading] = useState(false);
    const [error, setError] = useState<RpcError>();
    const [loading, setLoading] = useState(true);
    const [moreLoading, setMoreLoading] = useState(false);
    const [writing, setWriting] = useState(false);
    const [writeUnknown, setWriteUnknown] = useState(false);
    const [permission, setPermission] = useState<ExtensionPermissionGrant["permission"]>("projects.read");
    const [projectId, setProjectId] = useState("");
    const owner = useRef({ client, extensionId, generation, active: true, pending: false });
    if (owner.current.client !== client || owner.current.extensionId !== extensionId || owner.current.generation !== generation) {
        owner.current.active = false;
        owner.current = { client, extensionId, generation, active: true, pending: false };
    }
    const scope = owner.current;
    const current = () => scope.active && owner.current === scope;
    const reload = () => { scope.active = false; setGeneration((version) => version + 1); };
    const readPermissions = async () => {
        if (!canRead || !current()) return;
        const version = ++readVersion.current;
        try {
            const load = async () => {
                let result = await client.extensionPermissionsList(extensionId);
                if (result.extensionId !== extensionId || !hasPermissionRequests(result)) throw { code: "internal_error" };
                const target = value?.grants.length ?? 0;
                while (result.nextCursor !== undefined && result.grants.length < target) {
                    const page = await client.extensionPermissionsList(extensionId, result.nextCursor);
                    if (page.extensionId !== extensionId || page.grantRevision !== result.grantRevision) throw { code: "revision_conflict" };
                    result = { ...page, grants: [...result.grants, ...page.grants] };
                }
                return result;
            };
            const result = events ? await events.consistentRead(["extensions"], load) : await load();
            if (current() && version === readVersion.current) { setValue(result); setError(undefined); }
        } catch (caught: unknown) {
            if (current() && version === readVersion.current) setError(requestError(caught));
            throw caught;
        } finally { if (current() && version === readVersion.current) setLoading(false); }
    };
    useCoreInvalidation("extension-permissions:" + extensionId, ["extensions"], readPermissions);

    useEffect(() => {
        scope.active = true;
        setValue(undefined); setError(undefined); setLoading(true); setMoreLoading(false); setWriting(false); setWriteUnknown(false);
        if (canRead) void readPermissions().catch(() => {}); else setLoading(false);
        return () => { scope.active = false; readVersion.current += 1; };
    }, [canRead, client, extensionId, scope]);

    const readProjects = async () => {
        if (!canReadProjects || !current()) return;
        const version = ++projectReadVersion.current;
        try {
            const load = async () => {
            const all: ProjectSnapshot[] = [];
            let cursor: Parameters<GuiRpcClient["projectsList"]>[0];
            do {
                const page = await client.projectsList(cursor);
                all.push(...page.projects); cursor = page.nextCursor;
            } while (cursor !== undefined);
                return all;
            };
            const all = events ? await events.consistentRead(["projects"], load) : await load();
            if (current() && version === projectReadVersion.current) { setProjects(all); setProjectsError(undefined); }
        } catch (caught: unknown) {
            if (current() && version === projectReadVersion.current) setProjectsError(requestError(caught));
            throw caught;
        } finally { if (current() && version === projectReadVersion.current) setProjectsLoading(false); }
    };
    useCoreInvalidation("extension-permission-projects:" + extensionId, ["projects"], readProjects);
    useEffect(() => {
        setProjects([]); setProjectsError(undefined); setProjectsLoading(canReadProjects);
        if (canReadProjects) void readProjects().catch(() => {});
        return () => { projectReadVersion.current += 1; };
    }, [canReadProjects, client, extensionId, generation]);

    const more = async () => {
        if (value?.nextCursor === undefined || scope.pending || !current()) return;
        const version = readVersion.current;
        scope.pending = true; setMoreLoading(true); setError(undefined);
        try {
            const page = await client.extensionPermissionsList(extensionId, value.nextCursor);
            if (!current() || version !== readVersion.current) return;
            if (page.extensionId !== extensionId || page.grantRevision !== value.grantRevision) throw { code: "revision_conflict" };
            setValue({ ...page, grants: [...value.grants, ...page.grants] });
        } catch (caught: unknown) {
            if (!current()) return;
            const nextError = requestError(caught);
            if (nextError.code === "revision_conflict") setValue(undefined);
            setError(nextError);
        } finally { scope.pending = false; if (current()) setMoreLoading(false); }
    };

    const write = async (grant: ExtensionPermissionGrant, revoke: boolean) => {
        if (!canWrite || !current() || scope.pending || value === undefined) return;
        scope.pending = true; setWriting(true); setError(undefined);  onBusyChange(true);
        try {
            await (revoke
                ? client.extensionRevokeGrant(extensionId, grant.permission, grant.resourceKind, grant.resourceId, value.grantRevision)
                : client.extensionSetGrant(extensionId, grant.permission, grant.resourceKind, grant.resourceId, value.grantRevision));
            if (!current()) return;

            onChanged?.(); reload();
        } catch (caught: unknown) {
            if (current()) {
                const nextError = requestError(caught);
                const unknown = nextError.code === "daemon_unavailable" || nextError.code === "internal_error";
                if (unknown || nextError.code === "revision_conflict") setValue(undefined);
                setWriteUnknown(unknown); setError(nextError);
            }
        } finally { scope.pending = false; if (owner.current === scope) setWriting(false); onBusyChange(false); }
    };
    const scopeLabel = (grant: ExtensionPermissionGrant) => grant.resourceKind === "Extension"
        ? grant.resourceId === extensionId ? t("This extension") : grant.resourceId
        : displayPath(projects.find((project) => project.projectId === grant.resourceId)?.rootPath ?? t("Project {id}", { id: grant.resourceId }));
    const requested = value === undefined ? [] : [...new Set([...value.requiredPermissions, ...value.optionalPermissions])].filter((item) => item === "background.run" || item === "projects.read");
    const selected = requested.includes(permission) ? permission : requested[0];
    return <section className="extension-permissions" aria-label={t("Extension permissions")}>
        {!canRead ? <Reminder tone="warning">{t("Permission details are unavailable from this Core.")}</Reminder> : loading ? <LoadingState title={t("Loading…")} /> : null}
        {error === undefined ? null : <RequestReminder error={error}>{!writeUnknown ? null : <p>{t("The permission change may have completed. Refresh permissions before trying again.")}</p>}<Button disabled={writing || moreLoading} onClick={reload} type="button" variant="text">{t("Refresh")}</Button></RequestReminder>}
        {value === undefined ? null : <>
            <ExtensionPermissionRequests required={value.requiredPermissions} optional={value.optionalPermissions} />
            <section aria-label={t("Allowed permissions")}>
                <h3>{t("Allowed permissions")}</h3>
                {value.grants.length === 0 ? <p>{t("No permissions granted")}</p> : <ul className="extension-grants">{value.grants.map((grant) => <li key={`${grant.permission}:${grant.resourceKind}:${grant.resourceId}`}>
                    <div><strong>{permissionName(grant.permission, t)}</strong><span title={grant.resourceId}>{scopeLabel(grant)}</span></div>
                    <Button disabled={!canWrite || writing || moreLoading} onClick={() => void write(grant, true)} type="button" variant="text">{t("Revoke")}</Button>
                </li>)}</ul>}
                {value.nextCursor === undefined ? null : <Button disabled={writing || moreLoading} onClick={() => void more()} type="button" variant="text">{t("Load more")}</Button>}
            </section>
            {selected === undefined ? null : <>
                <Select disabled={writing || moreLoading} label={t("Permission")} value={selected} onChange={(next) => { if (next === "background.run" || next === "projects.read") { setPermission(next); setError(undefined); } }} options={requested.map((item) => ({ value: item, label: permissionName(item, t) }))} />
                {selected !== "projects.read" ? <p>{t("This extension")}</p> : <>
                    <Select disabled={!canReadProjects || projectsLoading || writing || moreLoading} label={t("Project")} value={projectId} onChange={(next) => { setProjectId(next); setError(undefined); }} options={[{ value: "", label: t("Select project") }, ...projects.flatMap((project) => project.projectId === undefined ? [] : [{ value: project.projectId, label: displayPath(project.rootPath) }])]} />
                    {projectsLoading ? <LoadingState title={t("Loading…")} /> : null}
                    {projectsError === undefined ? null : <RequestReminder error={projectsError}><Button disabled={writing || moreLoading} onClick={reload} type="button" variant="text">{t("Refresh")}</Button></RequestReminder>}
                </>}
                <div className="dialog-actions"><Button disabled={!canWrite || writing || moreLoading || (selected === "projects.read" && !projects.some((project) => project.projectId === projectId))} onClick={() => void write({ permission: selected, resourceKind: selected === "background.run" ? "Extension" : "Project", resourceId: selected === "background.run" ? extensionId : projectId }, false)} type="button">{t("Grant")}</Button></div>
            </>}
        </>}
        <div className="dialog-actions"><Button disabled={writing} onClick={onClose} type="button" variant="text">{t("Close")}</Button>{!canRead || value === undefined ? null : <Button disabled={writing || moreLoading} onClick={reload} type="button" variant="text">{t("Refresh")}</Button>}</div>
    </section>;
}

function requestError(caught: unknown): RpcError {
    if (typeof caught === "object" && caught !== null && "code" in caught && typeof caught.code === "string") {
        return { code: caught.code, message: "", ...("diagnosticId" in caught && typeof caught.diagnosticId === "string" ? { diagnosticId: caught.diagnosticId } : {}) };
    }
    return { code: "internal_error", message: "" };
}
