import { productFamily } from "@alcomd/ui";
import type { RpcError } from "@alcomd/sdk";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { WindowStateMemory, windowGeometry } from "./window-state";

export function nativeWindowStateMemory(feedback: (error: RpcError | undefined) => void): WindowStateMemory | undefined {
    if (!isTauri()) return undefined;
    const host = getCurrentWindow();
    return new WindowStateMemory({
        configure: async (enabled, geometry, fullscreen) => windowGeometry(await invoke("gui_window_state_configure", {
            enabled, fullscreen, width: geometry?.[0] ?? null, height: geometry?.[1] ?? null, maximized: geometry?.[2] ?? null
        })),
        listen: async (changed, failed) => {
            const stopGeometry = await host.listen("alcomd-window-state", ({ payload }) => changed(payload));
            try {
                const stopErrors = await host.listen<RpcError>("alcomd-window-state-error", ({ payload }) => failed(payload));
                return () => { stopGeometry(); stopErrors(); };
            } catch (error) { stopGeometry(); throw error; }
        }
    }, () => window.localStorage, `${productFamily.toLowerCase()}.gui.window-state.v1`, feedback);
}
