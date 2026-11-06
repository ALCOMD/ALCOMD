/** Approved bounded text-list exchange; registration uses the existing repository authority. */
export interface RepositoryListEntry {
    credentialsConfigured?: boolean;
    line: number;
    status: "candidate" | "duplicate" | "invalid" | "unsupported";
    source?: { kind: "remote"; url: string };
    reason?: string;
}

/** Write-only repository request headers. */
export interface RepositoryCredentials {
    headers: Record<string, string>;
}
export interface RepositorySetDisplayNameParams {
    repositoryId: string;
    expectedRevision: number;
    idempotencyKey: string;
    displayName: string | null;
}
export interface RepositoryListInspection { entries: RepositoryListEntry[]; ignoredLines: number; fingerprint: string }
export interface RepositoryListImportParams { path: string; expectedFingerprint: string; idempotencyKey: string }
export type RepositoryListImportItem =
    | { line: number; status: "added" | "existing" | "unprocessed" }
    | { line: number; status: "failed" | "unknown"; code: string };
export interface RepositoryListImportResult { items: RepositoryListImportItem[] }
export interface RepositoryListCancelParams { idempotencyKey: string }
export interface RepositoryListCancelResult { stopped: boolean }
export interface RepositoryRefreshAllParams { idempotencyKey: string }
export type RepositoryRefreshAllItem =
    | { repositoryId: string; status: "refreshed" }
    | { repositoryId: string; status: "failed"; code: string };
export interface RepositoryRefreshAllResult { items: RepositoryRefreshAllItem[] }
export interface RepositoryListPreview {
    exportableCount: number;
    excluded: { repositoryId: string; reason: "local_source" | "authentication_not_supported" | "unsupported_source" }[];
    byteCount: number;
    fingerprint: string;
}
export interface RepositoryListExportResult { exportedCount: number; byteCount: number; sha256: string }
