import { useEffect, useRef, useState } from "react";
import type { RepositoryListEntry, RepositoryListInspection, RepositoryListPreview, RepositoryListExportResult, RpcError } from "@alcomd/sdk";
import type { GuiRpcClient } from "./rpc";
import { RegisterRepositoryPanel, useActionScope } from "./CoreActions";
import { Button, Checkbox, Dialog, MenuItem, Progress, TextField } from "./Material";
import { SplitButton } from "./SplitButton";
import { UtilityActionDialog } from "./UtilityActionDialog";
import { Reminder, RequestReminder } from "./Reminder";
import { ScrollArea } from "./ScrollArea";
import { useTranslation } from "./i18n";
import { capabilities, capabilityUnavailableTitle, useCapability } from "./capabilities";
import "./RepositoryListDialog.css";

export function RepositoryToolbar({ client, onChanged }: { client: GuiRpcClient; onChanged(): void }) {
    const { t } = useTranslation();
    const manage = useCapability(capabilities.repositoriesRegistry);
    const files = useCapability(capabilities.repositoriesListFiles);
    const [dialog, setDialog] = useState<"add" | "import" | "export">();
    const menu = useRef<HTMLElement | null>(null);
    const primary = useRef<HTMLElement | null>(null);
    const previous = useRef<typeof dialog>(undefined);
    useEffect(() => {
        if (previous.current !== undefined && dialog === undefined) (previous.current === "add" ? primary : menu).current?.focus();
        previous.current = dialog;
    }, [dialog]);
    const close = () => setDialog(undefined);
    return <>
        <SplitButton disabled={!manage} label={t("Add repository")} menuButtonRef={menu} primaryRef={primary} menuLabel={t("Repository list actions")} onClick={() => setDialog("add")} title={capabilityUnavailableTitle(manage, capabilities.repositoriesRegistry, t)}>
            <MenuItem disabled={!files || !manage} label={t("Import list")} onClick={() => setDialog("import")} title={capabilityUnavailableTitle(files, capabilities.repositoriesListFiles, t)} />
            <MenuItem disabled={!files} label={t("Export list")} onClick={() => setDialog("export")} title={capabilityUnavailableTitle(files, capabilities.repositoriesListFiles, t)} />
        </SplitButton>
        {dialog === "add" ? <UtilityActionDialog title={t("Add repository")} onClose={close}><RegisterRepositoryPanel client={client} onChanged={onChanged} /></UtilityActionDialog> : null}
        {dialog === "import" ? <ImportRepositoryList client={client} onChanged={onChanged} onClose={close} /> : null}
        {dialog === "export" ? <ExportRepositoryList client={client} onClose={close} /> : null}
    </>;
}

type ItemPhase = "pending" | "checking" | "ready" | "failed" | "registering" | "registered" | "unknown" | "duplicate";
interface ItemResult { phase: ItemPhase; error?: RpcError }
const safeError = (caught: unknown): RpcError => ({ code: typeof caught === "object" && caught !== null && "code" in caught && typeof caught.code === "string" ? caught.code : "internal_error", message: "Repository list request failed" });
const definiteRejection = new Set(["repository_already_registered", "repository_source_invalid", "repository_inaccessible", "repository_unavailable", "repository_document_invalid", "repository_document_too_large", "repository_credentials_unsupported", "permission_denied", "invalid_request", "capability_required"]);

function ImportRepositoryList({ client, onChanged, onClose }: { client: GuiRpcClient; onChanged(): void; onClose(): void }) {
    const { t } = useTranslation();
    const current = useActionScope(client);
    const [inspection, setInspection] = useState<RepositoryListInspection>();
    const [selected, setSelected] = useState<Set<number>>(new Set());
    const [results, setResults] = useState<Record<number, ItemResult>>({});
    const [stage, setStage] = useState<"select" | "review" | "result">("select");
    const [busy, setBusy] = useState(false);
    const [busyAction, setBusyAction] = useState<"reading" | "checking" | "adding">("reading");
    const [cancelled, setCancelled] = useState(false);
    const [error, setError] = useState<RpcError>();
    const pending = useRef(false);
    const cancel = useRef(false);
    const stoppedForUnknown = useRef(false);
    useEffect(() => () => { cancel.current = true; }, []);
    const begin = (action: "reading" | "checking" | "adding") => { pending.current = true; cancel.current = false; stoppedForUnknown.current = false; setCancelled(false); setBusyAction(action); setBusy(true); setError(undefined); };
    const end = () => { pending.current = false; if (current()) setBusy(false); };
    const read = async () => {
        if (pending.current) return;
        begin("reading");
        try {
            const path = await client.selectRepositoryListFile("open");
            if (!current() || cancel.current || path === undefined) return;
            const value = await client.repositoryListInspectFile(path);
            if (!current()) return;
            setInspection(value); setSelected(new Set(value.entries.filter((entry) => entry.status === "candidate").map((entry) => entry.line))); setResults({}); setStage("select");
        } catch (caught) { if (current()) setError(safeError(caught)); }
        finally { end(); }
    };
    const update = (line: number, value: ItemResult) => { if (current()) setResults((items) => ({ ...items, [line]: value })); };
    const check = async () => {
        if (pending.current || !inspection) return;
        begin("checking");
        const entries = inspection.entries.filter((entry) => selected.has(entry.line) && entry.source);
        setResults(Object.fromEntries(entries.map((entry) => [entry.line, { phase: "pending" }])));
        for (const entry of entries) {
            if (cancel.current || !current()) break;
            update(entry.line, { phase: "checking" });
            try { await client.repositoriesInspect(entry.source!); update(entry.line, { phase: "ready" }); }
            catch (caught) { update(entry.line, { phase: "failed", error: safeError(caught) }); }
        }
        if (current()) { setStage("review"); setCancelled(cancel.current); }
        end();
    };
    const register = async () => {
        if (pending.current || !inspection || stage !== "review") return;
        begin("adding");
        for (const entry of inspection.entries) {
            if (cancel.current || !current()) break;
            if (results[entry.line]?.phase !== "ready" || !entry.source) continue;
            update(entry.line, { phase: "registering" });
            try {
                await client.repositoryRegister(entry.source);
                update(entry.line, { phase: "registered" });
                if (current()) onChanged();
            } catch (caught) {
                const failure = safeError(caught);
                update(entry.line, { phase: failure.code === "repository_already_registered" ? "duplicate" : definiteRejection.has(failure.code) ? "failed" : "unknown", error: failure });
                // A transport/store failure may hide a committed write. Do not retry it or start more writes.
                if (!definiteRejection.has(failure.code)) { cancel.current = true; stoppedForUnknown.current = true; if (current()) onChanged(); break; }
            }
        }
        if (current()) { setStage("result"); setCancelled(cancel.current && !stoppedForUnknown.current); }
        end();
    };
    const entries = inspection?.entries ?? [];
    const ready = Object.values(results).filter((item) => item.phase === "ready").length;
    const registered = Object.values(results).filter((item) => item.phase === "registered").length;
    const failures = Object.values(results).filter((item) => item.phase === "failed").length;
    const unknown = Object.values(results).filter((item) => item.phase === "unknown").length;
    const unprocessed = Object.values(results).filter((item) => item.phase === "pending" || item.phase === "ready").length;
    const completed = Object.values(results).filter((item) => !["pending", "checking", "registering", ...(busyAction === "adding" ? ["ready"] : [])].includes(item.phase)).length;
    const phaseText = (entry: RepositoryListEntry) => {
        if (stage !== "select" && entry.status === "candidate" && !results[entry.line]) return t("Not selected");
        const phase = results[entry.line]?.phase ?? entry.status;
        const labels = { candidate: "Available to select", duplicate: "Duplicate", invalid: "Invalid", unsupported: "Unsupported", pending: "Not processed", checking: "Checking repository", ready: stage === "result" ? "Not processed" : "Ready to add", failed: "Failed", registering: "In flight", registered: "Added", unknown: "Result unknown" };
        return t(labels[phase]);
    };
    return <Dialog wide open dismissible={!busy} title={t("Import list")} onClose={onClose} actions={<>
        {busy ? <Button disabled={cancelled} onClick={() => { cancel.current = true; setCancelled(true); }} type="button" variant="text">{t("Stop after current request")}</Button> : <Button onClick={onClose} type="button" variant="text">{t("Close")}</Button>}
        {!busy && (inspection === undefined || stage === "select" || stage === "result") ? <Button onClick={() => void read()} type="button" variant="text">{t("Choose list file")}</Button> : null}
        {!busy && stage === "select" && inspection ? <Button disabled={selected.size === 0} onClick={() => void check()} type="button">{t("Check selected repositories")}</Button> : null}
        {!busy && stage === "review" ? <><Button onClick={() => { setStage("select"); setResults({}); setError(undefined); setCancelled(false); }} type="button" variant="text">{t("Back")}</Button><Button disabled={ready === 0} onClick={() => void register()} type="button">{t("Add {count} repositories", { count: ready })}</Button></> : null}
    </>}>
        <p>{t("Append repositories from a UTF-8 list. One HTTP(S) URL per line; # starts a comment. Existing repositories are kept.")}</p>
        {error ? <RequestReminder error={error} /> : null}
        {inspection ? <>
            <p>{t("{count} entries · {ignored} blank or comment lines", { count: entries.length, ignored: inspection.ignoredLines })}</p>
            {stage !== "result" ? <Reminder>{t("Checking only reads repositories. Adding starts after your explicit confirmation; each entry is independent.")}</Reminder> : <Reminder tone={failures || unknown || cancelled ? "warning" : "success"}>
                <p>{t("Added: {added} · Failed: {failed} · Unknown: {unknown} · Not processed: {pending}", { added: registered, failed: failures, unknown, pending: unprocessed })}</p>
                <p>{t("Completed additions are kept. Unknown results are never retried automatically. Refresh the list and check them before starting another import.")}</p>
            </Reminder>}
            {stage === "review" ? <Reminder tone={failures || unprocessed ? "warning" : "info"}>{t("Ready: {ready} · Check failed: {failed} · Not processed: {pending}", { ready, failed: failures, pending: Object.values(results).filter((item) => item.phase === "pending").length })}</Reminder> : null}
            {cancelled ? <Reminder tone="warning">{t("Cancellation stopped new requests. Completed additions remain; an in-flight request is allowed to finish.")}</Reminder> : null}
            <ScrollArea className="repository-list-entries"><ul>{entries.map((entry) => <li key={entry.line}>
                <div>{stage === "select" && entry.status === "candidate" ? <Checkbox checked={selected.has(entry.line)} disabled={busy} label={t("Select line {line}", { line: entry.line })} onChange={(checked) => setSelected((before) => { const next = new Set(before); if (checked) next.add(entry.line); else next.delete(entry.line); return next; })} /> : <strong>{t("Line {line}", { line: entry.line })}</strong>}<span>{phaseText(entry)}</span></div>
                {entry.source ? <code>{entry.source.url}</code> : null}
                {entry.reason ? <code>{entry.reason}</code> : null}
                {results[entry.line]?.error ? <code className="repository-list-error">{results[entry.line]?.error?.code}</code> : null}
            </li>)}</ul></ScrollArea>
        </> : null}
        {busy ? <div className="repository-list-progress" aria-live="polite"><Progress label={t("Repository list progress")} /><span>{busyAction === "reading" ? t("Loading…") : busyAction === "checking" ? t("Checking repositories: {count} completed", { count: completed }) : t("Adding repositories: {count} completed", { count: completed })}</span></div> : null}
    </Dialog>;
}

function ExportRepositoryList({ client, onClose }: { client: GuiRpcClient; onClose(): void }) {
    const { t } = useTranslation();
    const current = useActionScope(client);
    const [preview, setPreview] = useState<RepositoryListPreview>();
    const [confirmed, setConfirmed] = useState(false);
    const [busy, setBusy] = useState(false);
    const pending = useRef(false);
    const [error, setError] = useState<RpcError>();
    const [saved, setSaved] = useState<RepositoryListExportResult>();
    const [fileName, setFileName] = useState("repositories.txt");
    const nameValid = fileName.length > 0 && fileName.length <= 255 && !/[\\/:*?"<>|\u0000-\u001f]/.test(fileName) && !/[. ]$/.test(fileName);
    const load = async () => {
        if (pending.current) return;
        pending.current = true; setBusy(true); setError(undefined); setPreview(undefined); setSaved(undefined); setConfirmed(false);
        try { const result = await client.repositoryListPreviewExport(); if (current()) setPreview(result); }
        catch (caught) { if (current()) setError(safeError(caught)); }
        finally { pending.current = false; if (current()) setBusy(false); }
    };
    useEffect(() => { void load(); }, [client]);
    const save = async () => {
        if (!preview || !confirmed || pending.current || saved || !nameValid) return;
        pending.current = true; setBusy(true); setError(undefined);
        try {
            const path = await client.selectRepositoryListFile("save", fileName);
            if (!current() || path === undefined) return;
            const result = await client.repositoryListExportFile(path, preview.fingerprint);
            if (current()) setSaved(result);
        } catch (caught) { if (current()) { const failure = safeError(caught); setError(failure); if (failure.code === "repository_list_changed") { setPreview(undefined); setConfirmed(false); } } }
        finally { pending.current = false; if (current()) setBusy(false); }
    };
    const reasonText = { local_source: "Local source cannot be represented by this URL list.", authentication_not_supported: "Authentication settings cannot be represented by this URL list.", unsupported_source: "This source cannot be represented by this URL list." };
    return <Dialog wide open dismissible={!busy} title={t("Export list")} onClose={onClose} actions={<><Button disabled={busy} onClick={onClose} type="button" variant="text">{t("Close")}</Button><Button disabled={busy} onClick={() => void load()} type="button" variant="text">{t("Preview again")}</Button>{saved ? null : <Button disabled={busy || !confirmed || !preview || preview.exportableCount === 0 || !nameValid} onClick={() => void save()} type="button">{t("Save new list file")}</Button>}</>}>
        {error ? <RequestReminder error={error}>{error.code === "repository_list_target_exists" ? t("The file already exists and was preserved. Choose another file name.") : error.code === "repository_list_changed" ? t("The repository snapshot changed. Preview again before saving.") : undefined}</RequestReminder> : null}
        {preview ? <>
            <p>{t("{count} repositories can be exported · {excluded} excluded · {bytes} bytes", { count: preview.exportableCount, excluded: preview.excluded.length, bytes: preview.byteCount })}</p>
            <Reminder tone="warning">{t("This file contains complete repository addresses, which may contain credentials. Check it before sharing. Authentication headers and stored credentials are not included. This is not a complete backup.")}</Reminder>
            {preview.excluded.length ? <ScrollArea className="repository-list-entries"><ul>{preview.excluded.map((entry) => <li key={entry.repositoryId}><code>{entry.repositoryId}</code><span>{t(reasonText[entry.reason])}</span></li>)}</ul></ScrollArea> : null}
            <p>{t("Only a new file can be created. Existing files are preserved; choose a different name if the destination exists.")}</p>
            {!saved ? <><TextField disabled={busy} error={!nameValid} errorText={!nameValid ? t("Enter a file name without path separators or reserved characters.") : undefined} id="repository-list-file-name" label={t("New list file name")} maxLength={255} onInput={(value) => { setFileName(value); setError(undefined); }} value={fileName} /><p>{t("Choose the destination folder. A new file with this name will be created there.")}</p></> : null}
            {!saved ? <Checkbox checked={confirmed} disabled={busy} label={t("I have reviewed the export scope and the address-sharing risk.")} onChange={setConfirmed} /> : <Reminder tone="success"><p>{t("Saved {count} repositories ({bytes} bytes).", { count: saved.exportedCount, bytes: saved.byteCount })}</p><code>{saved.sha256}</code></Reminder>}
        </> : null}
        {busy ? <Progress label={t("Repository list progress")} /> : null}
    </Dialog>;
}
