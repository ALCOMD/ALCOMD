import type { EventsBatch, EventsClosed, EventsCursor, EventsSubscribeResult, RpcError } from "@alcomd/sdk";

export interface GuiEventsRequest {
    consumerId: string;
    generation: number;
    cursor?: EventsCursor;
}

interface GuiEventIdentity { consumerId: string; generation: number }
export type GuiEventPayload =
    | { type: "ready"; ready: EventsSubscribeResult }
    | { type: "batch"; batch: EventsBatch }
    | { type: "closed"; closed: EventsClosed }
    | { type: "error"; error: RpcError; retryable: boolean }
;
export type GuiEventMessage = GuiEventIdentity & GuiEventPayload;

export interface GuiEventSubscription {
    acknowledge(batch: EventsBatch): Promise<void>;
    close(): Promise<void>;
}
