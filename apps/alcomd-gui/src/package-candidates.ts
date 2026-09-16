import type { CandidateChoice, CandidateSnapshot, CandidateSource, PackageCandidateEvidence, PackageCandidateSummary, ProjectCandidatePage, ProjectCandidateRequest, PackageUpdateEvidence } from "@alcomd/sdk";
import type { PackageBulkIntent, PackageSourceSelector } from "./core-models";
import type { GuiRpcClient } from "./rpc";

export function candidateSource(source: CandidateSource): PackageSourceSelector {
    return source.kind === "repository" ? { kind: source.kind, repositoryId: source.repository_id } : { kind: source.kind, userPackageId: source.user_package_id };
}
export function wireSource(source: PackageSourceSelector): CandidateSource {
    return source.kind === "repository" ? { kind: source.kind, repository_id: source.repositoryId } : { kind: source.kind, user_package_id: source.userPackageId };
}
export function candidateSourceKey(source: CandidateSource): string {
    return source.kind === "repository" ? `repository:${source.repository_id}` : `user-package:${source.user_package_id}`;
}
export function choiceText(choice: CandidateChoice): string {
    return choice.kind === "candidate" ? choice.candidate.version : choice.kind === "ambiguous" ? "Choose a source" : choice.reasons.join(", ").replaceAll("_", " ");
}
export function updateReason(update: PackageUpdateEvidence): string {
    return update.kind === "target" ? "" : (update.kind === "no_update" ? update.reason : update.reasons.join(", ")).replaceAll("_", " ");
}
export function intentForSummary(row: PackageCandidateSummary, stable: boolean): PackageBulkIntent | undefined {
    const update = stable ? row.stableUpdate : row.update;
    const choice = stable ? row.projectLatestStable : row.projectLatest;
    const candidate = row.installed.kind === "locked" ? (update.kind === "target" ? update.candidate : undefined) : row.installed.kind === "absent" && choice.kind === "candidate" ? choice.candidate : undefined;
    if (candidate?.eligibility !== "eligible" || candidate.relation === "unknown") return undefined;
    return { kind: row.installed.kind === "locked" ? "upgrade" : "install", packageId: row.packageId, versionRange: `=${candidate.version}`, source: candidateSource(candidate.source), includePrerelease: candidate.classification === "prerelease" };
}
export function uncertainUpdate(row: PackageCandidateSummary, stable: boolean): boolean {
    const choice = stable ? row.projectLatestStable : row.projectLatest;
    return row.installed.kind === "unknown" || choice.kind === "unknown" || choice.kind === "ambiguous";
}

// Shared across every workspace/menu. At most two requests, one per traversal.
let inFlight = 0;
const waiting: Array<() => void> = [];
export async function queryCandidates(client: GuiRpcClient, request: ProjectCandidateRequest): Promise<ProjectCandidatePage> {
    if (inFlight >= 2) await new Promise<void>((resolve) => waiting.push(resolve));
    else inFlight += 1;
    try { return await client.packageQueryProjectCandidates(request); }
    finally { const next = waiting.shift(); if (next) next(); else inFlight -= 1; }
}
export interface CandidateSummarySet { items: PackageCandidateSummary[]; snapshot: CandidateSnapshot; catalogComplete: boolean }
export async function readCandidateSummary(client: GuiRpcClient, request: ProjectCandidateRequest): Promise<CandidateSummarySet> {
    let cursor = request.cursor;
    let snapshot: CandidateSnapshot | undefined;
    let catalogComplete = true;
    const items: PackageCandidateSummary[] = [];
    const seenIds = new Set<string>();
    for (let pageIndex = 0; pageIndex < 1563; pageIndex += 1) {
        const page = await queryCandidates(client, { ...request, limit: 64, ...(snapshot === undefined ? {} : { expectedSnapshot: snapshot.fingerprint }), ...(cursor === undefined ? {} : { cursor }) });
        if (page.view !== "summary" || page.projectId !== request.projectId || page.snapshot.projectRevision !== request.expectedRevision) throw { code: "package_candidate_evidence_stale" };
        if ((snapshot && page.snapshot.fingerprint !== snapshot.fingerprint) || (request.expectedSnapshot && page.snapshot.fingerprint !== request.expectedSnapshot)) throw { code: "package_candidate_evidence_stale" };
        snapshot = page.snapshot;
        catalogComplete &&= page.catalogComplete;
        for (const item of page.items) {
            if (seenIds.has(item.packageId)) throw { code: "package_candidate_evidence_stale" };
            seenIds.add(item.packageId);
            items.push(item);
        }
        if (items.length > 100000) throw { code: "package_candidate_limit_exceeded" };
        if (page.nextCursor === null) return { items, snapshot, catalogComplete };
        if (page.nextCursor.offset <= (cursor?.offset ?? 0)) throw { code: "package_candidate_evidence_stale" };
        cursor = page.nextCursor;
    }
    throw { code: "package_candidate_limit_exceeded" };
}
export function actionableVersion(candidate: PackageCandidateEvidence): boolean {
    return candidate.eligibility === "eligible" && ["not_installed", "newer", "older"].includes(candidate.relation);
}

export function validateVersionPage(page: ProjectCandidatePage, expected: { projectId: string; projectRevision: number; packageId: string; fingerprint: string; pages: number; count: number; offset: number }): asserts page is Extract<ProjectCandidatePage, { view: "versions" }> {
    if (expected.pages >= 1563 || expected.count + page.items.length > 100000 || (page.nextCursor?.offset ?? 0) > 100000) throw { code: "package_candidate_limit_exceeded" };
    if (page.view !== "versions" || page.projectId !== expected.projectId || page.packageId !== expected.packageId || page.snapshot.projectRevision !== expected.projectRevision || page.snapshot.fingerprint !== expected.fingerprint || (page.nextCursor !== null && (page.nextCursor.offset <= expected.offset || page.nextCursor.snapshotFingerprint !== expected.fingerprint))) throw { code: "package_candidate_evidence_stale" };
}
