export const RPC_VERSION = 1 as const;

export const RPC_CAPABILITIES = {
    diagnosticsLog: "diagnostics.log.v1",
    eventsLive: "events.live.v1",
    repositoriesListFiles: "repositories.list-files.v1",
    unityHubOpen: "unity.hub.open.v1",
    packagesCandidates: "packages.candidates.v1",
    extensionsLifecycle: "extensions.lifecycle.v1",
    extensionsPermissions: "extensions.permissions.v1",
    extensionsPermissionsRead: "extensions.permissions.read.v1"
} as const;

export * from "./portable-ui";
export * from "./diagnostic-log";
export * from "./events";
export * from "./package-candidates";
export * from "./unity-hub";
export * from "./repository-list";

export interface ClientInfo {
    name: string;
    version: string;
    instanceId: string;
}

export interface ExtensionPermissionGrant {
    permission: "background.run" | "projects.read";
    resourceKind: "Extension" | "Project";
    resourceId: string;
}

export interface ExtensionPermissionsListParams {
    extensionId: string;
    cursor?: string;
    limit?: number;
}

export interface ExtensionPermissionsListResult {
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
