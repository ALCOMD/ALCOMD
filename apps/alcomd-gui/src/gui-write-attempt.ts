import { useMemo } from "react";
import type { GuiRpcClient } from "./rpc";

type KeyedMethod =
    | "stateCheck" | "operationCancel" | "projectRegister" | "projectRefresh" | "projectSetFavorite" | "projectUnregister"
    | "projectPlanCopy" | "projectPlanDeleteDirectory" | "projectPlanUnityMigration"
    | "repositoryRegister" | "repositorySetCredentials" | "repositorySetDisplayName" | "repositoryMove"
    | "repositoryRefresh" | "repositoriesRefreshAll" | "repositoryUnregister"
    | "userPackageEnroll" | "userPackageRefresh" | "userPackageRemove"
    | "unityInstallationRegister" | "unityInstallationRemove" | "unityInstallationsRefresh"
    | "unityProjectLaunchConfigSet" | "unityProjectLaunchConfigClear" | "unityLaunch"
    | "extensionEnable" | "extensionDisable" | "extensionSetGrant" | "extensionRevokeGrant";

// The view supplies the business arguments. The attempt supplies the transport key.
type BusinessArgs<Method extends KeyedMethod> = Parameters<GuiRpcClient[Method]> extends [...infer Args, string] ? Args : never;
type WriteResult<Method extends KeyedMethod> = Awaited<ReturnType<GuiRpcClient[Method]>>;

export function createGuiWriteAttempt<Methods extends KeyedMethod>(client: Pick<GuiRpcClient, Methods>) {
    const attempts = new Map<KeyedMethod, { args: unknown[]; unknown: boolean }>();

    return async <Method extends Methods>(method: Method, ...input: BusinessArgs<Method>): Promise<WriteResult<Method>> => {
        const previous = attempts.get(method);
        const retry = previous?.unknown && JSON.stringify(input) === JSON.stringify(previous.args.slice(0, -1));
        const attempt = retry ? previous : { args: [...structuredClone(input), crypto.randomUUID()], unknown: false };
        attempts.set(method, attempt);
        attempt.unknown = false;
        try {
            const invoke = client[method] as (...args: unknown[]) => Promise<WriteResult<Method>>;
            return await invoke.apply(client, attempt.args);
        } catch (error: unknown) {
            attempt.unknown = typeof error === "object" && error !== null && "code" in error
                && (error.code === "daemon_unavailable" || error.code === "internal_error");
            throw error;
        } finally {
            if (!attempt.unknown && attempts.get(method) === attempt) attempts.delete(method);
        }
    };
}

// A new dialog, target or edited draft owns a separate attempt. Only this view
// can replay its unacknowledged write, including the original arguments and key.
export function useGuiWrite(client: GuiRpcClient, scope?: unknown, draft?: unknown) {
    return useMemo(() => createGuiWriteAttempt(client), [client, scope, draft]);
}
