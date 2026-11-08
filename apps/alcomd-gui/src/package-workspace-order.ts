import type { PackageCandidateSummary } from "@alcomd/sdk";

interface PackageOrderRow {
    installedVersion?: string;
    evidence?: Pick<PackageCandidateSummary, "providers">;
}

export function orderPackageWorkspaceRows<T extends PackageOrderRow>(rows: readonly T[]): T[] {
    const ranked = rows.map((row) => ({ row, sourceOrder: Math.min(Number.MAX_SAFE_INTEGER, ...(row.evidence?.providers ?? []).map((provider) => provider.displayOrder)) }));
    // Stable sorting retains Core enumeration within each source. The earliest
    // visible provider fixes a shared package's position, independently of selection.
    ranked.sort((left, right) => Number(right.row.installedVersion !== undefined) - Number(left.row.installedVersion !== undefined)
        || left.sourceOrder - right.sourceOrder);
    return ranked.map(({ row }) => row);
}
