export const RPC_VERSION = 1 as const;

/** Disabled package identities from one repository, across all versions. */
export interface RepositoryPackageSettings {
    repositoryId: string;
    packageIds: string[];
}

/** Config13 package source preferences. Core normalizes and validates writes. */
export interface PackageSettings {
    showPrerelease: boolean;
    preferPrerelease: boolean;
    hiddenRepositoryIds: string[];
    hiddenRepositoryPackages: RepositoryPackageSettings[];
    hideLocalUserPackages: boolean;
}

/** Frozen dependency conflicts for a reviewed removal-only package plan. */
export interface PackageRemovalConflict {
    packageId: string;
    requiredBy: { packageId: string; range: string }[];
}

export const RPC_CAPABILITIES = {
    eventsLive: "events.live.v1",
    logsRead: "logs.read.v1",
    repositoriesListFiles: "repositories.list-files.v1",
    repositoriesListImport: "repositories.list-import.v1",
    unityHubOpen: "unity.hub.open.v2",
    packagesCandidates: "packages.candidates.v2",
    extensionsRemoteInstall: "extensions.remote-install.v1",
    extensionsLifecycle: "extensions.lifecycle.v1",
    extensionsPermissions: "extensions.permissions.v1",
    extensionsPermissionsRead: "extensions.permissions.read.v1",
    mcpRuntimeRead: "mcp.runtime.read.v1"
} as const;

export * from "./portable-ui.js";
export * from "./mcp-runtime.js";
export * from "./events.js";
export * from "./operation-logs.js";
export * from "./package-candidates.js";
export * from "./project-inventory.js";
export * from "./unity-hub.js";
export * from "./repository-list.js";
export * from "./templates.js";

export interface ClientInfo {
    name: string;
    version: string;
    instanceId: string;
}

export interface ExtensionPermissionGrant {
    permission: "background.run" | "projects.read" | "mcp.clients.read" | "mcp.clients.manage" | "mcp.connections.read" | "mcp.requests.read" | "mcp.subscription-streams.read" | "mcp.tools.provide" | "repositories.read" | "packages.read" | "packages.manage" | "operations.read" | "operations.cancel" | "unity.sessions.read" | "local-ipc.connect";
    resourceKind: "Extension" | "Global" | "LocalIpcTarget";
    resourceId: string;
}

export interface ExtensionPermissionsListParams {
    extensionId: string;
    cursor?: string;
    limit?: number;
}

export interface ExtensionInstallationTerms { title: string; text: string }
export interface ExtensionInstallConsent {
    planFingerprint: string;
    installationTermsAccepted: boolean;
    requiredPermissionsAccepted: boolean;
}

export interface ExtensionPermissionsListResult {
    availablePermissions: ExtensionPermissionGrant["permission"][];
    extensionId: string;
    grantRevision: number;
    requiredPermissions: string[];
    optionalPermissions: string[];
    grants: ExtensionPermissionGrant[];
    nextCursor?: string;
}

export interface HelloRequest {
    rpcVersion: typeof RPC_VERSION;
    client: ClientInfo;
    capabilities: string[];
}

export interface HelloResponse {
    rpcVersion: typeof RPC_VERSION;
    daemonVersion: string;
    capabilities: string[];
    dataSchema?: number;
    configSchema?: number;
    extensionApi?: {
        major: number;
        world: string;
    };
}

export * from "./extension-sources.js";

export type * from "./core-models.js";
export { SDK_METHODS, type NativeRpcMethods, type BackupCreateParams, type BackupRestoreParams } from "./sdk-methods.js";
