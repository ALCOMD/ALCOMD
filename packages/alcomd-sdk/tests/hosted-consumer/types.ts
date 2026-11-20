import { RPC_VERSION, type NativeRpcMethods } from "@alcomd/sdk";
import { connect, type AlcomdClient } from "@alcomd/sdk/node";

async function useInstalledSdk() {
    const client: AlcomdClient = await connect();
    try {
        const status: NativeRpcMethods["system.status"]["result"] = await client.request("system.status", {});
        const subscription = await client.subscribeEvents();
        subscription.close();
        return { status, RPC_VERSION };
    } finally {
        client.close();
    }
}

void useInstalledSdk;

const install = {
    projectId: "project",
    expectedRevision: 1,
    packageId: "com.example.package",
    versionRange: null,
    source: null
} satisfies NativeRpcMethods["packages.planInstall"]["params"];
const resolve = {
    projectId: install.projectId,
    expectedRevision: 1,
} satisfies NativeRpcMethods["packages.planResolve"]["params"];
// @ts-expect-error The native owner can select a local package but cannot claim a bundled source.
const bundled: NativeRpcMethods["extensions.planInstall"]["params"]["sourceKind"] = "first_party_packaged";
// @ts-expect-error The projects update requires the auto-discovery setting.
const emptyProjects: NonNullable<NativeRpcMethods["settings.update"]["params"]["update"]["projects"]> = {};
void [resolve, bundled, emptyProjects];
