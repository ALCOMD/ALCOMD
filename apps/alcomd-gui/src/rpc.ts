import type {
    ExtensionResult,
    ExtensionUiCloseParams,
    ExtensionUiCloseResult,
    ExtensionUiDispatchParams,
    ExtensionUiDispatchResult,
    ExtensionUiOpenParams,
    ExtensionUiOpenResult,
    ExtensionUiRefreshParams,
    ExtensionUiSnapshotResult,
    RpcError
} from "@alcomd/sdk";
import { invoke } from "@tauri-apps/api/core";

export interface GuiRpcClient {
    extensionGet(extensionId: string): Promise<ExtensionResult>;
    extensionUiOpen(params: ExtensionUiOpenParams): Promise<ExtensionUiOpenResult>;
    extensionUiRefresh(params: ExtensionUiRefreshParams): Promise<ExtensionUiSnapshotResult>;
    extensionUiDispatch(params: ExtensionUiDispatchParams): Promise<ExtensionUiDispatchResult>;
    extensionUiClose(params: ExtensionUiCloseParams): Promise<ExtensionUiCloseResult>;
}

class TauriGuiRpcClient implements GuiRpcClient {
    extensionGet(extensionId: string): Promise<ExtensionResult> {
        return invokeTyped("gui_extension_get", { extensionId });
    }

    extensionUiOpen(params: ExtensionUiOpenParams): Promise<ExtensionUiOpenResult> {
        return invokeTyped("gui_extension_ui_open", { params });
    }

    extensionUiRefresh(params: ExtensionUiRefreshParams): Promise<ExtensionUiSnapshotResult> {
        return invokeTyped("gui_extension_ui_refresh", { params });
    }

    extensionUiDispatch(params: ExtensionUiDispatchParams): Promise<ExtensionUiDispatchResult> {
        return invokeTyped("gui_extension_ui_dispatch", { params });
    }

    extensionUiClose(params: ExtensionUiCloseParams): Promise<ExtensionUiCloseResult> {
        return invokeTyped("gui_extension_ui_close", { params });
    }
}

async function invokeTyped<Result>(command: string, args: Record<string, unknown>): Promise<Result> {
    try {
        return await invoke<Result>(command, args);
    } catch (error: unknown) {
        throw normalizeRpcError(error);
    }
}

export function normalizeRpcError(value: unknown): RpcError {
    if (isRecord(value) && typeof value.code === "string") {
        return {
            code: value.code,
            message: typeof value.message === "string"
                ? value.message
                : "The request could not be completed.",
            ...(typeof value.diagnosticId === "string"
                ? { diagnosticId: value.diagnosticId }
                : {})
        };
    }
    return {
        code: "daemon_unavailable",
        message: "The ALCOMD core is unavailable."
    };
}

function isRecord(value: unknown): value is Record<string, unknown> {
    return typeof value === "object" && value !== null;
}

export const guiRpcClient: GuiRpcClient = new TauriGuiRpcClient();
