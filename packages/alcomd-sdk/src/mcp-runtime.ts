/** Read-only Core IPC/call metadata; not HTTP sockets, sessions or MCP queues. */
export interface McpRuntimeSnapshotParams { clientId?: string; limit?: number }
export interface McpRuntimeConnection { connectionId: string; clientId: string; connectedAtMs: number; activeCallCount: number }
export type McpObservedCoreMethod = "mcp.tools.catalog" | "extensions.mcpTools.call" | "system.status" | "projects.get" | "projects.list" | "state.check" | "operations.get" | "operations.list" | "operations.cancel" | "events.list" | "packages.planRemove" | "packages.applyPlan";
export interface McpRuntimeCall { callId: string; connectionId: string; clientId: string; method: McpObservedCoreMethod; startedAtMs: number }
export interface McpRuntimeRows<T> { items: T[]; totalTracked: number; truncated: boolean }
export interface McpRuntimeSnapshot {
    instanceId: string;
    observedAtMs: number;
    canReadConnections: boolean;
    canReadRequests: boolean;
    coverageIncomplete: boolean;
    connections?: McpRuntimeRows<McpRuntimeConnection>;
    requests?: McpRuntimeRows<McpRuntimeCall>;
}
