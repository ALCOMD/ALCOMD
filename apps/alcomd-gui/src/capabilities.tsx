import { RPC_CAPABILITIES } from "@alcomd/sdk";
import { createContext, useContext, type ReactNode } from "react";

export const capabilities = {
    backupsCreate: "backups.create.v1",
    backupsRead: "backups.read.v1",
    backupsRestore: "backups.restore.v1",
    extensionsLifecycle: RPC_CAPABILITIES.extensionsLifecycle,
    extensionsPermissions: RPC_CAPABILITIES.extensionsPermissions,
    extensionsPortableUi: "extensions.ui.portable.v1",
    operations: "operations.v1",
    packagesApply: "packages.apply.v1",
    packagesPlanV1: "packages.plan.v1",
    packagesPlanV2: "packages.plan.v2",
    packagesUserPackages: "packages.user-packages.v1",
    projectsCopy: "projects.copy.v1",
    projectsDelete: "projects.delete.v1",
    projectsRead: "projects.read.v1",
    projectsRegistry: "projects.registry.v1",
    repositoriesRead: "repositories.read.v1",
    repositoriesRegistry: "repositories.registry.v1",
    stateCheck: "state.check.v1",
    templatesCreateProject: "templates.create-project.v1",
    templatesManage: "templates.manage.v1",
    templatesRead: "templates.read.v1",
    unityLaunch: "unity.launch.v1",
    unityManage: "unity.manage.v1",
    unityRead: "unity.read.v1"
} as const;

export type CapabilitySnapshot =
    | { readonly kind: "ready"; readonly values: ReadonlySet<string> }
    | { readonly kind: "status-unavailable" };

const CapabilityContext = createContext<CapabilitySnapshot | undefined>(undefined);

export function CapabilityProvider({ children, value }: { children: ReactNode; value?: CapabilitySnapshot }) {
    return <CapabilityContext.Provider value={value}>{children}</CapabilityContext.Provider>;
}

export function useCapability(capability: string): boolean {
    const snapshot = useContext(CapabilityContext);
    return snapshot?.kind === "ready" && snapshot.values.has(capability);
}

export type CapabilityState = "checking" | "available" | "unavailable" | "status-unavailable";

export function useCapabilityState(capability: string): CapabilityState {
    return useCapabilitiesState([capability]);
}

export function useCapabilitiesState(required: readonly string[]): CapabilityState {
    return useCapabilityCheck(required).state;
}

export function useCapabilityCheck(required: readonly string[]): { readonly missing: readonly string[]; readonly state: CapabilityState } {
    const snapshot = useContext(CapabilityContext);
    if (snapshot === undefined) return { missing: [], state: "checking" };
    if (snapshot.kind === "status-unavailable") return { missing: [], state: "status-unavailable" };
    const missing = required.filter((capability) => !snapshot.values.has(capability));
    return { missing, state: missing.length === 0 ? "available" : "unavailable" };
}

export function capabilityUnavailableTitle(available: boolean, capability: string): string | undefined {
    return available ? undefined : `Unavailable because ${capability} was not negotiated.`;
}
