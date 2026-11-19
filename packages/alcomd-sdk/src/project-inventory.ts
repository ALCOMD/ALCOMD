export interface ProjectInventoryParams {
    projectId: string;
    expectedRevision: number;
    cursor?: string;
    limit?: number;
}

export interface ProjectInventoryEntry {
    packageId: string;
    declaredRange: string | null;
    lockedVersion: string | null;
    observedVersion: string | null;
    state: "matched" | "missing" | "version_mismatch" | "unknown";
}

export interface ProjectInventoryResult {
    entries: ProjectInventoryEntry[];
    resolution: "needed" | "no_issue_observed" | "unknown";
    reason: string;
    nextCursor: string | null;
}
