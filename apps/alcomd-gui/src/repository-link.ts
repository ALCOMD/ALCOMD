export interface RepositoryLinkDraft {
    url: string;
    headers: Array<{ name: string; value: string }>;
}

// Decode the desktop activation into the existing form. Core validates the
// source and credentials when the user chooses to review the repository.
export function parseRepositoryLink(input: string): RepositoryLinkDraft | undefined {
    if (input.length > 16 * 1024) return undefined;
    try {
        const link = new URL(input);
        if (link.protocol !== "vcc:" || link.host !== "vpm" || link.pathname !== "/addRepo"
            || link.username || link.password) return undefined;

        const urls = link.searchParams.getAll("url");
        const [target] = urls;
        if (urls.length !== 1 || target === undefined) return undefined;
        const source = new URL(target);
        if ((source.protocol !== "https:" && source.protocol !== "http:")
            || source.username || source.password) return undefined;

        const headers = new Map<string, string>();
        for (const [key, entry] of link.searchParams) {
            if (key !== "headers[]") continue;
            const separator = entry.indexOf(":");
            if (separator < 1) return undefined;
            headers.set(entry.slice(0, separator), entry.slice(separator + 1));
        }
        return { url: source.href, headers: Array.from(headers, ([name, value]) => ({ name, value })) };
    } catch {
        return undefined;
    }
}
