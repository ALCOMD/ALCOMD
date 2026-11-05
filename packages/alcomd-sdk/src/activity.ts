export interface ActivityCursor { startedAtMs: number; activityId: string }
export interface ActivitySubject { kind: string; id?: string; name?: string }
/** Allowed persisted parameters; requests and translated summaries are never stored here. */
export interface ActivityParameters {
    planId?: string;
    compression?: string;
    unityVersion?: string;
    favorite?: boolean;
    includeLibrary?: boolean;
    excludeVpmPackages?: boolean;
    changeCount?: number;
    permission?: string;
    granted?: boolean;
}
export interface ActivityItem {
    activityId: string;
    revision: number;
    action: string;
    status: string;
    startedAtMs: number;
    finishedAtMs?: number;
    subject: ActivitySubject;
    parameters: ActivityParameters;
    result?: { code: string };
    operationId?: string;
    diagnosticId?: string;
}
export interface ActivityListResult { items: ActivityItem[]; nextCursor?: ActivityCursor }
export interface ActivityGetResult { activity: ActivityItem }
