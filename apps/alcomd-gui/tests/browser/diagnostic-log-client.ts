import type { DiagnosticLogItem, DiagnosticLogReadParams, DiagnosticLogReadResult } from "@alcomd/sdk";

export const DIAGNOSTIC_LOG_FIXTURE_IDS = {
    coreA: "00000000-0000-4000-8000-000000009100",
    coreB: "00000000-0000-4000-8000-000000009101",
    historic: "00000000-0000-4000-8000-000000009200",
    restarted: "00000000-0000-4000-8000-000000009201",
    newLog: "00000000-0000-4000-8000-000000009202",
    quietWarning: "00000000-0000-4000-8000-000000009203",
    quietError: "00000000-0000-4000-8000-000000009204",
    quietInformation: "00000000-0000-4000-8000-000000009205",
    visibleWarning: "00000000-0000-4000-8000-000000009206",
    navigation: "00000000-0000-4000-8000-000000009207",
    modal: "00000000-0000-4000-8000-000000009208",
    resumed: "00000000-0000-4000-8000-000000009209",
    oldInstance: "00000000-0000-4000-8000-000000009210",
    newInstance: "00000000-0000-4000-8000-000000009211"
} as const;

declare global {
    interface Window {
        diagnosticLogRequests: DiagnosticLogReadParams[];
        diagnosticLogCancels: number;
    }
}

export class BrowserDiagnosticLog {
    private readonly enabled: boolean;
    private initialFailure: boolean;
    private instance: string = DIAGNOSTIC_LOG_FIXTURE_IDS.coreA;
    private items: DiagnosticLogItem[] = [{ instanceId: this.instance, sequence: 1, occurredAtMs: 1_700_000_001_000, level: "error", code: "diagnostic.error", diagnosticId: DIAGNOSTIC_LOG_FIXTURE_IDS.historic, notify: true }];
    private pending?: { params: DiagnosticLogReadParams; resolve(result: DiagnosticLogReadResult): void; reject(error: unknown): void };

    constructor(enabled: boolean, initialFailure = false) {
        this.enabled = enabled;
        this.initialFailure = initialFailure;
        window.diagnosticLogRequests = [];
        window.diagnosticLogCancels = 0;
        window.addEventListener("test-diagnostic-log", ((event: CustomEvent<Partial<DiagnosticLogItem>>) => {
            const level = event.detail.level ?? "error";
            this.items.push({ instanceId: this.instance, sequence: this.items.length + 1, occurredAtMs: 1_700_000_001_000 + this.items.length,
                level, code: event.detail.code ?? `diagnostic.${level}`, diagnosticId: event.detail.diagnosticId ?? `00000000-0000-4000-8000-${String(9300 + this.items.length + 1).padStart(12, "0")}`,
                notify: event.detail.notify ?? level === "error" });
            this.flush();
        }) as EventListener);
        window.addEventListener("test-diagnostic-log-failure", () => { this.pending?.reject({ code: "daemon_unavailable", message: "never render raw failure detail" }); this.pending = undefined; });
        window.addEventListener("test-diagnostic-log-reset", () => {
            this.instance = DIAGNOSTIC_LOG_FIXTURE_IDS.coreB;
            this.items = [{ instanceId: this.instance, sequence: 1, occurredAtMs: 1_700_000_002_000, level: "error", code: "diagnostic.error", diagnosticId: DIAGNOSTIC_LOG_FIXTURE_IDS.restarted, notify: true }];
            this.flush();
        });
    }

    read(params: DiagnosticLogReadParams): Promise<DiagnosticLogReadResult> {
        window.diagnosticLogRequests.push(structuredClone(params));
        if (!this.enabled) return Promise.reject({ code: "method_not_found" });
        if (this.initialFailure) { this.initialFailure = false; return Promise.reject({ code: "daemon_unavailable" }); }
        const response = this.result(params);
        if (params.cursor === undefined || response.reset || response.items.length !== 0) return Promise.resolve(response);
        return new Promise((resolve, reject) => { this.pending = { params, resolve, reject }; });
    }

    async cancel(): Promise<void> {
        window.diagnosticLogCancels += 1;
        this.pending?.reject({ code: "daemon_unavailable" });
        this.pending = undefined;
    }

    private result(params: DiagnosticLogReadParams): DiagnosticLogReadResult {
        const reset = params.cursor !== undefined && params.cursor.instanceId !== this.instance;
        const items = params.cursor === undefined || reset ? this.items : this.items.filter((item) => item.sequence > params.cursor!.sequence);
        return { items: structuredClone(items), nextCursor: { instanceId: this.instance, sequence: this.items.length }, reset, gap: false };
    }

    private flush() {
        const pending = this.pending;
        if (pending === undefined) return;
        this.pending = undefined;
        pending.resolve(this.result(pending.params));
    }
}
