import { CircleCheck, CircleX, Clock3, Info, OctagonAlert } from "lucide-react";
import type React from "react";
import {
	memo,
	useCallback,
	useEffect,
	useLayoutEffect,
	useMemo,
	useRef,
	useState,
} from "react";
import { ScrollableCardTable } from "@/components/ScrollableCardTable";
import type {
	ActivityEntry,
	ActivityKind,
	ActivitySource,
	ActivityStatus,
} from "@/lib/bindings";
import i18next, { tc } from "@/lib/i18n";

type ActivityDetailRow = {
	id: string;
	key: string;
	value: React.ReactNode;
};

const ACTIVITY_LOG_RENDER_BATCH_SIZE = 80;
const ACTIVITY_LOG_SCROLL_END_THRESHOLD_PX = 160;
const ACTIVITY_COLUMNS = [
	{ head: "logs:time", width: "w-[17%]" },
	{ head: "logs:activity:source", width: "w-[10%]" },
	{ head: "logs:activity:status", width: "w-[13%]" },
	{ head: "logs:activity:summary", width: "w-[30%]" },
	{ head: "logs:activity:target", width: "w-[20%]" },
	{ head: "logs:activity:duration", width: "w-[10%]" },
] as const;

export const ActivityListCard = memo(function ActivityListCard({
	entries,
	resetKey,
	showDetails,
}: {
	entries: ActivityEntry[];
	resetKey: string;
	showDetails: boolean;
}) {
	const [visibleCount, setVisibleCount] = useState(
		ACTIVITY_LOG_RENDER_BATCH_SIZE,
	);
	const scrollContainerRef = useRef<HTMLDivElement>(null);
	const previousResetKeyRef = useRef(resetKey);
	useLayoutEffect(() => {
		if (previousResetKeyRef.current === resetKey) return;
		previousResetKeyRef.current = resetKey;
		setVisibleCount(ACTIVITY_LOG_RENDER_BATCH_SIZE);
		if (scrollContainerRef.current != null) {
			scrollContainerRef.current.scrollTop = 0;
		}
	}, [resetKey]);
	const visibleEntries = useMemo(
		() => entries.slice(0, visibleCount),
		[entries, visibleCount],
	);
	const showMoreEntries = useCallback(() => {
		setVisibleCount((current) => {
			if (current >= entries.length) return current;
			return Math.min(current + ACTIVITY_LOG_RENDER_BATCH_SIZE, entries.length);
		});
	}, [entries.length]);

	useEffect(() => {
		const container = scrollContainerRef.current;
		if (container == null || visibleCount >= entries.length) return;

		const handleScroll = () => {
			const remainingScroll =
				container.scrollHeight - container.scrollTop - container.clientHeight;
			if (remainingScroll <= ACTIVITY_LOG_SCROLL_END_THRESHOLD_PX) {
				showMoreEntries();
			}
		};

		container.addEventListener("scroll", handleScroll, { passive: true });
		const animationFrame = window.requestAnimationFrame(handleScroll);

		return () => {
			window.cancelAnimationFrame(animationFrame);
			container.removeEventListener("scroll", handleScroll);
		};
	}, [entries.length, showMoreEntries, visibleCount]);

	return (
		<ScrollableCardTable
			className={"h-full w-full"}
			tableClassName="table-fixed min-w-[60rem]"
			viewportRef={scrollContainerRef}
		>
			<colgroup>
				{ACTIVITY_COLUMNS.map((column) => (
					<col key={column.head} className={column.width} />
				))}
			</colgroup>
			<thead className={"w-full"}>
				<tr>
					{ACTIVITY_COLUMNS.map((column) => (
						<th
							key={column.head}
							className={
								"sticky top-0 z-10 border-b border-primary bg-secondary text-secondary-foreground p-2.5"
							}
						>
							<small className="font-normal leading-none">
								{tc(column.head)}
							</small>
						</th>
					))}
				</tr>
			</thead>
			<tbody>
				{entries.length === 0 ? (
					<tr>
						<td
							className="p-4 text-muted-foreground"
							colSpan={ACTIVITY_COLUMNS.length}
						>
							{tc("logs:activity:empty")}
						</td>
					</tr>
				) : (
					visibleEntries.map((entry) => (
						<tr key={entry.id} className="even:bg-secondary/30 align-top">
							<ActivityRow entry={entry} showDetails={showDetails} />
						</tr>
					))
				)}
			</tbody>
		</ScrollableCardTable>
	);
});

const ActivityRow = memo(function ActivityRow({
	entry,
	showDetails,
}: {
	entry: ActivityEntry;
	showDetails: boolean;
}) {
	const [detailsOpen, setDetailsOpen] = useState(false);
	const cellClass = "p-2.5 compact:py-1";

	return (
		<>
			<td className={cellClass}>{formatDate(entry.startedAt)}</td>
			<td className={cellClass}>
				<SourcePill source={entry.source} />
			</td>
			<td className={cellClass}>
				<StatusPill status={entry.status} />
			</td>
			<td className={`${cellClass} break-words`}>
				<div className="flex flex-col gap-1">
					<div className="font-normal text-primary">{activityTitle(entry)}</div>
					<div className="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
						<span>{kindLabel(entry.kind)}</span>
					</div>
					{showDetails ? (
						<details
							className="text-xs text-muted-foreground"
							open={detailsOpen}
							onToggle={(event) => setDetailsOpen(event.currentTarget.open)}
						>
							<summary className="cursor-pointer text-primary">
								{tc("logs:activity:details")}
							</summary>
							{detailsOpen ? <ActivityDetails entry={entry} /> : null}
						</details>
					) : null}
				</div>
			</td>
			<td className={`${cellClass} break-words`}>
				{entry.target ?? <span className="text-muted-foreground">-</span>}
			</td>
			<td className={cellClass}>{formatDuration(entry.durationMs)}</td>
		</>
	);
});

const ActivityDetails = memo(function ActivityDetails({
	entry,
}: {
	entry: ActivityEntry;
}) {
	const detailRows = withStableDetailIds([
		{
			key: "activity",
			value: operationLabel(entry.operation, entry.summary),
		},
		{ key: "operation", value: entry.operation },
		{ key: "kind", value: kindLabel(entry.kind) },
		{ key: "status", value: statusLabel(entry.status) },
		{ key: "source", value: sourceLabel(entry.source) },
		{ key: "startedAt", value: formatDate(entry.startedAt) },
		...(entry.finishedAt
			? [{ key: "finishedAt", value: formatDate(entry.finishedAt) }]
			: []),
		{ key: "duration", value: formatDuration(entry.durationMs) },
		...entry.details,
		...(entry.toolName ? [{ key: "toolName", value: entry.toolName }] : []),
		...(entry.requestId ? [{ key: "requestId", value: entry.requestId }] : []),
		...(entry.clientName
			? [{ key: "clientName", value: entry.clientName }]
			: []),
		...(entry.error ? [{ key: "error", value: entry.error }] : []),
	]);

	return (
		<dl className="mt-1 grid grid-cols-[max-content_minmax(12rem,1fr)] gap-x-3 gap-y-1 break-words">
			{detailRows.map((detail) => (
				<DetailItem
					key={`${entry.id}:${detail.id}`}
					name={detail.key}
					value={detail.value}
				/>
			))}
		</dl>
	);
});

function DetailItem({ name, value }: { name: string; value: React.ReactNode }) {
	const displayValue =
		typeof value === "string" ? detailValue(name, value) : value;

	return (
		<>
			<dt className="font-normal text-primary">{detailLabel(name)}</dt>
			<dd className="min-w-0">{displayValue}</dd>
		</>
	);
}

function withStableDetailIds(
	rows: Omit<ActivityDetailRow, "id">[],
): ActivityDetailRow[] {
	const counts = new Map<string, number>();

	return rows.map((row) => {
		const baseId = `${row.key}:${detailStableValue(row.value)}`;
		const count = counts.get(baseId) ?? 0;
		counts.set(baseId, count + 1);

		return {
			...row,
			id: count === 0 ? baseId : `${baseId}:${count}`,
		};
	});
}

function detailStableValue(value: React.ReactNode) {
	if (typeof value === "string" || typeof value === "number") {
		return value.toString();
	}

	return "localized";
}

function SourcePill({ source }: { source: ActivitySource }) {
	return (
		<span className="inline-flex items-center rounded-full bg-secondary px-2.5 py-1 text-xs text-secondary-foreground">
			{sourceLabel(source)}
		</span>
	);
}

function StatusPill({ status }: { status: ActivityStatus }) {
	const iconClass = "size-4";
	const label = statusLabel(status);

	switch (status) {
		case "Succeeded":
			return (
				<span className="inline-flex items-center gap-1.5 text-success">
					<CircleCheck className={iconClass} />
					{label}
				</span>
			);
		case "Failed":
			return (
				<span className="inline-flex items-center gap-1.5 text-destructive">
					<CircleX className={iconClass} />
					{label}
				</span>
			);
		case "Cancelled":
			return (
				<span className="inline-flex items-center gap-1.5 text-warning">
					<OctagonAlert className={iconClass} />
					{label}
				</span>
			);
		case "Started":
			return (
				<span className="inline-flex items-center gap-1.5 text-info">
					<Clock3 className={iconClass} />
					{label}
				</span>
			);
		default:
			return (
				<span className="inline-flex items-center gap-1.5">
					<Info className={iconClass} />
					{label}
				</span>
			);
	}
}

function formatDate(dateString: string) {
	return new Date(dateString).toLocaleString();
}

function formatDuration(durationMs: number | null) {
	if (durationMs == null) return "-";
	if (durationMs < 1000) return `${durationMs} ms`;
	return `${(durationMs / 1000).toFixed(1)} s`;
}

function sourceLabel(source: ActivitySource) {
	return tc(`logs:activity:source:${source}`);
}

function statusLabel(status: ActivityStatus) {
	return tc(`logs:activity:status:${status}`);
}

function kindLabel(kind: ActivityKind) {
	return tc(`logs:activity:kind:${kind}`);
}

function activityTitle(entry: ActivityEntry) {
	const summary = summaryLabel(entry.summary);
	if (summary != null) return summary;
	return operationLabel(entry.operation, entry.summary);
}

function summaryLabel(summary: string) {
	const key = activityTranslationKey(`summary:${summary}`);
	return i18next.exists(key) ? tc(key) : null;
}

function operationLabel(operation: string, fallback: string) {
	return localizedActivityText("operation", operation, fallback);
}

function detailLabel(detailKey: string) {
	return localizedActivityText("detail", detailKey, detailKey);
}

function detailValue(detailKey: string, value: string) {
	const specificKey = activityTranslationKey(
		`detail value:${detailKey}:${value}`,
	);
	if (i18next.exists(specificKey)) {
		return tc(specificKey);
	}

	const genericKey = activityTranslationKey(`detail value:${value}`);
	if (i18next.exists(genericKey)) {
		return tc(genericKey);
	}

	return value;
}

function localizedActivityText(
	category: "operation" | "detail",
	name: string,
	fallback: string,
) {
	const key = activityTranslationKey(`${category}:${name}`);
	return i18next.exists(key) ? tc(key) : fallback;
}

function activityTranslationKey(suffix: string) {
	return `logs:activity:${sanitizeActivityKey(suffix)}`;
}

function sanitizeActivityKey(value: string) {
	return value.replace(/[.\s-]/g, "_");
}
