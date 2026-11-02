/** A readable label only. Keep the original path for RPC, editing and filesystem actions. */
export function displayPath(path: string): string {
    if (/^\\\\\?\\UNC\\[^\\]+\\[^\\]+(?:\\|$)/i.test(path)) return `\\\\${path.slice(8)}`;
    if (/^\\\\\?\\[A-Za-z]:\\/.test(path)) return path.slice(4);
    return path;
}
