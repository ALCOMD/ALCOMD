export const RPC_VERSION = 1 as const;

export interface ClientInfo {
    name: string;
    version: string;
    instanceId: string;
}

export interface HelloRequest {
    rpcVersion: typeof RPC_VERSION;
    client: ClientInfo;
    capabilities: string[];
}

export interface HelloResponse {
    rpcVersion: typeof RPC_VERSION;
    daemonVersion: string;
    dataSchema: number;
    configSchema: number;
    extensionApi: number;
}
