export type SdkErrorKind = "remote" | "transport" | "protocol" | "unavailable" | "timeout" | "invalid_request" | "cancelled";

interface ErrorDetails {
    code?: string;
    diagnosticId?: string;
    data?: unknown;
    outcomeUnknown?: boolean;
}

export class AlcomdSdkError extends Error {
    constructor(kind: SdkErrorKind, message: string, details?: ErrorDetails);
    readonly kind: SdkErrorKind;
    readonly code?: string;
    readonly diagnosticId?: string;
    readonly data?: unknown;
    readonly outcomeUnknown: boolean;
}

interface ExchangeOptions {
    signal?: AbortSignal;
    timeoutMs?: number;
}

export interface FramedBridge {
    exchange(method: string, params: unknown, options?: ExchangeOptions, mutates?: boolean): Promise<unknown>;
    notification(): Promise<unknown>;
    close(): void;
}

export function openBridge(helperPath?: string): Promise<FramedBridge>;
export function isRecord(value: unknown): value is Record<string, unknown>;
export function requestTimeout(value?: number, fallback?: number): number;
