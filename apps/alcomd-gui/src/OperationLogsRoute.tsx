import type { LogFilter, OperationLogEntry } from "@alcomd/sdk";
import { useCallback, useEffect, useId, useLayoutEffect, useRef, useState, useSyncExternalStore } from "react";

import { useReconnect } from "./capabilities";
import { useCoreEvents, useCoreInvalidation } from "./core-events-ui";
import type { EventConnectionState } from "./core-events";
import { useTranslation, type Translate } from "./i18n";
import { OperationLogPage } from "./OperationLogPage";
import { normalizeRequestError, requestErrorMessage } from "./request-errors";
import type { GuiRpcClient } from "./rpc";

type Direction = "refresh" | "older" | "newer";
interface LogWindow { entries: OperationLogEntry[]; older: boolean; newer: boolean }
interface Anchor { sequence: number; id: string; top: number }
const empty: LogWindow = { entries: [], older: false, newer: false };
const scopes = ["operation-log"] as const;
const EDGE = 160;
const MAX_ROWS = 500;
const MAX_BYTES = 8 * 1024 * 1024;
const encoder = new TextEncoder();
const ready: EventConnectionState = { kind: "ready" };
const noSubscription = () => () => {};
const readReady = () => ready;

function scrollViewport(root: HTMLElement | null): HTMLElement | null {
    return root?.querySelector<HTMLElement>("[data-overlayscrollbars-viewport]")
        ?? root?.querySelector<HTMLElement>(".desktop-list-viewport") ?? null;
}

function readingAnchor(viewport: HTMLElement | null, preserveTop = false): Anchor | undefined {
    if (!viewport || (!preserveTop && viewport.scrollTop < 24)) return;
    const top = viewport.getBoundingClientRect().top + 42;
    for (const row of viewport.querySelectorAll<HTMLElement>("tr[data-row-key]")) {
        const bounds = row.getBoundingClientRect();
        if (bounds.bottom <= top || !row.dataset.rowKey) continue;
        const [sequence] = row.dataset.rowKey.split(":", 1);
        return { sequence: Number(sequence), id: row.dataset.rowKey, top: bounds.top };
    }
}

/** Keep a contiguous, bounded window containing the reading anchor. */
function limitWindow(entries: OperationLogEntry[], direction: Direction, anchor?: Anchor): LogWindow {
    const bytes = entries.map((entry) => encoder.encode(JSON.stringify(entry)).length);
    const found = anchor === undefined ? -1 : entries.findIndex((entry) => entry.sequence === anchor.sequence);
    const center = found < 0 ? (direction === "older" ? entries.length - 1 : 0) : found;
    if (!entries.length) return empty;
    let first = center;
    let end = center + 1;
    let total = bytes[center] ?? 0;
    const extend = (towardOlder: boolean) => {
        while (end - first < MAX_ROWS) {
            const index = towardOlder ? end : first - 1;
            const size = bytes[index];
            if (size === undefined || total + size > MAX_BYTES) break;
            total += size;
            if (towardOlder) end++; else first--;
        }
    };
    extend(direction !== "newer");
    extend(direction === "newer");
    return { entries: entries.slice(first, end), older: end < entries.length, newer: first > 0 };
}

export function OperationLogsRoute({ client }: { client: GuiRpcClient }) {
    const { t } = useTranslation();
    const events = useCoreEvents();
    const reconnect = useReconnect();
    const connection = useSyncExternalStore(events?.subscribe ?? noSubscription, events?.getSnapshot ?? readReady);
    const id = useId();
    const root = useRef<HTMLDivElement>(null);
    const generation = useRef(0);
    const initialized = useRef(false);
    const active = useRef<Promise<void> | undefined>(undefined);
    const state = useRef<LogWindow>(empty);
    const failedDirection = useRef<Direction | undefined>(undefined);
    const restore = useRef(events?.metadata<number | undefined>("operation-log", "sequence", undefined));
    const anchor = useRef<Anchor | undefined>(undefined);
    const filter = useRef<LogFilter>({});
    const filterTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
    const [model, setModel] = useState<LogWindow>(empty);
    const [pending, setPending] = useState(true);
    const [error, setError] = useState<string>();
    const [filters, setFilters] = useState<LogFilter>({});
    const [actors, setActors] = useState<string[]>([]);
    const filtering = Boolean(filters.search?.trim() || filters.actor || filters.outcome || filters.type);

    const apply = useCallback((next: LogWindow) => {
        state.current = next;
        setModel(next);
    }, []);
    const read = useCallback(async (direction: Direction) => {
        const epoch = generation.current;
        while (active.current && generation.current === epoch) {
            if (direction !== "refresh") return;
            await active.current.catch(() => {});
        }
        if (generation.current !== epoch || root.current === null || filterTimer.current !== undefined) return;
        const current = state.current;
        const viewport = scrollViewport(root.current);
        if (direction === "refresh" && !failedDirection.current && initialized.current && (current.newer || (viewport?.scrollTop ?? 0) > EDGE)) {
            // Keep the reader's window still. The next scroll toward newer data
            // reads the adjacent range from Core; no incoming records are cached here.
            apply({ ...current, newer: true });
            return;
        }
        if (direction === "older" && !current.older || direction === "newer" && !current.newer) return;
        setPending(true);
        const request = async () => {
            const restoring = direction === "refresh" && !initialized.current && restore.current !== undefined && restore.current < Number.MAX_SAFE_INTEGER;
            const params = direction === "older" ? { beforeSequence: current.entries.at(-1)?.sequence }
                : direction === "newer" ? { afterSequence: current.entries[0]?.sequence ?? 0 }
                : restoring ? { beforeSequence: restore.current! + 1 } : {};
            const page = await client.logsRead({ ...params, filter: filter.current });
            if (generation.current !== epoch) return;
            if (page.highWatermark < (current.entries[0]?.sequence ?? 0)) throw { code: "event_protocol_error", message: "" };
            const incoming = [...page.entries].reverse();
            const remembered = readingAnchor(scrollViewport(root.current), direction === "newer");
            const adjacent = direction !== "refresh" || incoming.some((entry) => current.entries.some((row) => row.entryId === entry.entryId));
            const unique = new Map((adjacent ? current.entries : []).map((entry) => [entry.entryId, entry]));
            incoming.forEach((entry) => unique.set(entry.entryId, entry));
            const combined = [...unique.values()].sort((a, b) => b.sequence - a.sequence);
            const limited = limitWindow(combined, direction, remembered);
            anchor.current = remembered;
            initialized.current = true;
            setActors(page.actors);
            apply({
                entries: limited.entries,
                older: limited.older || (direction === "newer" ? current.older : page.hasMore || (direction === "refresh" && adjacent && current.older)),
                newer: limited.newer || (direction === "older" ? current.newer : direction === "newer" && page.hasMore)
                    || (restoring && page.highWatermark > (limited.entries[0]?.sequence ?? 0))
            });
            events?.noteDisplayedData();
            failedDirection.current = undefined;
            setError(undefined);
        };
        const promise = request();
        active.current = promise;
        try { await promise; }
        catch (cause) {
            if (generation.current === epoch) {
                failedDirection.current = direction;
                setError(normalizeRequestError(cause).code);
            }
            throw cause;
        } finally {
            if (active.current === promise) active.current = undefined;
            if (generation.current === epoch) setPending(false);
        }
    }, [apply, client, events]);
    // A ready connection can start the initial read during registration.
    useEffect(() => {
        ++generation.current;
        if (!events) void read("refresh").catch(() => {});
        return () => {
            ++generation.current;
            clearTimeout(filterTimer.current);
            filterTimer.current = undefined;
        };
    }, [events, read]);
    const retry = useCoreInvalidation(`operation-logs:${id}`, scopes, () => read(failedDirection.current ?? "refresh"), true);

    const changeFilter = (next: LogFilter) => {
        ++generation.current;
        filter.current = next;
        setFilters(next);
        initialized.current = false;
        failedDirection.current = undefined;
        restore.current = undefined;
        anchor.current = undefined;
        apply(empty);
        setError(undefined);
        setPending(true);
        const viewport = scrollViewport(root.current);
        if (viewport) viewport.scrollTop = 0;
        clearTimeout(filterTimer.current);
        filterTimer.current = setTimeout(() => {
            filterTimer.current = undefined;
            void read("refresh").catch(() => {});
        }, 150);
    };

    const readAtEdge = useCallback((viewport: HTMLElement) => {
        if (active.current) return;
        if (viewport.scrollTop <= EDGE && state.current.newer) void read("newer").catch(() => {});
        else if (viewport.scrollHeight - viewport.clientHeight - viewport.scrollTop <= EDGE && state.current.older) void read("older").catch(() => {});
    }, [read]);
    useLayoutEffect(() => {
        const viewport = scrollViewport(root.current);
        const before = anchor.current;
        anchor.current = undefined;
        if (!viewport) return;
        if (before) {
            const row = viewport.querySelector<HTMLElement>(`tr[data-row-key="${CSS.escape(before.id)}"]`);
            if (row) viewport.scrollTop += row.getBoundingClientRect().top - before.top;
        }
        if (!pending && !error && viewport.scrollHeight <= viewport.clientHeight + 4) readAtEdge(viewport);
    }, [error, model, pending, readAtEdge]);

    const disconnected = connection.kind === "disconnected" || connection.kind === "failed";
    const problem = error ?? (disconnected ? connection.error?.code ?? "daemon_unavailable" : undefined);
    const errorMessage = problem === undefined || initialized.current ? undefined : t(requestErrorMessage(problem));
    return <OperationLogPage entries={model.entries.map((entry) => {
        return {
            id: `${entry.sequence}:${entry.entryId}`, actor: entry.actor, occurredAtMs: entry.occurredAtMs,
            operation: label(t, actions, entry.action), step: label(t, steps, entry.step), outcome: entry.outcome,
            target: entry.target === undefined ? undefined : {
                name: entry.target.name ?? entry.target.path?.split(/[\\/]/).filter(Boolean).at(-1) ?? label(t, targets, entry.target.kind),
                path: entry.target.path
            },
            details: entry.details
        };
    })} actors={actors} filters={filters} pending={pending && !disconnected} error={errorMessage} listRef={root} onFilterChange={changeFilter}
        onRetry={() => { if (disconnected || error === "daemon_unavailable") reconnect?.(); retry(); }}
        onWheel={(event) => {
            const viewport = scrollViewport(root.current);
            if (event.deltaY < 0 && viewport && viewport.scrollTop <= EDGE && state.current.newer) readAtEdge(viewport);
        }}
        onScroll={(event) => {
            if (!(event.target instanceof HTMLElement)) return;
            const position = readingAnchor(event.target)?.sequence ?? (state.current.newer ? state.current.entries[0]?.sequence : undefined);
            if (!filtering) events?.setMetadata("operation-log", "sequence", position);
            readAtEdge(event.target);
        }} />;
}

const label = (t: Translate, map: Readonly<Record<string, string>>, key: string) => t((Object.hasOwn(map, key) ? map[key] : undefined) ?? key);
const actions = {
    "projects.register": "Add project",
    "projects.refresh": "Read project content",
    "projects.unregister": "Remove project registration",
    "projects.setFavorite": "Change project favorite",
    "repositories.register": "Add repository",
    "repositories.refresh": "Fetch repository",
    "repositories.unregister": "Remove repository",
    "repositories.exportListFile": "Export repository list",
    "packages.apply": "Change packages",
    "packages.bulk": "Change packages",
    "packages.install": "Install packages",
    "packages.remove": "Remove packages",
    "packages.upgrade": "Upgrade packages",
    "packages.downgrade": "Downgrade packages",
    "packages.resolve": "Resolve packages",
    "packages.reinstall": "Reinstall packages",
    "packages.userPackages.enroll": "Add user package",
    "packages.userPackages.refresh": "Read user package content",
    "packages.userPackages.remove": "Remove user package registration",
    "backups.create": "Create backup",
    "backups.restore": "Restore backup",
    "projects.create": "Create project",
    "projectTemplates.save": "Save template",
    "projectTemplates.remove": "Remove template",
    "projectTemplates.export": "Export template",
    "projects.copy": "Copy project",
    "projects.delete-directory": "Delete project directory",
    "projects.unity-migration": "Change project Unity version",
    "extensions.install": "Install extension",
    "extensions.uninstall": "Uninstall extension",
    "state.check": "Check runtime status",
    "extensions.enable": "Enable extension",
    "extensions.disable": "Disable extension",
    "extensions.permission": "Change extension permissions",
    "unity.launch": "Request Unity launch",
    "unity.hub.open": "Open Unity Hub",
    "unity.projectLaunchConfig": "Change Unity launch arguments",
    "unity.installations.register": "Add Unity installation",
    "unity.installations.remove": "Remove Unity installation",
    "unity.installations.refresh": "Find Unity installations",
    "settings.update": "Save settings"
};
const steps = {
    inspect_project: "Check project files", fetch_repository: "Fetch repository", registered: "Save registration",
    updated: "Save changes", refreshed: "Refresh complete", no_changes: "No changes found", favorited: "Add to favorites", unfavorited: "Remove from favorites",
    unregistered: "Remove registration", accepted: "Accept operation", execute: "Start operation", completed: "Completed",
    failed: "Failed", cancelled: "Cancelled", request_cancel: "Request cancellation", recover: "Resume operation", recovered: "Recovery complete",
    interrupted: "Interrupted", waiting_for_input: "Waiting for input", queued: "Queued", plan: "Prepare plan", planned: "Save plan",
    archiving: "Write archive", verifying: "Verify files", extracting: "Extract files", prepared: "Preparation complete",
    prepare_destination: "Prepare destination", check_destination: "Check destination", check_backup: "Check backup archive",
    inspect_package: "Check package files",
    resolve_dependencies: "Resolve packages", check_plan: "Check plan", check_editor: "Check Unity Editor", find_editors: "Find Unity installations",
    installations_synchronized: "Save installation list", launch_requested: "Send launch request", hub_not_installed: "Unity Hub not found",
    open_hub: "Open Unity Hub", saved: "Save changes", cleared: "Clear saved value", write_repository_list: "Write repository list",
    enable_requested: "Request enable", disable_requested: "Request disable", granted: "Grant permission", revoked: "Revoke permission",
    preflight_complete: "Checks complete", inventory_ready: "File inventory ready", archive_ready: "Archive ready",
    archive_verified: "Verify archive", archive_published: "Save archive", publish_intent: "Prepare publication",
    staging: "Prepare files", staging_complete: "Preparation complete", target_published: "Save destination files",
    project_registry_commit_intent: "Prepare registration", registry_commit_intent: "Prepare registry change",
    state_committed: "Save changes", cleanup_complete: "Cleanup complete", recovery_required: "Recovery required",
    quarantine_intent: "Prepare removal", root_quarantined: "Isolate project directory", deleting: "Delete files",
    preparation_intent: "Prepare project", preparation_complete: "Preparation complete", launch_intent: "Prepare Unity launch",
    unity_started: "Unity process started", unity_exited: "Unity process exited", project_reobserved: "Recheck project files",
    extracted: "Extract files", packages_replaced: "Replace packages", vpm_manifest_committed: "Save VPM manifest", filesystem_committed: "Save file changes"
};
const targets = { project: "Project", repository: "Repository", file: "File", directory: "Folder", extension: "Extension", "project-template": "Template", unity: "Unity", package: "Package", backup: "Backup" };
