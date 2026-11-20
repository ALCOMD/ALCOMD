import { spawn } from "node:child_process";
import { stat } from "node:fs/promises";
import { isAbsolute } from "node:path";
import { fileURLToPath } from "node:url";

const MAX_FRAME_BYTES = 4 * 1024 * 1024;
const MAX_TIMEOUT_MS = 2_147_483_647;
const DECODER = new TextDecoder("utf-8", { fatal: true });
const ERROR_KINDS = new Set(["remote", "transport", "protocol", "unavailable", "timeout", "invalid_request", "cancelled"]);

export class AlcomdSdkError extends Error {
    constructor(kind, message, details = {}) {
        super(message);
        this.name = "AlcomdSdkError";
        this.kind = kind;
        this.code = details.code;
        this.diagnosticId = details.diagnosticId;
        this.data = details.data;
        this.outcomeUnknown = details.outcomeUnknown === true;
    }
}

export function isRecord(value) {
    return typeof value === "object" && value !== null && !Array.isArray(value);
}

function protocolError(message) {
    return new AlcomdSdkError("protocol", message);
}

function checkNumber(_key, value) {
    if (typeof value === "number"
        && (!Number.isFinite(value) || (Number.isInteger(value) && !Number.isSafeInteger(value)))) {
        throw new RangeError("The value exceeds the supported JavaScript numeric range.");
    }
    return value;
}

function encodeFrame(message) {
    try {
        const json = JSON.stringify(message, checkNumber);
        const length = Buffer.byteLength(json);
        if (length === 0 || length > MAX_FRAME_BYTES) throw new RangeError();
        const frame = Buffer.allocUnsafe(4 + length);
        frame.writeUInt32LE(length);
        frame.write(json, 4, length, "utf8");
        return frame;
    } catch {
        throw new AlcomdSdkError("invalid_request", "The request is not bounded, lossless JSON.");
    }
}

export class FrameReader {
    #chunks;
    #remaining = Buffer.alloc(0);

    constructor(readable) {
        this.#chunks = readable[Symbol.asyncIterator]();
    }

    async #read(length, atFrameBoundary = false) {
        const output = Buffer.allocUnsafe(length);
        let offset = 0;
        while (offset < length) {
            if (this.#remaining.length === 0) {
                let chunk;
                try {
                    chunk = await this.#chunks.next();
                } catch {
                    throw new AlcomdSdkError("transport", "The SDK bridge connection was interrupted.");
                }
                if (chunk.done) {
                    if (atFrameBoundary && offset === 0) {
                        throw new AlcomdSdkError("transport", "The SDK bridge closed the connection.");
                    }
                    throw protocolError("The SDK bridge sent a truncated frame.");
                }
                this.#remaining = chunk.value;
            }
            const count = Math.min(length - offset, this.#remaining.length);
            this.#remaining.copy(output, offset, 0, count);
            offset += count;
            this.#remaining = this.#remaining.subarray(count);
        }
        return output;
    }

    async read() {
        const length = (await this.#read(4, true)).readUInt32LE();
        if (length === 0 || length > MAX_FRAME_BYTES) {
            throw protocolError("The SDK bridge sent an invalid frame length.");
        }
        const payload = await this.#read(length);
        try {
            return JSON.parse(DECODER.decode(payload), checkNumber);
        } catch {
            throw protocolError("The SDK bridge sent invalid or numerically unsafe JSON.");
        }
    }
}

function extractResponse(message, id) {
    if (!isRecord(message) || message.id !== id
        || Object.hasOwn(message, "result") === Object.hasOwn(message, "error")) {
        throw protocolError("The SDK bridge response does not match its request.");
    }
    if (Object.hasOwn(message, "result")) return message.result;
    const error = message.error;
    if (!isRecord(error) || !ERROR_KINDS.has(error.kind) || typeof error.message !== "string"
        || (error.kind === "remote" && typeof error.code !== "string")
        || (error.code !== undefined && typeof error.code !== "string")
        || (error.diagnosticId !== undefined && typeof error.diagnosticId !== "string")
        || (error.outcomeUnknown !== undefined && typeof error.outcomeUnknown !== "boolean")) {
        throw protocolError("The SDK bridge sent an invalid error.");
    }
    return new AlcomdSdkError(error.kind, error.message, error);
}

export function requestTimeout(value, fallback = 30_000) {
    const timeout = value ?? fallback;
    if (!Number.isInteger(timeout) || timeout <= 0 || timeout > MAX_TIMEOUT_MS) {
        throw new AlcomdSdkError("invalid_request", "timeoutMs must be a positive integer within the Node timer limit.");
    }
    return timeout;
}

export class FramedBridge {
    #child;
    #reader;
    #nextId = 1n;
    #pending;
    #closed = false;

    constructor(child) {
        this.#child = child;
        this.#reader = new FrameReader(child.stdout);
        child.on("error", () => this.close());
        // The write callback reports specific failures to the request. Some Node
        // stream errors are also emitted and must never become uncaught events.
        child.stdin.on("error", () => {});
        child.stdout.on("error", () => {});
    }

    async exchange(method, params, options = {}, mutates = false) {
        if (this.#closed) throw new AlcomdSdkError("transport", "The SDK bridge connection is closed.");
        if (this.#pending) {
            throw new AlcomdSdkError("invalid_request", "Only one request can run on this connection at a time.");
        }
        if (options.signal?.aborted) {
            throw new AlcomdSdkError("cancelled", "The request was cancelled before dispatch.");
        }
        const timeout = requestTimeout(options.timeoutMs);
        if (!isRecord(params)) throw new AlcomdSdkError("invalid_request", "RPC parameters must be an object.");
        const id = String(this.#nextId);
        const frame = encodeFrame({ id, method, params });
        if (options.signal?.aborted) {
            throw new AlcomdSdkError("cancelled", "The request was cancelled before dispatch.");
        }
        this.#nextId++;
        let stop;
        const interrupted = new Promise((_resolve, reject) => {
            stop = (kind, message) => reject(new AlcomdSdkError(kind, message, { outcomeUnknown: mutates }));
        });
        const abort = () => stop("cancelled", "The request was cancelled.");
        this.#pending = stop;
        options.signal?.addEventListener("abort", abort, { once: true });
        const timer = setTimeout(() => stop("timeout", "The request exceeded its deadline."), timeout);
        let response;
        try {
            const query = new Promise((resolve, reject) => {
                this.#child.stdin.write(frame, (error) => {
                    if (error) reject(new AlcomdSdkError("transport", "The SDK bridge could not send the request."));
                    else resolve();
                });
            }).then(() => this.#reader.read()).then((message) => extractResponse(message, id));
            response = await Promise.race([query, interrupted]);
        } catch (cause) {
            this.close();
            const error = cause instanceof AlcomdSdkError
                ? cause
                : new AlcomdSdkError("transport", "The SDK bridge could not complete the request.");
            if (mutates && !error.outcomeUnknown) {
                throw new AlcomdSdkError(error.kind, error.message, { ...error, outcomeUnknown: true });
            }
            throw error;
        } finally {
            this.#pending = undefined;
            clearTimeout(timer);
            options.signal?.removeEventListener("abort", abort);
        }
        if (response instanceof AlcomdSdkError) {
            if (response.kind === "transport" || response.kind === "protocol" || response.kind === "unavailable"
                || response.kind === "timeout" || response.kind === "cancelled") {
                this.close();
                if (mutates && !response.outcomeUnknown) {
                    throw new AlcomdSdkError(response.kind, response.message, { ...response, outcomeUnknown: true });
                }
            }
            throw response;
        }
        return response;
    }

    notification() {
        return this.#reader.read();
    }

    close() {
        if (this.#closed) return;
        this.#closed = true;
        this.#pending?.("transport", "The SDK bridge connection is closed.");
        this.#child.stdin.destroy();
        this.#child.stdout.destroy();
        if (this.#child.pid !== undefined && this.#child.exitCode === null && this.#child.signalCode === null) {
            this.#child.kill();
        }
    }
}

export async function openBridge(helperPath) {
    const executable = process.platform === "win32" ? "alcomd-sdk-bridge.exe" : "alcomd-sdk-bridge";
    const command = helperPath ?? fileURLToPath(new URL(`../bin/${executable}`, import.meta.url));
    if (typeof command !== "string" || !isAbsolute(command)) {
        throw new AlcomdSdkError("invalid_request", "The trusted helper override must be an absolute path.");
    }
    try {
        if (!(await stat(command)).isFile()) throw new Error();
        const child = spawn(command, [], { shell: false, windowsHide: true, stdio: ["pipe", "pipe", "ignore"] });
        const bridge = new FramedBridge(child);
        await new Promise((resolve, reject) => {
            child.once("spawn", resolve);
            child.once("error", reject);
        });
        return bridge;
    } catch {
        throw new AlcomdSdkError("unavailable", "The installed ALCOMD SDK bridge is unavailable.");
    }
}
