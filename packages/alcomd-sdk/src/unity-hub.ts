/** Guards against opening a different effective Hub than the accepted settings evidence. */
export interface UnityHubOpenParams {
    idempotencyKey: string;
    expectedRevision: number;
    expectedTargetToken: string | null;
}

/** Spawn acceptance does not imply that a Unity Hub window is ready. */
export interface UnityHubOpenResult {
    outcome: "launch_requested" | "download_required";
}

export const UNITY_HUB_OPEN_METHOD = "unity.hub.open" as const;
