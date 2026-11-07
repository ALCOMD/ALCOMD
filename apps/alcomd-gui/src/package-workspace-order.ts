import type { PackageCandidateSummary } from "@alcomd/sdk";
import type { RepositorySnapshot } from "./core-models";

interface PackageOrderRow {
    installedVersion?: string;
    evidence?: Pick<PackageCandidateSummary, "providers">;
}

export function orderPackageWorkspaceRows<T extends PackageOrderRow>(rows: readonly T[], repositories: readonly RepositorySnapshot[]): T[] {
    const repositoryOrder = new Map<string, number>();
    for (const [index, repository] of repositories.entries()) {
        if (repository.repositoryId === undefined) continue;
        repositoryOrder.set(repository.repositoryId, repository.builtin === "official" ? 0 : repository.builtin === "curated" ? 1 : index + 3);
    }
    const unregisteredSource = repositories.length + 3;
    const noSource = unregisteredSource + 1;
    const ranked = rows.map((row) => {
        let sourceOrder = noSource;
        for (const { source } of row.evidence?.providers ?? []) {
            const rank = source.kind === "user_package" ? 2 : repositoryOrder.get(source.repository_id) ?? unregisteredSource;
            sourceOrder = Math.min(sourceOrder, rank);
        }
        return { row, sourceOrder };
    });
    // Stable sorting retains Core enumeration within each source. The earliest
    // visible provider fixes a shared package's position, independently of selection.
    ranked.sort((left, right) => Number(right.row.installedVersion !== undefined) - Number(left.row.installedVersion !== undefined)
        || left.sourceOrder - right.sourceOrder);
    return ranked.map(({ row }) => row);
}
