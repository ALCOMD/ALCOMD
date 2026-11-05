/** Technical entries retain tracing-subscriber JSON without rewriting their body. */
export type DiagnosticLogLevel = "TRACE" | "DEBUG" | "INFO" | "WARN" | "ERROR";
export interface DiagnosticLogQuery { search?: string; levels?: DiagnosticLogLevel[]; target?: string; diagnosticId?: string }
export interface TracingLogRecord {
    timestamp?: string;
    level?: DiagnosticLogLevel;
    target?: string;
    fields?: Record<string, unknown>;
    spans?: Array<Record<string, unknown>>;
    [key: string]: unknown;
}
export interface DiagnosticLogItem {
    sourceId: string;
    sessionId: string;
    segmentId: string;
    offset: number;
    record: TracingLogRecord;
}
export interface DiagnosticLogHealth {
    sourceId: string;
    sessionId: string;
    revision: number;
    state: "ready" | "degraded" | "closed";
    queueDropped: number;
    oversizeRejected: number;
    writeFailed: number;
    rotationFailed: number;
    flushFailed: number;
    hasGap: boolean;
    lastFailure?: { kind: string; message: string };
}
export interface DiagnosticLogIssue { kind: string; segmentId?: string; offset?: number }
export interface DiagnosticLogReadParams { cursor?: string; limit?: number; follow?: boolean; query?: DiagnosticLogQuery }
export interface DiagnosticLogHistoryParams { cursor?: string; before?: string; limit?: number; query?: DiagnosticLogQuery }
export interface DiagnosticLogReadResult {
    items: DiagnosticLogItem[];
    nextCursor: string;
    baseline: boolean;
    health: DiagnosticLogHealth;
    issues: DiagnosticLogIssue[];
    hasMore: boolean;
    scannedBytes: number;
}
export interface DiagnosticLogHistoryResult {
    items: DiagnosticLogItem[];
    nextCursor?: string;
    health: DiagnosticLogHealth;
    issues: DiagnosticLogIssue[];
    hasMore: boolean;
    scannedBytes: number;
}
export interface FrontendException { name: string; message: string; stack?: string; location?: string }
