import { useEffect, useMemo, useState } from "react";
import type { DiagnosticLogHealth, DiagnosticLogLevel, DiagnosticLogQuery } from "@alcomd/sdk";
import { Button, Checkbox, SearchField, Select } from "./Material";
import { DesktopList } from "./DesktopList";
import { Reminder, RequestReminder } from "./Reminder";
import { useTechnicalLogs } from "./diagnostic-log-ui";
import { diagnosticLogKey, logTimestamp } from "./diagnostic-log";
import { useTranslation } from "./i18n";
import type { GuiRpcClient } from "./rpc";

function HealthStatus({ label, health }: { label: string; health?: DiagnosticLogHealth }) {
    const { t } = useTranslation();
    if (health === undefined || health.state === "ready" && !health.hasGap) return null;
    if (health.state === "closed") return <Reminder title={label} message="The log source has stopped." />;
    return <Reminder title={label} tone="warning" details={<pre>{JSON.stringify(health, null, 2)}</pre>}>
        <p>{t("Log storage is degraded. Some records may be unavailable.")}</p>
        <dl className="logs-details">
            <dt>{t("Queue drops")}</dt><dd>{health.queueDropped}</dd>
            <dt>{t("Oversize records rejected")}</dt><dd>{health.oversizeRejected}</dd>
            <dt>{t("Write failures")}</dt><dd>{health.writeFailed}</dd>
            <dt>{t("Rotation failures")}</dt><dd>{health.rotationFailed}</dd>
            <dt>{t("Flush failures")}</dt><dd>{health.flushFailed}</dd>
        </dl>
        {health.lastFailure === undefined ? null : <p className="logs-message">{health.lastFailure.message}</p>}
    </Reminder>;
}
export function TechnicalLogs({ client }: { client: GuiRpcClient }) {
    const { t, locale } = useTranslation();
    const [search, setSearch] = useState("");
    const [appliedSearch, setAppliedSearch] = useState("");
    const [level, setLevel] = useState("");
    const [source, setSource] = useState("");
    const [details, setDetails] = useState(false);
    const searchInvalid = new TextEncoder().encode(search.trim()).byteLength > 4096;
    useEffect(() => {
        if (searchInvalid) return;
        const timer = setTimeout(() => setAppliedSearch(search.trim()), 200);
        return () => clearTimeout(timer);
    }, [search, searchInvalid]);
    const diagnosticSearch = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(appliedSearch);
    const query = useMemo<DiagnosticLogQuery>(() => ({ ...(appliedSearch ? diagnosticSearch ? { diagnosticId: appliedSearch } : { search: appliedSearch } : {}), ...(level ? { levels: [level as DiagnosticLogLevel] } : {}) }), [appliedSearch, diagnosticSearch, level]);
    const { core, gui, feeds } = useTechnicalLogs(client, query);
    const sources = [{ key: "core", label: "Core", state: core, feed: feeds.core }, { key: "gui", label: "GUI", state: gui, feed: feeds.gui }].filter((item) => source === "" || item.key === source);
    const records = sources.flatMap((item) => item.state.items.map((record) => ({ record, label: item.label }))).sort((a, b) => logTimestamp(b.record) - logTimestamp(a.record) || diagnosticLogKey(a.record).localeCompare(diagnosticLogKey(b.record)));
    return <div className="logs-workspace">
        <div className="alcomd-filter-bar logs-filter-toolbar" aria-label={t("Technical log filters")}>
            <SearchField className="logs-search" label={t("Search technical logs")} value={search} onInput={setSearch} error={searchInvalid} aria-invalid={searchInvalid} errorText={searchInvalid ? t("Search text is too long. Shorten it to apply the filter.") : undefined} />
            <Select label={t("Severity")} value={level} onSelect={setLevel} options={[{ value: "", label: t("All levels") }, ...["TRACE", "DEBUG", "INFO", "WARN", "ERROR"].map((value) => ({ value, label: t(value) }))]} />
            <Select label={t("Source")} value={source} onSelect={setSource} options={[{ value: "", label: t("All sources") }, { value: "core", label: "Core" }, { value: "gui", label: "GUI" }]} />
            <Checkbox label={t("Show details")} checked={details} onChange={setDetails} />
        </div>
        <p role="status">{t("Search covers the retained files for each source. Results are read in bounded pages.")} {t("Scanned bytes")}: {sources.reduce((sum, item) => sum + item.state.scannedBytes, 0)}</p>
        {sources.map(({ key, label, state, feed }) => <div key={key} className="logs-source-status" data-log-source={key}>
            <HealthStatus label={label} health={state.health} />
            {state.error === undefined ? null : <RequestReminder error={state.error} message={logReadErrorMessage(state.error.code)} title={t("{source} logs could not be read.", { source: label })}>
                <Button variant="text" type="button" onClick={() => void feed.retry()}>{t("Retry")}</Button>
            </RequestReminder>}
            {state.issues.length === 0 ? null : <Reminder tone="warning" details={<pre>{JSON.stringify(state.issues, null, 2)}</pre>}>{t("Some retained log records are damaged or no longer available.")}</Reminder>}
            {state.limited ? <p role="status">{t("The display limit was reached. Narrow the search to view other records.")}</p> : null}
        </div>)}
        {diagnosticSearch && records.length === 0 && sources.every(({ state }) => !state.loading && !state.hasMore && state.error === undefined) ? <p role="status">{t("No associated record was found in retained logs. It may not have been written or may have been rotated out.")}</p> : null}
        <DesktopList label={t("Technical Logs")} minWidth={760} sizing="fixed" columns={[{ key: "time", header: t("Time"), width: 210 }, { key: "level", header: t("Severity"), width: 100 }, { key: "source", header: t("Source"), width: 90 }, { key: "body", header: t("Log entry"), cellFlow: "wrap" }]}
            emptyMessage={t(sources.some(({ state }) => state.loading) ? "Loading…" : "No readable records loaded.")} rows={records.map(({ record: item, label }) => ({ key: diagnosticLogKey(item), cells: [
                item.record.timestamp && Number.isFinite(Date.parse(item.record.timestamp)) ? new Date(item.record.timestamp).toLocaleString(locale) : "—",
                item.record.level ? t(item.record.level) : "—", label,
                <div className="logs-summary" data-log-id={diagnosticLogKey(item)}><span className="logs-message">{typeof item.record.fields?.message === "string" ? item.record.fields.message : JSON.stringify(item.record.fields ?? {})}</span><small>{item.record.target}</small>{details ? <pre className="logs-message">{JSON.stringify(item.record, null, 2)}</pre> : null}</div>
            ] }))} />
        {sources.map(({ key, label, state, feed }) => !state.hasMore || state.limited ? null : <div key={key} className="pagination-controls"><Button disabled={state.loading} type="button" variant="tonal" onClick={feed.loadMore}>{t("Load more")} · {label}</Button></div>)}
    </div>;
}

function logReadErrorMessage(code: string): string | undefined {
    switch (code) {
        case "log_cursor_expired": return "Some saved logs have been rotated out. Reload the saved history to continue.";
        case "log_unavailable": return "Saved logs could not be loaded. The loaded entries are still shown.";
        case "daemon_unavailable": return "Core is unavailable. GUI logs remain available.";
        case "internal_error": return "The log read failed. Loaded records are preserved; retry to read again.";
        default: return undefined;
    }
}
