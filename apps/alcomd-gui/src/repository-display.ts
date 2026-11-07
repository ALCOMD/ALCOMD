import type { RepositorySnapshot } from "./core-models";
import type { Translate } from "./i18n-types";

export function isBuiltinRepository(repository: RepositorySnapshot): boolean {
    return repository.builtin !== undefined;
}

export function repositoryDefaultName(t: Translate, repository: RepositorySnapshot, fallback: string): string {
    if (repository.builtin === "official") return t("Official");
    if (repository.builtin === "curated") return t("Curated");
    return repository.name ?? repository.declaredId ?? fallback;
}

export function repositoryDisplayName(t: Translate, repository: RepositorySnapshot, fallback: string): string {
    return repository.displayName ?? repositoryDefaultName(t, repository, fallback);
}
