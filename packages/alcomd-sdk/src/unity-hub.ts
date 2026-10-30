/** Guards against opening a different effective Hub than the accepted settings evidence. */
export interface UnityHubOpenParams {
    expectedRevision: number;
    expectedPath: string | null;
}

/** Spawn acceptance does not imply that a Unity Hub window is ready. */
export interface UnityHubOpenResult {
    outcome: "spawned" | "download_required";
}

export const UNITY_HUB_OPEN_METHOD = "unity.hub.open" as const;
