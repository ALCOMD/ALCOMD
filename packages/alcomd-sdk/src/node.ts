import type { EventNotification, EventsCursor, EventsSubscribeParams, EventsSubscribeResult } from "./events.js";
import { RPC_VERSION, type HelloResponse } from "./index.js";
import { AlcomdSdkError, isRecord, openBridge, requestTimeout, type FramedBridge } from "./node-runtime.mjs";
import { SDK_METHODS, type NativeRpcMethods } from "./sdk-methods.js";

export { AlcomdSdkError, type SdkErrorKind } from "./node-runtime.mjs";

export interface RequestOptions {
    readonly timeoutMs?: number;
    readonly signal?: AbortSignal;
}

export interface ConnectOptions extends RequestOptions {
    /** Absolute path to an explicitly trusted helper for development or custom installations. */
    readonly helperPath?: string;
}

export interface EventSubscription {
    readonly ready: EventsSubscribeResult;
    /** One consumer per subscription. Returns batches and the native terminal notification. */
    readonly notifications: AsyncIterable<EventNotification>;
    close(): void;
}

export type CommandMethod = Exclude<keyof NativeRpcMethods, "events.subscribe">;

export interface AlcomdClient {
    readonly hello: HelloResponse;
    request<M extends CommandMethod>(
        method: M, params: NativeRpcMethods[M]["params"], options?: RequestOptions
    ): Promise<NativeRpcMethods[M]["result"]>;
    /** Opens a dedicated stream. Pass the last consumed cursor to recover after disconnect. */
    subscribeEvents(params?: EventsSubscribeParams, options?: RequestOptions): Promise<EventSubscription>;
    close(): void;
}

class NativeClient implements AlcomdClient {
    readonly #helperPath: string | undefined;
    readonly #timeout: number;
    readonly #bridge: FramedBridge;
    readonly #subscriptions = new Set<FramedBridge>();
    #closed = false;

    constructor(bridge: FramedBridge, readonly hello: HelloResponse, helperPath: string | undefined, timeout: number) {
        this.#bridge = bridge;
        this.#helperPath = helperPath;
        this.#timeout = timeout;
    }

    async request<M extends CommandMethod>(
        method: M, params: NativeRpcMethods[M]["params"], options: RequestOptions = {}
    ): Promise<NativeRpcMethods[M]["result"]> {
        const methods: Readonly<Record<string, "read" | "write" | "stream">> = SDK_METHODS;
        const kind = Object.hasOwn(methods, method) ? methods[method] : undefined;
        if (kind !== "read" && kind !== "write") {
            throw new AlcomdSdkError("invalid_request", "The method is not an SDK command.");
        }
        const timeoutMs = requestTimeout(options.timeoutMs, this.#timeout);
        return await this.#bridge.exchange(method, params, { ...options, timeoutMs }, kind === "write") as NativeRpcMethods[M]["result"];
    }

    async subscribeEvents(params: EventsSubscribeParams = {}, options: RequestOptions = {}): Promise<EventSubscription> {
        if (this.#closed) throw new AlcomdSdkError("transport", "The SDK connection is closed.");
        const timeoutMs = requestTimeout(options.timeoutMs, this.#timeout);
        const settings = { ...options, timeoutMs };
        const { bridge } = await connectBridge(this.#helperPath, settings);
        if (this.#closed) {
            bridge.close();
            throw new AlcomdSdkError("transport", "The SDK connection is closed.");
        }
        this.#subscriptions.add(bridge);
        const close = () => {
            this.#subscriptions.delete(bridge);
            bridge.close();
        };
        try {
            const ready = subscriptionReady(await bridge.exchange("events.subscribe", params, settings));
            const notifications = this.notifications(bridge, ready, close);
            return { ready, notifications, close };
        } catch (error) {
            close();
            throw error;
        }
    }

    async *notifications(
        bridge: FramedBridge, ready: EventsSubscribeResult, close: () => void
    ): AsyncGenerator<EventNotification> {
        try {
            while (this.#subscriptions.has(bridge)) {
                const raw = await bridge.notification();
                if (!this.#subscriptions.has(bridge)) return;
                const notification = eventNotification(raw, ready);
                if (notification.method === "events.closed") {
                    close();
                    yield notification;
                    return;
                }
                yield notification;
            }
        } catch (error) {
            if (this.#subscriptions.has(bridge)) throw error;
        } finally {
            close();
        }
    }

    close(): void {
        this.#closed = true;
        this.#bridge.close();
        for (const bridge of this.#subscriptions) bridge.close();
        this.#subscriptions.clear();
    }
}

function positiveU32(value: unknown): boolean {
    return typeof value === "number" && Number.isInteger(value) && value > 0 && value <= 0xffff_ffff;
}

function helloResult(value: unknown): HelloResponse {
    if (!isRecord(value) || value.rpcVersion !== RPC_VERSION
        || typeof value.daemonVersion !== "string" || value.daemonVersion.length === 0
        || !Array.isArray(value.capabilities)
        || value.capabilities.some((capability) => typeof capability !== "string")
        || (value.dataSchema !== undefined && !positiveU32(value.dataSchema))
        || (value.configSchema !== undefined && !positiveU32(value.configSchema))
        || (value.extensionApi !== undefined && (!isRecord(value.extensionApi)
            || !positiveU32(value.extensionApi.major) || typeof value.extensionApi.world !== "string"))) {
        throw new AlcomdSdkError("protocol", "The SDK bridge sent an invalid hello result.");
    }
    return value as unknown as HelloResponse;
}

function nonnegativeInteger(value: unknown): boolean {
    return typeof value === "number" && Number.isSafeInteger(value) && value >= 0;
}

function isCursor(value: unknown, instanceId: string): value is EventsCursor {
    return isRecord(value) && value.instanceId === instanceId
        && nonnegativeInteger(value.sequence) && nonnegativeInteger(value.resourcesRevision);
}

function subscriptionReady(value: unknown): EventsSubscribeResult {
    if (!isRecord(value) || typeof value.subscriptionId !== "string"
        || typeof value.instanceId !== "string" || typeof value.resyncRequired !== "boolean"
        || !isCursor(value.baselineCursor, value.instanceId)) {
        throw new AlcomdSdkError("protocol", "The SDK bridge sent an invalid event subscription receipt.");
    }
    return value as unknown as EventsSubscribeResult;
}

function eventNotification(value: unknown, ready: EventsSubscribeResult): EventNotification {
    if (isRecord(value) && !Object.hasOwn(value, "id") && isRecord(value.params)
        && value.params.subscriptionId === ready.subscriptionId && value.params.instanceId === ready.instanceId) {
        if (value.method === "events.batch" && nonnegativeInteger(value.params.batchId)
            && value.params.batchId !== 0 && Array.isArray(value.params.events)
            && Array.isArray(value.params.invalidatedScopes)
            && isCursor(value.params.nextCursor, ready.instanceId)) {
            return value as unknown as EventNotification;
        }
        if (value.method === "events.closed" && typeof value.params.retryable === "boolean"
            && isRecord(value.params.error) && typeof value.params.error.code === "string"
            && typeof value.params.error.message === "string") {
            return value as unknown as EventNotification;
        }
    }
    throw new AlcomdSdkError("protocol", "The SDK bridge sent an invalid event notification.");
}

async function connectBridge(helperPath: string | undefined, options: RequestOptions) {
    if (options.signal?.aborted) throw new AlcomdSdkError("cancelled", "The connection was cancelled.");
    const bridge = await openBridge(helperPath);
    try {
        const hello = helloResult(await bridge.exchange("connect", {}, options));
        return { bridge, hello };
    } catch (error) {
        bridge.close();
        throw error;
    }
}

/** Connects as the current operating-system user over the packaged native bridge. */
export async function connect(options: ConnectOptions = {}): Promise<AlcomdClient> {
    const timeoutMs = requestTimeout(options.timeoutMs);
    const { bridge, hello } = await connectBridge(options.helperPath, { signal: options.signal, timeoutMs });
    return new NativeClient(bridge, hello, options.helperPath, timeoutMs);
}
