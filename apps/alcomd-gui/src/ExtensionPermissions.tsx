import { withActionNotification } from "./app-notifications";
import { requestErrorMessage } from "./request-errors";
import { useEffect, useRef, useState } from "react";
import type { ExtensionPermissionGrant, ExtensionPermissionsListResult, RpcError } from "@alcomd/sdk";
import type { ProjectSnapshot } from "./core-models";
import type { GuiRpcClient } from "./rpc";
import { DialogCloseButton, Button, LoadingButton, useActionFeedback, Select } from "./Material";
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
    if (permission === "mcp.clients.read") return t("Read MCP clients");
    if (permission === "mcp.clients.manage") return t("Manage MCP clients");
    return t("Unknown permission: {permission}", { permission });
}

function permissionDescription(permission: string, t: Translate): string {
    switch (permission) {
        case "background.run": return t("Run background tasks when this extension's page is closed.");
        case "projects.read": return t("View basic information about only the projects you allow below.");
        case "mcp.clients.read": return t("View all registered MCP clients and their grants.");
        case "mcp.clients.manage": return t("Create MCP clients, change their grants, and permanently revoke them.");
        default: return t("This app cannot grant this permission.");
    }
}

function editablePermission(permission: string): permission is ExtensionPermissionGrant["permission"] {
    return permission === "background.run" || permission === "projects.read" || permission === "mcp.clients.read" || permission === "mcp.clients.manage";
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

export function ExtensionPermissions({ client, extensionId, onBusyChange, onChanged }: {
    client: GuiRpcClient;
    extensionId: string;
    onBusyChange(busy: boolean): void;
    onChanged?(): void;
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
    const [writing, setWriting] = useState(false);
    const [writeUnknown, setWriteUnknown] = useState(false);
    const [projectId, setProjectId] = useState("");
    const owner = useRef({ client, extensionId, generation, active: true, pending: false });
    if (owner.current.client !== client || owner.current.extensionId !== extensionId || owner.current.generation !== generation) {
        owner.current.active = false;
        owner.current = { client, extensionId, generation, active: true, pending: false };
    }
    const scope = owner.current;
    const current = () => scope.active && owner.current === scope;
    const reload = () => { scope.active = false; setGeneration((version) => version + 1); };
    const refreshFeedback = useActionFeedback(loading || projectsLoading);
    const manualReload = () => refreshFeedback.run(async () => {
        setValue(undefined); setProjects([]); setError(undefined); setProjectsError(undefined);
        setLoading(canRead); setProjectsLoading(canReadProjects);
        try {
            await withActionNotification(client, "Refresh", async () => {
                const results = await Promise.allSettled([readPermissions(), readProjects()]);
                const failure = results.find((result) => result.status === "rejected");
                if (failure?.status === "rejected") throw failure.reason;
            });
            if (current()) setWriteUnknown(false);
        } catch { /* The read functions retain their local, actionable errors. */ }
    });
    const readPermissions = async () => {
        if (!canRead || !current()) return;
        const version = ++readVersion.current;
        setLoading(true);
        try {
            const load = async () => {
                let result = await client.extensionPermissionsList(extensionId);
                if (result.extensionId !== extensionId || !hasPermissionRequests(result)) throw { code: "internal_error" };
                while (result.nextCursor !== undefined) {
                    if (!current() || version !== readVersion.current) break;
                    const page = await client.extensionPermissionsList(extensionId, result.nextCursor);
                    if (page.extensionId !== extensionId || page.grantRevision !== result.grantRevision) throw { code: "revision_conflict" };
                    result = { ...page, grants: [...result.grants, ...page.grants] };
                }
                return result;
            };
            const result = events ? await events.consistentRead(["extensions"], load) : await load();
            if (current() && version === readVersion.current) { setValue(result); setError(undefined); }
        } catch (caught: unknown) {
            if (current() && version === readVersion.current) { setValue(undefined); setError(requestError(caught)); }
            throw caught;
        } finally { if (current() && version === readVersion.current) setLoading(false); }
    };
    useCoreInvalidation("extension-permissions:" + extensionId, ["extensions"], readPermissions);

    useEffect(() => {
        scope.active = true;
        setValue(undefined); setError(undefined); setLoading(true); setWriting(false); setWriteUnknown(false);
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

    const write = async (grant: ExtensionPermissionGrant, revoke: boolean) => {
        if (!canWrite || !current() || scope.pending || loading || writeUnknown || value === undefined) return;
        scope.pending = true; setWriting(true); setError(undefined);  onBusyChange(true);
        try {
            await (revoke
                ? client.extensionRevokeGrant(extensionId, grant.permission, grant.resourceKind, grant.resourceId, value.grantRevision)
                : client.extensionSetGrant(extensionId, grant.permission, grant.resourceKind, grant.resourceId, value.grantRevision));
            if (!current()) return;

            if (!revoke && grant.permission === "projects.read") setProjectId("");
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
    const projectName = (id: string) => {
        const project = projects.find((item) => item.projectId === id);
        return project ? project.displayName || displayPath(project.rootPath) : t("Project {id}", { id });
    };
    const projectPath = (id: string) => {
        const project = projects.find((item) => item.projectId === id);
        return project ? displayPath(project.rootPath) : id;
    };
    const requested = value === undefined ? [] : [...new Set([...value.requiredPermissions, ...value.optionalPermissions])];
    const blocked = !canWrite || writing || loading || writeUnknown;
    return <section className="extension-permissions" aria-label={t("Extension permissions")}>
        {!canRead ? <div>{t("Permission details are unavailable from this Core.")}</div> : loading && value === undefined ? <LoadingState title={t("Loading…")} /> : null}
        {error === undefined ? null : <div role="alert" data-notification-error={error.code}><p>{t(requestErrorMessage(error.code))}</p><code>{error.code}</code>{error.diagnosticId === undefined ? null : <p><code>{error.diagnosticId}</code></p>}{!writeUnknown ? null : <p>{t("The permission change may have completed. Refresh permissions before trying again.")}</p>}<LoadingButton disabled={writing} loading={refreshFeedback.busy} onClick={manualReload} type="button" variant="text">{t("Refresh")}</LoadingButton></div>}
        {value === undefined ? null : requested.length === 0 ? <p>{t("No permissions requested")}</p> : <div className="extension-permission-options">
            {requested.map((permission) => {
                const name = permissionName(permission, t);
                const grants = value.grants.filter((grant) => grant.permission === permission);
                const projectPermission = permission === "projects.read";
                const known = editablePermission(permission);
                const kind = permission === "background.run" ? "Extension" : "Global";
                const resourceId = permission === "background.run" ? extensionId : "*";
                const allowed = !projectPermission && grants.some((grant) => grant.resourceKind === kind && grant.resourceId === resourceId);
                const selectedAllowed = grants.some((grant) => grant.resourceKind === "Project" && grant.resourceId === projectId);
                return <section className="extension-permission-option" role="group" aria-label={name} key={permission}>
                    <div className="extension-permission-heading">
                        <h3>{name}</h3>
                        <div className="extension-permission-controls">
                            <span className="extension-permission-state">{projectPermission && grants.length > 0 ? t("Allowed projects: {count}", { count: grants.length }) : t(allowed ? "Allowed" : "Not allowed")}</span>
                            {!known || projectPermission ? null : <Button aria-label={t(allowed ? "Revoke {permission}" : "Allow {permission}", { permission: name })} disabled={blocked} onClick={() => void write({ permission, resourceKind: kind, resourceId }, allowed)} type="button" variant={allowed ? "text" : "tonal"}>{t(allowed ? "Revoke" : "Allow")}</Button>}
                        </div>
                    </div>
                    <p className="extension-permission-description">{permissionDescription(permission, t)}</p>
                    {!value.requiredPermissions.includes(permission) ? null : <p className="extension-permission-required">{t("Required to enable")}</p>}
                    {!known ? null : projectPermission ? <>
                        {grants.length === 0 ? null : <ul className="extension-grants">{grants.map((grant) => <li key={`${grant.resourceKind}:${grant.resourceId}`}>
                            <span title={projectPath(grant.resourceId)}>{projectName(grant.resourceId)}</span>
                            <Button aria-label={t("Revoke access to {project}", { project: projectName(grant.resourceId) })} disabled={blocked} onClick={() => void write(grant, true)} type="button" variant="text">{t("Revoke")}</Button>
                        </li>)}</ul>}
                        <div className="extension-project-permission">
                            <Select disabled={!canReadProjects || projectsLoading || blocked} label={t("Project")} value={projectId} onChange={(next) => { setProjectId(next); setError(undefined); }} options={[{ value: "", label: t("Select project") }, ...projects.flatMap((project) => project.projectId === undefined ? [] : [{ value: project.projectId, label: project.displayName || displayPath(project.rootPath), title: displayPath(project.rootPath) }])]} />
                            <Button disabled={blocked || projectsLoading || selectedAllowed || !projects.some((project) => project.projectId === projectId)} onClick={() => void write({ permission: "projects.read", resourceKind: "Project", resourceId: projectId }, false)} type="button" variant="tonal">{t("Allow selected project")}</Button>
                        </div>
                        {!projectId ? null : <p className="extension-permission-path" title={projectPath(projectId)}>{projectPath(projectId)}</p>}
                        {projectsLoading ? <LoadingState title={t("Loading…")} /> : null}
                        {projectsError === undefined ? null : <div role="alert" data-notification-error={projectsError.code}><p>{t(requestErrorMessage(projectsError.code))}</p><code>{projectsError.code}</code>{projectsError.diagnosticId === undefined ? null : <p><code>{projectsError.diagnosticId}</code></p>}<LoadingButton disabled={writing} loading={refreshFeedback.busy} onClick={manualReload} type="button" variant="text">{t("Refresh")}</LoadingButton></div>}
                    </> : null}
                </section>;
            })}
        </div>}
        <div className="dialog-actions"><DialogCloseButton disabled={writing} variant="text">{t("Close")}</DialogCloseButton>{!canRead ? null : <LoadingButton disabled={writing} loading={refreshFeedback.busy} onClick={manualReload} type="button" variant="text">{t("Refresh")}</LoadingButton>}</div>
    </section>;

}

function requestError(caught: unknown): RpcError {
    if (typeof caught === "object" && caught !== null && "code" in caught && typeof caught.code === "string") {
        return { code: caught.code, message: "", ...("diagnosticId" in caught && typeof caught.diagnosticId === "string" ? { diagnosticId: caught.diagnosticId } : {}) };
    }
    return { code: "internal_error", message: "" };
}
