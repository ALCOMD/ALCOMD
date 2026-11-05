import { useState } from "react";

import { LocaleProvider, type SupportedLocale } from "../../src/i18n";
import { OperationLogPage, type OperationLogRow } from "../../src/OperationLogPage";

declare global {
    interface Window {
        logPreview: {
            replace(entries: OperationLogRow[]): void;
            fail(message: string): void;
            retries: number;
        };
    }
}

const path = "C:\\ALCOMD-Test\\项目A\\Packages\\vpm-manifest.json";
const samples: OperationLogRow[] = [
    { id: "sample-4", actor: "GUI", occurredAtMs: 1793937603000, operation: "添加项目", step: "读取项目清单", outcome: "failed", target: { name: "项目A", path }, details: `Failed to read project manifest: ${path}\ncaused by: access denied (os error 5)` },
    { id: "sample-3", actor: "MCP", occurredAtMs: 1793937602000, operation: "创建备份", step: "备份已保存", outcome: "succeeded", target: { name: "项目A", path: "C:\\ALCOMD-Test\\备份\\项目A.zip" } },
    { id: "sample-2", actor: "MCP", occurredAtMs: 1793937601000, operation: "创建备份", step: "写入归档", outcome: "started", target: { name: "项目A", path: "C:\\ALCOMD-Test\\项目A" } },
    { id: "sample-1", actor: "CLI", occurredAtMs: 1793937600000, operation: "创建备份", step: "检查项目", outcome: "succeeded", target: { name: "项目A", path: "C:\\ALCOMD-Test\\项目A" } }
];

/** Explicitly a view preview, not a replacement for the future Core IPC fixture. */
export function OperationLogEvidence({ locale = "zh-CN" }: { locale?: SupportedLocale }) {
    const [entries, setEntries] = useState(samples);
    const [error, setError] = useState<string>();
    window.logPreview = {
        replace: setEntries,
        fail: setError,
        retries: window.logPreview?.retries ?? 0
    };
    return <LocaleProvider locale={locale}>
        <main style={{ height: "100vh", padding: 24, display: "flex", flexDirection: "column", gap: 12 }}>
            <p style={{ margin: 0 }}>界面样例，尚未接入 Core</p>
            <OperationLogPage entries={entries} error={error} onRetry={() => { window.logPreview.retries++; setError(undefined); }} />
        </main>
    </LocaleProvider>;
}
