/** A readable label only. Keep the original path for RPC and filesystem actions. */
export function displayPath(path: string): string {
    const suffix = path.startsWith("\\\\?\\") ? path.slice(4) : "";
    let ordinary: string;
    let components: string;
    if (/^[A-Za-z]:\\/.test(suffix)) {
        ordinary = suffix;
        components = suffix.slice(3);
        if (components.length === 0) return ordinary;
    } else if (/^UNC\\[^\\]+\\[^\\]+(?:\\|$)/i.test(suffix)) {
        components = suffix.slice(4);
        ordinary = "\\\\" + components;
    } else {
        return path;
    }
    // Match the ordinary spelling rules at the native boundary. A displayed
    // path should remain usable when copied to tools without long-path support.
    return ordinary.length < 260 && components.replace(/\\$/, "").split("\\").every(isOrdinaryComponent)
        ? ordinary
        : path;
}

function isOrdinaryComponent(name: string): boolean {
    if (name.length === 0 || /[. ]$/.test(name) || /[<>:"/|?*]/.test(name)
        || [...name].some((character) => character.charCodeAt(0) < 32)) return false;
    const base = name.split(".", 1)[0]!.replace(/ +$/, "");
    return !/^(?:CON|PRN|AUX|NUL|CONIN\$|CONOUT\$|(?:COM|LPT)[1-9¹²³])$/i.test(base);
}

/** Displays an absolute parent and a single leaf as one path. */
export function displayChildPath(parent: string, leaf: string): string {
    const separator = parent.startsWith("/") ? "/" : "\\";
    return displayPath(parent + (parent.endsWith(separator) ? "" : separator) + leaf);
}
