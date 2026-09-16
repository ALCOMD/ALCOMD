/** Read-only direct candidate evidence. It never authorizes Apply. */
export type CandidateSource = { kind: "repository"; repository_id: string } | { kind: "user_package"; user_package_id: string };
export type CandidateReason = "no_visible_candidate" | "source_unavailable" | "all_yanked" | "prerelease_excluded" | "stable_unavailable" | "unity_incompatible" | "project_unity_unknown" | "catalog_incomplete" | "metadata_invalid" | "classification_unknown" | "installed_evidence_unknown" | "legacy_metadata" | "source_ambiguous";
export interface CandidateCursor { snapshotFingerprint: string; queryFingerprint: string; offset: number }
export interface CandidateSnapshot { fingerprint: string; projectRevision: number; configRevision: number }
export interface ProjectCandidateRequest {
    projectId: string;
    expectedRevision: number;
    view: { kind: "summary"; packageIds?: string[]; sources?: Array<{ packageId: string; source: CandidateSource }> } | { kind: "versions"; packageId: string; source?: CandidateSource };
    limit?: number;
    expectedSnapshot?: string;
    cursor?: CandidateCursor;
}
export interface PackageCandidateEvidence {
    version: string;
    source: CandidateSource;
    sourceRevision: number;
    classification: "stable" | "prerelease" | "unknown";
    relation: "not_installed" | "newer" | "same_precedence" | "older" | "unknown";
    unity: "not_required" | "compatible" | "incompatible" | "unknown";
    eligibility: "eligible" | "blocked" | "unknown";
    reasons: CandidateReason[];
}
export type CandidateChoice = { kind: "candidate"; candidate: PackageCandidateEvidence } | { kind: "none" | "unknown"; reasons: CandidateReason[] } | { kind: "ambiguous"; reason: "source_ambiguous" };
export type PackageUpdateEvidence = { kind: "target"; candidate: PackageCandidateEvidence } | { kind: "no_update"; reason: "same_precedence" | "installed_newer" | "not_installed" } | { kind: "unavailable"; reasons: CandidateReason[] };
export interface PackageCandidateSummary {
    packageId: string;
    providers: Array<{ source: CandidateSource; sourceRevision: number }>;
    direct: boolean;
    installed: { kind: "absent" } | { kind: "locked"; version: string } | { kind: "unknown"; reason: "installed_evidence_unknown" };
    latest: CandidateChoice;
    latestStable: CandidateChoice;
    projectLatest: CandidateChoice;
    projectLatestStable: CandidateChoice;
    update: PackageUpdateEvidence;
    stableUpdate: PackageUpdateEvidence;
}
interface CandidatePageBase { projectId: string; snapshot: CandidateSnapshot; showPrerelease: boolean; catalogComplete: boolean; nextCursor: CandidateCursor | null }
export type ProjectCandidatePage = (CandidatePageBase & { view: "summary"; items: PackageCandidateSummary[] }) | (CandidatePageBase & { view: "versions"; packageId: string; items: PackageCandidateEvidence[] });
