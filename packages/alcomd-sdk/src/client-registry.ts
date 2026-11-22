export interface ClientProgramSummary {
    programKey: string;
    programId?: string;
    displayName: string;
    kind: "gui" | "cli" | "mcp" | "unknown";
    installations: { scope: "user" | "machine"; productVersion: string }[];
    activeInstanceCount: number;
    activeConnectionCount: number;
}

export interface ClientInstanceSummary {
    instanceId: string;
    connections: {
        connectionId: string;
        connectedAtMs: number;
        reported: { name: string; version: string; instanceId: string };
    }[];
}
