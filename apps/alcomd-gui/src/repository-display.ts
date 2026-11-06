import type { RepositorySnapshot } from "./core-models";
import type { Translate } from "./i18n-types";

function builtinRepositoryName(repositoryId: string | undefined): "Official" | "Curated" | undefined {
    switch (repositoryId) {
        case "7e2233c8-0b3f-4cf2-aeb4-57d3d240b101": return "Official";
        case "7e2233c8-0b3f-4cf2-aeb4-57d3d240b102": return "Curated";
        default: return undefined;
    }
}

export function isBuiltinRepository(repository: RepositorySnapshot): boolean {
    return builtinRepositoryName(repository.repositoryId) !== undefined;
}

export function repositoryDisplayName(t: Translate, repository: RepositorySnapshot, fallback: string): string {
    const name = builtinRepositoryName(repository.repositoryId);
    return name === undefined ? repository.name ?? repository.declaredId ?? fallback : t(name);
}
