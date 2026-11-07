export interface LogsReadParams {
    beforeSequence?: number;
    afterSequence?: number;
    limit?: number;
    filter?: LogFilter;
}

export type LogOutcome = "started" | "succeeded" | "failed" | "cancelled" | "waiting";
export type OperationLogType = "read" | "write" | "passive" | "open" | "maintenance";

export interface LogFilter {
    search?: string;
    actor?: string;
    outcome?: LogOutcome;
    type?: OperationLogType;
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
    outcome: LogOutcome;
    target?: { kind: string; id?: string; name?: string; path?: string };
    details?: string;
}

export interface LogsReadResult {
    entries: OperationLogEntry[];
    actors: string[];
    highWatermark: number;
    hasMore: boolean;
    nextSequence?: number;
}
