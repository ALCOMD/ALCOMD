import type { ExtensionPermissionGrant, ExtensionRecord } from "@alcomd/sdk";
import type { ExtensionPlan, Operation } from "../../src/core-models";
import type { GuiRpcClient } from "../../src/rpc";

export const H4_ALPHA = "com.example.alpha";
export const H4_BETA = "com.example.beta";
export const H4_GAMMA = "com.example.gamma";
export const H4_ADDED = "com.example.added";

interface ManagementRequest { method: string; params: Record<string, unknown> }
interface PendingManagementRequest { method: string; resolve(): void; reject(code: string): void }
type ManagementGrant = ExtensionPermissionGrant;
interface ManagementFixture {
    requests: ManagementRequest[];
    pending: PendingManagementRequest[];
    lastOperationId?: string;
    failNext(method: string, code: string): void;
    hold(method: string, held: boolean): void;
    release(method: string, code?: string): boolean;
    complete(state: "succeeded" | "failed" | "cancelled", errorCode?: string): void;
    setGrants(extensionId: string, values: ManagementGrant[]): void;
}

declare global { interface Window { h4Management: ManagementFixture } }

/** Opt-in public DTO fixture. It never reads or writes the native Core environment. */
export function withH4ManagementClient(base: GuiRpcClient, query: URLSearchParams): GuiRpcClient {
    const hash = "a".repeat(64);
    const extensions = new Map<string, ExtensionRecord>();
    for (const id of [H4_ALPHA, H4_BETA, H4_GAMMA]) extensions.set(id, record(id));
    if (query.has("h4-quarantined")) {
        const alpha = extensions.get(H4_ALPHA)!;
        alpha.quarantineState = "quarantined";
        alpha.runtimeState = "stopped";
    }
    const plans = new Map<string, ExtensionPlan>();
    const grants = new Map<string, ManagementGrant[]>();
    for (const id of extensions.keys()) grants.set(id, [
        { permission: "background.run", resourceKind: "Extension", resourceId: id },
        { permission: "projects.read", resourceKind: "Project", resourceId: "00000000-0000-4000-8000-000000000101" }
    ]);
    const tasks = new Map<string, { operation: Operation; plan: ExtensionPlan }>();
    const held = new Set<string>();
    let failure: { method: string; code: string } | undefined;
    let nextPlan = 0;
    let nextOperation = 0;
    const fixture: ManagementFixture = {
        requests: [], pending: [],
        failNext(method, code) { failure = { method, code }; },
        hold(method, value) { if (value) held.add(method); else held.delete(method); },
        release(method, code) {
            const index = fixture.pending.findIndex((request) => request.method === method);
            if (index < 0) return false;
            const request = fixture.pending.splice(index, 1)[0]!;
            if (code === undefined) request.resolve(); else request.reject(code);
            return true;
        },
        setGrants(extensionId, values) {
            const extension = installed(extensionId);
            grants.set(extensionId, structuredClone(values));
            extension.grantRevision += 1;
        },
        complete(state, errorCode) {
            const task = fixture.lastOperationId === undefined ? undefined : tasks.get(fixture.lastOperationId);
            if (task === undefined) throw new Error("No accepted fixture operation");
            task.operation = { ...task.operation, state, revision: task.operation.revision + 1, updatedAtMs: 1_700_000_002_000, completedAtMs: 1_700_000_002_000,
                ...(errorCode === undefined ? {} : { errorCode }) };
            if (state === "succeeded") {
                if (task.plan.action === "install") extensions.set(task.plan.extensionId, { ...record(task.plan.extensionId), desiredState: "installed_disabled", runtimeState: "stopped" });
                else extensions.delete(task.plan.extensionId);
            }
        }
    };
    window.h4Management = fixture;

    function record(extensionId: string): ExtensionRecord {
        return { extensionId, version: "1.0.0", apiMajor: 1, packageDigest: hash, publisherFingerprint: `ed25519-sha256:${hash}`,
            trustDecision: "user_approved_for_extension", desiredState: "enabled", quarantineState: "clear", runtimeState: "running",
            grantRevision: 3, lifecycleGeneration: 2, revision: 4, ui: { protocol: "portable-v1" } };
    }
    function log(method: string, params: Record<string, unknown>) { fixture.requests.push({ method, params: structuredClone(params) }); }
    function response<T>(method: string, value: T): Promise<T> {
        if (failure?.method === method) {
            const code = failure.code; failure = undefined;
            return Promise.reject({ code, message: "Fixture request refused.", diagnosticId: "h4-management-diagnostic" });
        }
        if (!held.has(method)) return Promise.resolve(value);
        return new Promise((resolve, reject) => fixture.pending.push({ method, resolve: () => resolve(value), reject: (code) => reject({ code, message: "Fixture request refused." }) }));
    }
    function installed(extensionId: string): ExtensionRecord {
        const extension = extensions.get(extensionId);
        if (extension === undefined) throw { code: "extension_not_installed", message: "Extension is not installed." };
        return extension;
    }
    function plan(action: "install" | "uninstall", extensionId: string, dataDisposition: string): ExtensionPlan {
        const result: ExtensionPlan = { planId: `00000000-0000-4000-8000-${String(800 + ++nextPlan).padStart(12, "0")}`, action,
            state: "unapplied", sourceKind: action === "install" ? "local_owner_selected" : "not_applicable", extensionId,
            version: "1.0.0", apiMajor: 1, profileVersion: 1, packageDigest: hash, publisherFingerprint: `ed25519-sha256:${hash}`,
            trustDecision: "user_approved_for_extension", dataDisposition, planFingerprint: hash, uiProtocol: "portable-v1",
            requiredPermissions: ["projects.read"], optionalPermissions: ["background.run"] };
        plans.set(result.planId, result);
        return structuredClone(result);
    }
    function accept(planId: string, action: "install" | "uninstall") {
        const reviewed = plans.get(planId);
        if (reviewed === undefined || reviewed.action !== action || reviewed.state !== "unapplied") throw { code: "revision_conflict" };
        reviewed.state = "applied";
        const operationId = `00000000-0000-4000-8000-${String(900 + ++nextOperation).padStart(12, "0")}`;
        tasks.set(operationId, { plan: reviewed, operation: { operationId, kind: `extensions.${action}`, state: "running", revision: 1,
            createdAtMs: 1_700_000_000_000, updatedAtMs: 1_700_000_001_000, progress: { phase: action === "install" ? "source_verified" : "grants_revoked" } } });
        fixture.lastOperationId = operationId;
        if (action === "uninstall") {
            const current = installed(reviewed.extensionId);
            grants.set(reviewed.extensionId, []);
            extensions.set(reviewed.extensionId, { ...current, desiredState: "uninstalling", runtimeState: "stopped", grantRevision: current.grantRevision + 1 });
        }
        return { operationId, replayed: false };
    }

    const methods: Partial<GuiRpcClient> = {
        async systemStatus() {
            const status = await base.systemStatus();
            return { ...status, capabilities: [...new Set([...status.capabilities, "extensions.permissions.read.v1"])] };
        },
        async projectsList(cursor) {
            log("projects.list", cursor === undefined ? {} : { cursor });
            return response("projects.list", await base.projectsList(cursor));
        },
        extensionsList(cursor) {
            log("extensions.list", cursor === undefined ? {} : { cursor });
            const last = cursor?.startsWith("h4-after:") ? cursor.slice("h4-after:".length) : undefined;
            const available = [...extensions.values()].sort((left, right) => left.extensionId.localeCompare(right.extensionId))
                .filter((extension) => last === undefined || extension.extensionId > last);
            const page = available.slice(0, 1);
            return response("extensions.list", { extensions: structuredClone(page), ...(available.length <= page.length ? {} : { nextCursor: `h4-after:${page[0]!.extensionId}` }) });
        },
        async extensionGet(extensionId) {
            log("extensions.get", { extensionId });
            return response("extensions.get", { extension: structuredClone(installed(extensionId)) });
        },
        async extensionPermissionsList(extensionId, cursor) {
            log("extensions.permissions.list", { extensionId, ...(cursor === undefined ? {} : { cursor }) });
            const extension = installed(extensionId);
            const parts = cursor?.split(":");
            if (parts !== undefined && (parts[0] !== extensionId || Number(parts[1]) !== extension.grantRevision)) throw { code: "revision_conflict" };
            const offset = parts === undefined ? 0 : Number(parts[2]);
            const all = grants.get(extensionId) ?? [];
            const page = all.slice(offset, offset + 1);
            return response("extensions.permissions.list", { extensionId, grantRevision: extension.grantRevision,
                requiredPermissions: ["projects.read"], optionalPermissions: ["background.run"], grants: structuredClone(page),
                ...(offset + page.length < all.length ? { nextCursor: `${extensionId}:${extension.grantRevision}:${offset + page.length}` } : {}) });
        },
        async extensionPlanInstall(packagePath, expectedRevision, publisherApproval) {
            log("extensions.planInstall", { packagePath, expectedRevision, publisherApproval });
            if (expectedRevision !== 0) throw { code: "revision_conflict" };
            if (publisherApproval !== "approve_for_extension") throw { code: "extension_publisher_confirmation_required" };
            if (extensions.has(H4_ADDED)) throw { code: "extension_already_installed" };
            return response("extensions.planInstall", { plan: plan("install", H4_ADDED, "not_applicable") });
        },
        async extensionApplyInstall(planId) {
            log("extensions.applyInstall", { planId });
            // A lost reply can follow actual acceptance; the UI must not retry it.
            return response("extensions.applyInstall", accept(planId, "install"));
        },
        async extensionEnable(extensionId, expectedRevision) {
            log("extensions.enable", { extensionId, expectedRevision });
            await response("extensions.enable", undefined);
            const extension = installed(extensionId);
            if (extension.revision !== expectedRevision) throw { code: "revision_conflict" };
            const updated: ExtensionRecord = { ...extension, desiredState: "enabled", quarantineState: "clear", runtimeState: "running", revision: extension.revision + 1 };
            extensions.set(extensionId, updated);
            return { extension: structuredClone(updated) };
        },
        async extensionDisable(extensionId, expectedRevision) {
            log("extensions.disable", { extensionId, expectedRevision });
            await response("extensions.disable", undefined);
            const extension = installed(extensionId);
            if (extension.revision !== expectedRevision) throw { code: "revision_conflict" };
            const updated: ExtensionRecord = { ...extension, desiredState: "installed_disabled", runtimeState: "stopped", revision: extension.revision + 1 };
            extensions.set(extensionId, updated);
            return { extension: structuredClone(updated) };
        },
        async extensionPlanUninstall(extensionId, expectedRevision, dataDisposition) {
            log("extensions.planUninstall", { extensionId, expectedRevision, dataDisposition });
            if (installed(extensionId).revision !== expectedRevision) throw { code: "revision_conflict" };
            return response("extensions.planUninstall", { plan: plan("uninstall", extensionId, dataDisposition) });
        },
        async extensionApplyUninstall(planId) {
            log("extensions.applyUninstall", { planId });
            return response("extensions.applyUninstall", accept(planId, "uninstall"));
        },
        async operationGet(operationId) {
            log("operations.get", { operationId });
            const task = tasks.get(operationId);
            return task === undefined ? base.operationGet(operationId) : response("operations.get", structuredClone(task.operation));
        },
        async extensionSetGrant(extensionId, permission, resourceKind, resourceId, expectedGrantRevision) {
            log("extensions.setGrant", { extensionId, permission, resourceKind, resourceId, expectedGrantRevision });
            await response("extensions.setGrant", undefined);
            const extension = installed(extensionId);
            if (extension.grantRevision !== expectedGrantRevision) throw { code: "revision_conflict" };
            if ((permission !== "background.run" && permission !== "projects.read") || (resourceKind !== "Extension" && resourceKind !== "Project")) throw { code: "invalid_request" };
            if (permission === "background.run" ? resourceKind !== "Extension" || resourceId !== extensionId : resourceKind !== "Project") throw { code: "extension_scope_denied" };
            const current = grants.get(extensionId) ?? [];
            if (!current.some((grant) => grant.permission === permission && grant.resourceKind === resourceKind && grant.resourceId === resourceId)) current.push({ permission, resourceKind, resourceId });
            grants.set(extensionId, current);
            extension.grantRevision += 1;
            return { extensionId, grantRevision: extension.grantRevision, state: "granted", replayed: false };
        },
        async extensionRevokeGrant(extensionId, permission, resourceKind, resourceId, expectedGrantRevision) {
            log("extensions.revokeGrant", { extensionId, permission, resourceKind, resourceId, expectedGrantRevision });
            await response("extensions.revokeGrant", undefined);
            const extension = installed(extensionId);
            if (extension.grantRevision !== expectedGrantRevision) throw { code: "revision_conflict" };
            grants.set(extensionId, (grants.get(extensionId) ?? []).filter((grant) => grant.permission !== permission || grant.resourceKind !== resourceKind || grant.resourceId !== resourceId));
            extension.grantRevision += 1;
            return { extensionId, grantRevision: extension.grantRevision, state: "revoked", replayed: false };
        }
    };
    return new Proxy(base, {
        get(target, property, receiver) {
            if (property in methods) return methods[property as keyof GuiRpcClient];
            const value: unknown = Reflect.get(target, property, receiver);
            return typeof value === "function" ? value.bind(target) : value;
        }
    });
}
