export type RuntimeParams = {
    clientId?: RuntimeId | null;
    limit?: number | null;
};

export type RuntimeId = string;

export type RuntimeSnapshot = {
    instanceId: RuntimeId;
    observedAtMs: RuntimeTime;
    canReadConnections: boolean;
    canReadRequests: boolean;
    coverageIncomplete: boolean;
    connections?: RuntimeConnectionRows;
    requests?: RuntimeCallRows;
};

export type RuntimeCallRows = {
    items: Array<RuntimeCall>;
    totalTracked: number;
    truncated: boolean;
};

export type RuntimeCall = {
    callId: RuntimeId;
    connectionId: RuntimeId;
    clientId: RuntimeId;
    method: "mcp.tools.catalog" | "extensions.mcpTools.call" | "system.status" | "projects.get" | "projects.list" | "state.check" | "operations.get" | "operations.list" | "operations.cancel" | "events.list" | "packages.planRemove" | "packages.applyPlan";
    startedAtMs: RuntimeTime;
};

export type RuntimeConnectionRows = {
    items: Array<RuntimeConnection>;
    totalTracked: number;
    truncated: boolean;
};

export type RuntimeConnection = {
    connectionId: RuntimeId;
    clientId: RuntimeId;
    connectedAtMs: RuntimeTime;
    activeCallCount: number;
};

export type RuntimeTime = number;

export type TransportQuery = {
    clientId?: string | null;
    limit?: number | null;
};

export type TransportSnapshot = {
    instanceId: string;
    observedAtMs: number;
    status: "available" | "stale" | "unavailable";
    canReadConnections: boolean;
    canReadRequests: boolean;
    canReadSubscriptionStreams: boolean;
    coverageIncomplete: boolean;
    observers: Array<TransportObserver>;
    connections?: TransportConnectionPage;
    requests?: TransportRequestPage;
    subscriptionStreams?: TransportStreamPage;
};

export type TransportConnectionPage = {
    items: Array<TransportConnection>;
    totalTracked: number;
    truncated: boolean;
};

export type TransportConnection = {
    connectionId: string;
    observerId: string;
    source: "adapterReported";
    firstObservedAtMs: number;
    stage: "accepted" | "tls" | "idle" | "active";
};

export type TransportObserver = {
    observerId: string;
    transport: "stdio" | "http" | "https-oauth";
    lastReportAtMs: number;
    status: "available" | "stale" | "unavailable";
};

export type TransportRequestPage = {
    items: Array<TransportRequest>;
    totalTracked: number;
    truncated: boolean;
};

export type TransportRequest = {
    requestId: string;
    connectionId: string;
    observerId: string;
    source: "adapterReported";
    firstObservedAtMs: number;
    method: "server/discover" | "tools/list" | "tools/call" | "subscriptions/listen";
    stage: "receiving" | "executing" | "streaming";
    clientId: string | null;
};

export type TransportStreamPage = {
    items: Array<TransportStream>;
    totalTracked: number;
    truncated: boolean;
};

export type TransportStream = {
    streamId: string;
    requestId: string;
    observerId: string;
    source: "adapterReported";
    firstObservedAtMs: number;
    toolsListChanged: true;
    clientId: string | null;
};
