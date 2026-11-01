/** Approved bounded text-list exchange; registration uses the existing repository authority. */
export interface RepositoryListEntry {
    line: number;
    status: "candidate" | "duplicate" | "invalid" | "unsupported";
    source?: { kind: "remote"; url: string };
    reason?: string;
}
export interface RepositoryListInspection { entries: RepositoryListEntry[]; ignoredLines: number }
export interface RepositoryListPreview {
    exportableCount: number;
    excluded: { repositoryId: string; reason: "local_source" | "authentication_not_supported" | "unsupported_source" }[];
    byteCount: number;
    fingerprint: string;
}
export interface RepositoryListExportResult { exportedCount: number; byteCount: number; sha256: string }
