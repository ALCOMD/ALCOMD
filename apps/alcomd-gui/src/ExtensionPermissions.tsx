import { useGuiWrite } from "./gui-write-attempt";
import { withActionNotification } from "./app-notifications";
import { requestErrorMessage } from "./request-errors";
import { useEffect, useRef, useState } from "react";
import type { ExtensionPermissionGrant, ExtensionPermissionsListResult, RpcError } from "@alcomd/sdk";
import type { GuiRpcClient } from "./rpc";
import { Button, DialogCloseButton, LoadingButton, useActionFeedback, Switch } from "./Material";
import { LoadingState } from "./LoadingState";
import { capabilities, useCapability } from "./capabilities";
import { useTranslation, type Translate } from "./i18n";
import { useCoreEvents, useCoreInvalidation } from "./core-events-ui";

export function hasPermissionRequests(value: { requiredPermissions?: unknown; optionalPermissions?: unknown }): boolean {
    return [value.requiredPermissions, value.optionalPermissions].every((items) => Array.isArray(items)
        && items.length <= 64 && items.every((item: unknown) => typeof item === "string" && item.length > 0 && item.length <= 128));
}

export function permissionName(permission: string, t: Translate): string {
    if (permission === "background.run") return t("Run in background");
    if (permission === "projects.read") return t("Read projects");
    if (permission === "unity.sessions.read") return t("Read active Unity sessions");
    if (permission === "local-ipc.connect") return t("Connect to local applications");
    if (permission === "mcp.clients.read") return t("Read MCP clients");
    if (permission === "mcp.clients.manage") return t("Manage MCP clients");
    if (permission === "mcp.connections.read") return t("Read MCP Core connections");
    if (permission === "mcp.subscription-streams.read") return t("Read MCP subscription streams");
    if (permission === "mcp.requests.read") return t("Read MCP Core calls");
    if (permission === "mcp.tools.provide") return t("Publish MCP tools");
    if (permission === "repositories.read") return t("Read repositories");
    if (permission === "packages.read") return t("Read package information");
    if (permission === "packages.manage") return t("Change project packages");
    if (permission === "operations.read") return t("Read operation results");
    if (permission === "operations.cancel") return t("Cancel operations");
    return t("Unknown permission: {permission}", { permission });
}

function permissionDescription(permission: string, t: Translate): string {
    switch (permission) {
        case "background.run": return t("Run background tasks when this extension's page is closed.");
        case "projects.read": return t("View basic information about all current and future projects.");
        case "unity.sessions.read": return t("Observe which registered projects are open in Unity.");
        case "local-ipc.connect": return t("Connect only to the local applications approved when enabling this extension.");
        case "mcp.connections.read": return t("View all MCP clients’ connections to Core.");
        case "mcp.subscription-streams.read": return t("View adapter-reported MCP notification streams.");
        case "mcp.requests.read": return t("View Core calls currently executing for all MCP clients.");
        case "mcp.clients.read": return t("View all registered MCP clients and their grants.");
        case "mcp.clients.manage": return t("Create and permanently revoke MCP clients.");
        case "mcp.tools.provide": return t("Make declared tools available to MCP clients with matching business permissions.");
        case "repositories.read": return t("Read repository and package source information.");
        case "packages.read": return t("Read available packages and prepare package change plans.");
        case "packages.manage": return t("Apply explicitly confirmed package change plans to current and future projects.");
        case "operations.read": return t("Read the invoking client’s operation status and results.");
        case "operations.cancel": return t("Request cancellation of the invoking client’s operations.");
        default: return t("This app cannot grant this permission.");
    }
}

function editablePermission(permission: string): permission is ExtensionPermissionGrant["permission"] {
    return permission === "background.run" || permission === "projects.read" || permission === "mcp.clients.read" || permission === "mcp.clients.manage" || permission === "mcp.connections.read" || permission === "mcp.requests.read" || permission === "mcp.subscription-streams.read" || permission === "mcp.tools.provide" || permission === "repositories.read" || permission === "packages.read" || permission === "packages.manage" || permission === "operations.read" || permission === "operations.cancel" || permission === "unity.sessions.read" || permission === "local-ipc.connect";
}

export function ExtensionPermissionRequests({ required, optional }: { required: string[]; optional: string[] }) {
    const { t } = useTranslation();
    return <section aria-label={t("Requested permissions")}>
        <h3>{t("Requested permissions")}</h3>
        {required.length === 0 && optional.length === 0 ? <p>{t("No permissions requested")}</p> : <dl className="dialog-summary">
            {required.length === 0 ? null : <div><dt>{t("Required permissions")}</dt><dd>{<ul className="extension-requested-permissions">{required.map((permission) => <li key={permission}><strong>{permissionName(permission, t)}</strong><p>{permissionDescription(permission, t)}</p></li>)}</ul>}</dd></div>}
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
    const writeRequest = useGuiWrite(client, extensionId);
    const canRead = useCapability(capabilities.extensionsPermissionsRead);
    const canWrite = useCapability(capabilities.extensionsPermissions);

    const events = useCoreEvents();
    const readVersion = useRef(0);
    const acceptedRead = useRef<{ client: GuiRpcClient; extensionId: string } | undefined>(undefined);
    const [generation, setGeneration] = useState(0);
    const [value, setValue] = useState<ExtensionPermissionsListResult>();
    const [error, setError] = useState<RpcError>();
    const [loading, setLoading] = useState(true);
    const [writing, setWriting] = useState(false);
    const [writeUnknown, setWriteUnknown] = useState(false);
    const pendingGrant = useRef<{ grant: ExtensionPermissionGrant; revoke: boolean; revision: number } | undefined>(undefined);
    const owner = useRef({ client, extensionId, canRead, generation, active: true, pending: false });
    if (owner.current.client !== client || owner.current.extensionId !== extensionId || owner.current.canRead !== canRead || owner.current.generation !== generation) {
        owner.current.active = false;
        owner.current = { client, extensionId, canRead, generation, active: true, pending: false };
    }
    const scope = owner.current;
    // A new owner renders before its read effect; never expose its predecessor's grants as writable.
    const ownsRead = acceptedRead.current === scope;
    const sameReadIdentity = acceptedRead.current?.client === client && acceptedRead.current.extensionId === extensionId;
    // Keep same-identity rows in place during readback, disabled until the new owner reads.
    const visibleValue = sameReadIdentity && canRead ? value : undefined;
    const visibleError = ownsRead ? error : undefined;
    const current = () => scope.active && owner.current === scope;
    const reload = () => { scope.active = false; setGeneration((version) => version + 1); };
    const refreshFeedback = useActionFeedback(loading);
    const manualReload = () => refreshFeedback.run(async () => {
        setError(undefined);
        setLoading(canRead);
        try {
            await withActionNotification(client, "Refresh", async () => {
                await readPermissions();
            });
            if (current()) { pendingGrant.current = undefined; setWriteUnknown(false); }
        } catch { /* The read functions retain their local, actionable errors. */ }
    });
    const readPermissions = async () => {
        if (!canRead || !current()) return;
        const version = ++readVersion.current;
        setLoading(true);
        try {
            const load = async () => {
                let result = await client.extensionPermissionsList(extensionId);
                if (result.extensionId !== extensionId || !hasPermissionRequests(result) || !Array.isArray(result.availablePermissions) || result.availablePermissions.length > 64 || result.availablePermissions.some((p) => !editablePermission(p))) throw { code: "internal_error" };
                while (result.nextCursor !== undefined) {
                    if (!current() || version !== readVersion.current) break;
                    const page = await client.extensionPermissionsList(extensionId, result.nextCursor);
                    if (page.extensionId !== extensionId || page.grantRevision !== result.grantRevision || JSON.stringify(page.availablePermissions) !== JSON.stringify(result.availablePermissions)) throw { code: "revision_conflict" };
                    result = { ...page, grants: [...result.grants, ...page.grants] };
                }
                return result;
            };
            const result = events ? await events.consistentRead(["extensions"], load) : await load();
            if (current() && version === readVersion.current) { acceptedRead.current = scope; setValue(result); setError(undefined); }
        } catch (caught: unknown) {
            if (current() && version === readVersion.current) { acceptedRead.current = scope; setValue(undefined); setError(requestError(caught)); }
            throw caught;
        } finally { if (current() && version === readVersion.current) setLoading(false); }
    };
    useCoreInvalidation("extension-permissions:" + extensionId, ["extensions"], readPermissions);

    useEffect(() => {
        scope.active = true;
        pendingGrant.current = undefined;
        if (!sameReadIdentity) setValue(undefined);
        setError(undefined); setLoading(true); setWriting(false); setWriteUnknown(false);
        if (canRead) void readPermissions().catch(() => {}); else setLoading(false);
        return () => { scope.active = false; readVersion.current += 1; };
    }, [canRead, client, extensionId, scope]);

    const write = async (grant: ExtensionPermissionGrant, revoke: boolean, revision = value?.grantRevision) => {
        if (!canWrite || !canRead || !current() || !ownsRead || scope.pending || loading || revision === undefined) return;
        scope.pending = true; setWriting(true); setError(undefined);  onBusyChange(true);
        try {
            await (revoke
                ? writeRequest("extensionRevokeGrant", extensionId, grant.permission, grant.resourceKind, grant.resourceId, revision)
                : writeRequest("extensionSetGrant", extensionId, grant.permission, grant.resourceKind, grant.resourceId, revision));
            if (!current()) return;

            onChanged?.(); reload();
        } catch (caught: unknown) {
            if (current()) {
                const nextError = requestError(caught);
                const unknown = nextError.code === "daemon_unavailable" || nextError.code === "internal_error";
                if (unknown || nextError.code === "revision_conflict") setValue(undefined);
                pendingGrant.current = unknown ? { grant, revoke, revision } : undefined;
                setWriteUnknown(unknown); setError(nextError);
            }
        } finally { scope.pending = false; if (owner.current === scope) setWriting(false); onBusyChange(false); }
    };
    const requested = visibleValue === undefined ? [] : [...new Set([...visibleValue.requiredPermissions, ...visibleValue.optionalPermissions])];
    const groups = [
        { title: t("Required permissions"), permissions: visibleValue?.requiredPermissions ?? [] },
        { title: t("Optional permissions"), permissions: visibleValue?.optionalPermissions ?? [] },
        { title: t("Other permissions"), permissions: visibleValue?.availablePermissions.filter((p) => !requested.includes(p)) ?? [] }
    ];
    const blocked = !canWrite || !ownsRead || writing || loading || writeUnknown;
    return <section className="extension-permissions" aria-label={t("Extension permissions")}>
        {!canRead ? <div>{t("Permission details are unavailable from this Core.")}</div> : (loading || !ownsRead) && visibleValue === undefined ? <LoadingState title={t("Loading…")} /> : null}
        {visibleError === undefined ? null : <div role="alert" data-notification-error={visibleError.code}><p>{t(requestErrorMessage(visibleError.code))}</p><code>{visibleError.code}</code>{visibleError.diagnosticId === undefined ? null : <p><code>{visibleError.diagnosticId}</code></p>}{!writeUnknown ? null : <><p>{t("The permission change may have completed.")}</p><Button disabled={!canWrite || writing || loading} onClick={() => { const request = pendingGrant.current; if (request) void write(request.grant, request.revoke, request.revision); }} type="button">{t("Retry")}</Button></>}<LoadingButton disabled={writing} loading={refreshFeedback.busy} onClick={manualReload} type="button" variant="text">{t("Refresh")}</LoadingButton></div>}
        {visibleValue === undefined ? null : groups.map((group) => <section className="extension-permission-group" aria-label={group.title} key={group.title}>
            <h3>{group.title}</h3>
            {group.permissions.length === 0 ? <p>{t("None")}</p> : <div className="extension-permission-options">
            {group.permissions.map((permission) => {
                const name = permissionName(permission, t);
                const grants = visibleValue.grants.filter((grant) => grant.permission === permission);
                const known = editablePermission(permission);
                const scoped = permission === "local-ipc.connect";
                const kind = permission === "background.run" ? "Extension" : "Global";
                const resourceId = permission === "background.run" ? extensionId : "*";
                const allowed = grants.some((grant) => grant.resourceKind === kind && grant.resourceId === resourceId);
                return <section className="extension-permission-option" role="group" aria-label={name} key={permission}>
                    <div className="extension-permission-heading">
                        <h3>{name}</h3>
                        <div className="extension-permission-controls">
                            {scoped ? grants.filter((grant) => grant.resourceKind === "LocalIpcTarget").map((grant) => <LoadingButton key={grant.resourceId} disabled={blocked} onClick={() => void write(grant, true)} type="button" variant="text">{t("Revoke")}</LoadingButton>) : !known ? null : <Switch label={name} disabled={blocked} selected={allowed} onChange={(next) => void write({ permission, resourceKind: kind, resourceId }, !next)} />}
                        </div>
                    </div>
                    <p className="extension-permission-description">{permissionDescription(permission, t)}</p>

                </section>;
            })}
        </div>}
        </section>)}
        <div className="dialog-actions"><DialogCloseButton disabled={writing} variant="text">{t("Close")}</DialogCloseButton>{!canRead ? null : <LoadingButton disabled={writing} loading={refreshFeedback.busy} onClick={manualReload} type="button" variant="text">{t("Refresh")}</LoadingButton>}</div>
    </section>;

}

function requestError(caught: unknown): RpcError {
    if (typeof caught === "object" && caught !== null && "code" in caught && typeof caught.code === "string") {
        return { code: caught.code, message: "", ...("diagnosticId" in caught && typeof caught.diagnosticId === "string" ? { diagnosticId: caught.diagnosticId } : {}) };
    }
    return { code: "internal_error", message: "" };
}

export function ExtensionToolPreview({ tools }: { tools: import("@alcomd/sdk").McpToolPreview[] | undefined }) {
    const { t } = useTranslation();
    if (tools === undefined || tools.length === 0) return null;
    return <section className="extension-tool-preview" aria-label={t("Declared MCP tools")}><h3>{t("Declared MCP tools")}</h3><dl className="dialog-summary">{tools.map((tool) => <div key={tool.name}><dt>{tool.title}</dt><dd><p>{tool.description}</p><p>{t(tool.effect === "pure" || tool.effect === "read" ? "Read or compute" : tool.effect === "plan" ? "Creates a review plan" : tool.effect === "apply" ? "Applies the confirmed plan" : "Requests task cancellation")}</p>{tool.requiredPermissions.length === 0 ? null : <p>{tool.requiredPermissions.map((p) => permissionName(p, t)).join(" · ")}</p>}</dd></div>)}</dl></section>;
}
