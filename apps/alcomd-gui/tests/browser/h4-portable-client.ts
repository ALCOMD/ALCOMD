import type { UiSnapshot } from "@alcomd/sdk";
import type { GuiRpcClient } from "../../src/rpc";

type HeldMethod = "open" | "identity" | "refresh" | "dispatch" | "settings";

interface PortableRequest {
    method: string;
    params?: unknown;
}

interface PendingPortableRead {
    method: HeldMethod;
    sessionId?: string;
    resolve(): void;
    reject(code: string): void;
}

interface PortableFixture {
    requests: PortableRequest[];
    pending: PendingPortableRead[];
    locale?: "en-US" | "zh-CN" | "ja-JP";
    removeUiCapability: boolean;
    hold(method: HeldMethod, held: boolean): void;
    release(method: HeldMethod, code?: string): boolean;
    releaseAll(method: HeldMethod): void;
    failNext(method: HeldMethod, code: string): void;
}

declare global {
    interface Window {
        h4Portable: PortableFixture;
    }
}

/** Public DTO fixture only; installed through the opt-in browser harness hook. */
export function withH4PortableClient(base: GuiRpcClient, query: URLSearchParams): GuiRpcClient {
    const held = new Set<HeldMethod>();
    const configuredHolds = query.get("h4-hold")?.split(",") ?? [];
    for (const method of configuredHolds) {
        if (method === "open" || method === "identity" || method === "refresh" || method === "dispatch" || method === "settings") held.add(method);
    }
    let failure: { method: HeldMethod; code: string } | undefined;
    let nextSession = 0;
    const sessions = new Map<string, { snapshot: UiSnapshot; extensionId: string; number: number }>();
    const identityReads = new Map<string, string>();
    const fixture: PortableFixture = {
        requests: [],
        pending: [],
        removeUiCapability: false,
        hold(method, value) { if (value) held.add(method); else held.delete(method); },
        release(method, code) {
            const index = fixture.pending.findIndex((pending) => pending.method === method);
            if (index < 0) return false;
            const pending = fixture.pending.splice(index, 1)[0]!;
            if (code === undefined) pending.resolve();
            else pending.reject(code);
            return true;
        },
        releaseAll(method) { while (fixture.release(method)) { /* Release the reads already held. */ } },
        failNext(method, code) { failure = { method, code }; }
    };
    window.h4Portable = fixture;

    function response<T>(method: HeldMethod, value: T, sessionId?: string): Promise<T> {
        if (failure?.method === method) {
            const code = failure.code;
            failure = undefined;
            return Promise.reject({ code, message: "Fixture request refused.", diagnosticId: "h4-portable-diagnostic" });
        }
        if (!held.has(method)) return Promise.resolve(value);
        return new Promise((resolve, reject) => {
            fixture.pending.push({
                method,
                ...(sessionId === undefined ? {} : { sessionId }),
                resolve: () => resolve(value),
                reject: (code) => reject({ code, message: "Fixture request refused." })
            });
        });
    }

    const methods: Partial<GuiRpcClient> = {
        async systemStatus() {
            fixture.requests.push({ method: "system.status" });
            const result = await base.systemStatus();
            return fixture.removeUiCapability
                ? { ...result, capabilities: result.capabilities.filter((capability) => capability !== "extensions.ui.portable.v1") }
                : result;
        },
        async settingsGet() {
            const result = await response("settings", await base.settingsGet());
            return fixture.locale === undefined ? result : { ...result, settings: { ...result.settings, locale: fixture.locale } };
        },
        async extensionGet(extensionId) {
            fixture.requests.push({ method: "extensions.get", params: { extensionId } });
            const result = await base.extensionGet(extensionId);
            const sessionId = identityReads.get(extensionId);
            if (sessionId === undefined) return result;
            identityReads.delete(extensionId);
            const session = sessions.get(sessionId);
            return response("identity", {
                ...result,
                extension: { ...result.extension, version: `1.0.${session?.number ?? 0}` }
            }, sessionId);
        },
        async extensionUiOpen(params) {
            fixture.requests.push({ method: "extensions.ui.open", params: structuredClone(params) });
            const result = await base.extensionUiOpen(params);
            const number = ++nextSession;
            const sessionId = `h4-session-${number}`;
            const snapshot = structuredClone(result.snapshot);
            snapshot.sessionId = sessionId;
            snapshot.snapshotRevision = 1;
            snapshot.document.title = `Fixture ${sessionId}`;
            const root = snapshot.document.nodes[0];
            if (root?.kind === "page") root.payload.title = `Fixture ${sessionId}`;
            sessions.set(sessionId, { snapshot, extensionId: params.extensionId, number });
            const opened = await response("open", {
                ...result,
                session: { ...result.session, sessionId },
                snapshot: structuredClone(snapshot)
            }, sessionId);
            identityReads.set(params.extensionId, sessionId);
            return opened;
        },
        extensionUiRefresh(params) {
            fixture.requests.push({ method: "extensions.ui.refresh", params: structuredClone(params) });
            const session = sessions.get(params.sessionId);
            if (session === undefined) return Promise.reject({ code: "extension_ui_session_not_found" });
            session.snapshot.snapshotRevision += 1;
            return response("refresh", { snapshot: structuredClone(session.snapshot) }, params.sessionId);
        },
        extensionUiDispatch(params) {
            fixture.requests.push({ method: "extensions.ui.dispatch", params: structuredClone(params) });
            const session = sessions.get(params.sessionId);
            if (session === undefined) return Promise.reject({ code: "extension_ui_session_not_found" });
            // A rejected action must not advance the snapshot in this fixture.
            if (failure?.method === "dispatch") return response("dispatch", { snapshot: structuredClone(session.snapshot), replayed: false }, params.sessionId);
            session.snapshot.snapshotRevision += 1;
            return response("dispatch", { snapshot: structuredClone(session.snapshot), replayed: false }, params.sessionId);
        },
        extensionUiClose(params) {
            fixture.requests.push({ method: "extensions.ui.close", params: structuredClone(params) });
            const session = sessions.get(params.sessionId);
            if (session !== undefined && identityReads.get(session.extensionId) === params.sessionId) identityReads.delete(session.extensionId);
            const closed = sessions.delete(params.sessionId);
            return Promise.resolve({ closed });
        }
    };
    return new Proxy(base, {
        get(target, property, receiver) {
            if (property in methods) return methods[property as keyof GuiRpcClient];
            const value: unknown = Reflect.get(target, property, receiver);
            return typeof value === "function" ? value.bind(target) : value;
        }
    });
}
