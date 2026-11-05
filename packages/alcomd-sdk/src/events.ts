import type { RpcError } from "./index";

export type CoreEventAggregateKind = "operation" | "project" | "repository" | "unity-installation"
    | "project-editor-preference" | "project-unity-launch-config" | "project-template" | "user-package";

export interface CoreEvent {
    sequence: number;
    eventId: string;
    kind: string;
    aggregateKind: CoreEventAggregateKind;
    aggregateId: string;
    aggregateRevision: number;
    occurredAtMs: number;
    payload: Record<string, unknown>;
}

export interface EventsCursor {
    instanceId: string;
    sequence: number;
    resourcesRevision: number;
}

export type EventInvalidation = "extensions" | "operation-log";

export interface EventsSubscribeParams {
    cursor?: EventsCursor;
    limit?: number;
}

export interface EventsSubscribeResult {
    subscriptionId: string;
    instanceId: string;
    baselineCursor: EventsCursor;
    resyncRequired: boolean;
}

export interface EventsBatch {
    subscriptionId: string;
    instanceId: string;
    batchId: number;
    events: CoreEvent[];
    invalidatedScopes: EventInvalidation[];
    nextCursor: EventsCursor;
}

export interface EventsClosed {
    subscriptionId: string;
    instanceId: string;
    error: RpcError;
    retryable: boolean;
}

export interface NotificationEnvelope<T> {
    method: "events.batch" | "events.closed";
    params: T;
}

export type EventNotification =
    | { method: "events.batch"; params: EventsBatch }
    | { method: "events.closed"; params: EventsClosed };

export interface OperationsListCursor {
    createdAtMs: number;
    operationId: string;
}

export interface OperationsListParams {
    cursor?: OperationsListCursor;
    limit?: number;
    activeOnly?: boolean;
}

/** Shared envelope shape without redefining the application's Operation DTO. */
export interface OperationsListResult<T> {
    operations: T[];
    activeCount: number;
    nextCursor?: OperationsListCursor;
}
