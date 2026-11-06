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
    type?: OperationLogType;
    step: string;
    outcome: "started" | "succeeded" | "failed" | "cancelled" | "waiting";
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

export type OperationLogType = keyof typeof typeLabels;

interface LogFilters {
    search: string;
    actor: string;
    outcome: string;
    type: string;
}

function hasFilters(filters: LogFilters) {
    return Boolean(filters.search.trim() || filters.actor || filters.outcome || filters.type);
}

/** The data source owns history, synchronization and errors; this view never creates records. */
export function OperationLogPage({ entries, pending = false, error, onRetry, listRef, onScroll, onWheel, onFilterChange }: {
    entries: readonly OperationLogRow[];
    pending?: boolean;
    error?: string;
    onRetry?(): void;
    listRef?: Ref<HTMLDivElement>;
    onScroll?: UIEventHandler<HTMLDivElement>;
    onWheel?: WheelEventHandler<HTMLDivElement>;
    onFilterChange?(active: boolean): void;
}) {
    const { locale, t } = useTranslation();
    const [filters, setFilters] = useState<LogFilters>({ search: "", actor: "", outcome: "", type: "" });
    const time = new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeStyle: "medium" });
    const filtering = hasFilters(filters);
    const query = filters.search.trim().toLocaleLowerCase(locale);
    const actors = new Set(entries.map(({ actor }) => actor));
    if (filters.actor) actors.add(filters.actor);
    const visible = entries.filter((entry) => {
        if (filters.actor && entry.actor !== filters.actor) return false;
        if (filters.outcome && entry.outcome !== filters.outcome) return false;
        if (filters.type && entry.type !== filters.type) return false;
        if (!query) return true;
        const path = entry.target?.path;
        return [entry.actor, entry.operation, entry.step, entry.target?.name, path,
            path && displayPath(path), entry.details, t(outcomeLabels[entry.outcome]), entry.type && t(typeLabels[entry.type])]
            .some((value) => value?.toLocaleLowerCase(locale).includes(query));
    });
    const updateFilters = (next: LogFilters) => {
        setFilters(next);
        onFilterChange?.(hasFilters(next));
    };
    return <section aria-labelledby="route-title" className="utility-workspace page-surface--utility operation-log-page">
        <PageHeader title={t("Logs")}>
            <SearchField className="operation-log-search" label={t("Search loaded logs")} placeholder={t("Search loaded logs")}
                value={filters.search} onInput={(search) => updateFilters({ ...filters, search })} />
        </PageHeader>
        <div className="alcomd-filter-bar operation-log-filters" role="group" aria-label={t("Log filters")}>
            <Select className="operation-log-select" label={t("Initiator")} value={filters.actor}
                onSelect={(actor) => updateFilters({ ...filters, actor })}
                options={[{ value: "", label: t("All initiators") }, ...[...actors].sort().map((actor) => ({ value: actor, label: actor }))]} />
            <Select className="operation-log-select" label={t("Status")} value={filters.outcome}
                onSelect={(outcome) => updateFilters({ ...filters, outcome })}
                options={[{ value: "", label: t("All statuses") }, ...Object.entries(outcomeLabels).map(([value, label]) => ({ value, label: t(label) }))]} />
            <Select className="operation-log-select" label={t("Operation type")} value={filters.type}
                onSelect={(type) => updateFilters({ ...filters, type })}
                options={[{ value: "", label: t("All types") }, ...Object.entries(typeLabels).map(([value, label]) => ({ value, label: t(label) }))]} />
        </div>
        <div aria-busy={pending} className="operation-log-list" onScrollCapture={filtering ? undefined : onScroll}
            onWheel={filtering ? undefined : onWheel} ref={listRef}>
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
                emptyMessage={pending ? t("Loading…") : filtering ? t("No matching logs in the loaded records") : t("No logs yet")}
                label={t("Logs")}
                rows={visible.map((entry) => ({
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
