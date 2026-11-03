export type DiagnosticLogLevel = "trace" | "debug" | "info" | "warning" | "error";

export interface DiagnosticLogCursor {
    instanceId: string;
    sequence: number;
}

export interface DiagnosticLogReadParams {
    cursor?: DiagnosticLogCursor;
    limit?: number;
    waitMs?: number;
}

export interface DiagnosticLogItem {
    instanceId: string;
    sequence: number;
    occurredAtMs: number;
    level: DiagnosticLogLevel;
    code: string;
    diagnosticId?: string;
    notify: boolean;
}

export interface DiagnosticLogReadResult {
    items: DiagnosticLogItem[];
    nextCursor: DiagnosticLogCursor;
    reset: boolean;
    gap: boolean;
}
