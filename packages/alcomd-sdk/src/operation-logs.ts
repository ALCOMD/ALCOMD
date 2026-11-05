export interface LogsReadParams {
    beforeSequence?: number;
    afterSequence?: number;
    limit?: number;
}

export interface OperationLogEntry {
    sequence: number;
    entryId: string;
    executionId: string;
    actor: string;
    operationId?: string;
    occurredAtMs: number;
    action: string;
    step: string;
    outcome: "started" | "succeeded" | "failed" | "cancelled" | "waiting";
    target?: { kind: string; id?: string; name?: string; path?: string };
    details?: string;
}

export interface LogsReadResult {
    entries: OperationLogEntry[];
    highWatermark: number;
    hasMore: boolean;
    nextSequence?: number;
}
