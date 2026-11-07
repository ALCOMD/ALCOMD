import type { LogFilter, LogOutcome, OperationLogType } from "@alcomd/sdk";
import { useState, type Ref, type UIEventHandler, type WheelEventHandler } from "react";

import { DesktopList } from "./DesktopList";
import { displayPath } from "./display-path";
import { useTranslation } from "./i18n";
import { Button, SearchField, Select } from "./Material";
import { PageHeader } from "./PageHeader";
import "./OperationLogPage.css";

/** Presentation only. Labels are resolved by the caller from actual Core records. */
export interface OperationLogRow {
    id: string;
    actor: string;
    occurredAtMs: number;
    operation: string;
    step: string;
    outcome: LogOutcome;
    target?: { name: string; path?: string };
    details?: string;
}

const outcomeLabels = {
    failed: "Failed",
    started: "Started",
    succeeded: "Succeeded",
    cancelled: "Cancelled",
    waiting: "Waiting"
} as const;

const typeLabels = {
    write: "Write",
    read: "Read",
    passive: "Passive",
    open: "Open",
    maintenance: "Maintenance"
} as const;

/** The data source owns history, synchronization and errors; this view never creates records. */
export function OperationLogPage({ entries, actors, filters, pending = false, error, onRetry, listRef, onScroll, onWheel, onFilterChange }: {
    entries: readonly OperationLogRow[];
    actors: readonly string[];
    filters: LogFilter;
    pending?: boolean;
    error?: string;
    onRetry?(): void;
    listRef?: Ref<HTMLDivElement>;
    onScroll?: UIEventHandler<HTMLDivElement>;
    onWheel?: WheelEventHandler<HTMLDivElement>;
    onFilterChange(filter: LogFilter): void;
}) {
    const { locale, t } = useTranslation();
    const time = new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeStyle: "medium" });
    const filtering = Boolean(filters.search?.trim() || filters.actor || filters.outcome || filters.type);
    return <section aria-labelledby="route-title" className="utility-workspace page-surface--utility operation-log-page">
        <PageHeader title={t("Logs")}>
            <SearchField className="operation-log-search" label={t("Search logs")} placeholder={t("Search logs")}
                value={filters.search ?? ""} onInput={(search) => onFilterChange({ ...filters, search: search || undefined })} />
        </PageHeader>
        <div className="alcomd-filter-bar operation-log-filters" role="group" aria-label={t("Log filters")}>
            <Select className="operation-log-select" label={t("Initiator")} value={filters.actor ?? ""}
                onSelect={(actor) => onFilterChange({ ...filters, actor: actor || undefined })}
                options={[{ value: "", label: t("All initiators") }, ...actors.map((actor) => ({ value: actor, label: actor }))]} />
            <Select className="operation-log-select" label={t("Status")} value={filters.outcome ?? ""}
                onSelect={(outcome) => onFilterChange({ ...filters, outcome: (outcome || undefined) as LogOutcome | undefined })}
                options={[{ value: "", label: t("All statuses") }, ...Object.entries(outcomeLabels).map(([value, label]) => ({ value, label: t(label) }))]} />
            <Select className="operation-log-select" label={t("Operation type")} value={filters.type ?? ""}
                onSelect={(type) => onFilterChange({ ...filters, type: (type || undefined) as OperationLogType | undefined })}
                options={[{ value: "", label: t("All types") }, ...Object.entries(typeLabels).map(([value, label]) => ({ value, label: t(label) }))]} />
        </div>
        <div aria-busy={pending} className="operation-log-list" onScrollCapture={onScroll} onWheel={onWheel} ref={listRef}>
            {error !== undefined && entries.length === 0 ? <div role="alert">
                <p>{error}</p>
                {onRetry === undefined ? null : <Button disabled={pending} onClick={onRetry} variant="text">{t("Retry")}</Button>}
            </div> : <DesktopList
                columns={[
                    { key: "time", header: t("Time"), width: "16%", cellFlow: "wrap" },
                    { key: "actor", header: t("Initiator"), width: "8%", cellFlow: "wrap" },
                    { key: "step", header: t("Operation and step"), width: "38%", cellFlow: "wrap" },
                    { key: "target", header: t("Target"), width: "28%", cellFlow: "wrap" },
                    { key: "result", header: t("Result"), width: "10%", cellFlow: "wrap" }
                ]}
                emptyMessage={pending ? t("Loading…") : filtering ? t("No matching logs") : t("No logs yet")}
                label={t("Logs")}
                rows={entries.map((entry) => ({
                    key: entry.id,
                    cells: [
                        <time dateTime={new Date(entry.occurredAtMs).toISOString()}>{time.format(entry.occurredAtMs)}</time>,
                        entry.actor,
                        <LogStep entry={entry} />,
                        entry.target === undefined ? "—" : <div className="operation-log-lines">
                            <span>{entry.target.name}</span>
                            {entry.target.path === undefined ? null : <small title={displayPath(entry.target.path)}>{displayPath(entry.target.path)}</small>}
                        </div>,
                        t(outcomeLabels[entry.outcome])
                    ]
                }))}
                sizing="fixed"
            />}
        </div>
    </section>;
}

function LogStep({ entry }: { entry: OperationLogRow }) {
    const { t } = useTranslation();
    const [expanded, setExpanded] = useState(false);
    const detailsId = `operation-log-details-${entry.id}`;
    return <div className="operation-log-lines">
        <strong>{entry.operation}</strong>
        <span>{entry.step}</span>
        {entry.details === undefined ? null : <>
            <Button aria-controls={detailsId} aria-expanded={expanded} onClick={() => setExpanded(!expanded)} variant="text">
                {t(expanded ? "Hide details" : "Details")}
            </Button>
            {expanded ? <pre id={detailsId}>{entry.details}</pre> : null}
        </>}
    </div>;
}
