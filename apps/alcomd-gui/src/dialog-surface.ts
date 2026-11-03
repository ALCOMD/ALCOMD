const surfaces: HTMLElement[] = [];
const listeners = new Set<() => void>();

export function subscribeDialogSurface(listener: () => void) {
    listeners.add(listener);
    return () => { listeners.delete(listener); };
}

export function currentDialogSurface() {
    return surfaces.at(-1);
}

/** Keep shared floating feedback inside the active native modal's focus scope. */
export function registerDialogSurface(surface: HTMLElement) {
    surfaces.push(surface);
    listeners.forEach((listener) => listener());
    return () => {
        const index = surfaces.indexOf(surface);
        if (index < 0) return;
        surfaces.splice(index, 1);
        listeners.forEach((listener) => listener());
    };
}
