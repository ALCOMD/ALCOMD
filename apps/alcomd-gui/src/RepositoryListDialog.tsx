import { useEffect, useRef, useState } from "react";
import type { RepositoryListImportItem, RepositoryListInspection, RepositoryListPreview, RepositoryListExportResult, RpcError } from "@alcomd/sdk";
import type { GuiRpcClient } from "./rpc";
import { RegisterRepositoryPanel, useActionScope } from "./CoreActions";
import { Button, Checkbox, Dialog, DialogCloseButton, MenuItem, Progress, TextField } from "./Material";
import { FilePickerControl } from "./FilePickerControl";
import { SplitButton } from "./SplitButton";
import { UtilityActionDialog } from "./UtilityActionDialog";
import { ScrollArea } from "./ScrollArea";
import { useTranslation } from "./i18n";
import { capabilities, capabilityUnavailableTitle, useCapability } from "./capabilities";
import { normalizeRequestError, requestErrorMessage } from "./request-errors";
import "./RepositoryListDialog.css";

export function RepositoryToolbar({ client, onChanged }: { client: GuiRpcClient; onChanged(): void }) {
    const { t } = useTranslation();
    const manage = useCapability(capabilities.repositoriesRegistry);
    const files = useCapability(capabilities.repositoriesListFiles);
    const importAvailable = useCapability(capabilities.repositoriesListImport);
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
            <MenuItem disabled={!files || !manage || !importAvailable} label={t("Import list")} onClick={() => setDialog("import")} title={capabilityUnavailableTitle(files, capabilities.repositoriesListFiles, t) ?? capabilityUnavailableTitle(importAvailable, capabilities.repositoriesListImport, t)} />
            <MenuItem disabled={!files} label={t("Export list")} onClick={() => setDialog("export")} title={capabilityUnavailableTitle(files, capabilities.repositoriesListFiles, t)} />
        </SplitButton>
        {dialog === "add" ? <UtilityActionDialog title={t("Add repository")} onClose={close}><RegisterRepositoryPanel client={client} onChanged={onChanged} /></UtilityActionDialog> : null}
        {dialog === "import" ? <ImportRepositoryList client={client} onChanged={onChanged} onClose={close} /> : null}
        {dialog === "export" ? <ExportRepositoryList client={client} onClose={close} /> : null}
    </>;
}

const safeError = (caught: unknown): RpcError => ({ code: typeof caught === "object" && caught !== null && "code" in caught && typeof caught.code === "string" ? caught.code : "internal_error", message: "Repository list request failed" });

function ImportRepositoryList({ client, onChanged, onClose }: { client: GuiRpcClient; onChanged(): void; onClose(): void }) {
    const { t } = useTranslation();
    const current = useActionScope(client);
    const [path, setPath] = useState("");
    const [choosing, setChoosing] = useState(false);
    const [preview, setPreview] = useState<{ path: string; inspection: RepositoryListInspection }>();
    const [results, setResults] = useState<RepositoryListImportItem[]>();
    const [finished, setFinished] = useState(false);
    const [busyAction, setBusyAction] = useState<"reading" | "adding">();
    const [stopping, setStopping] = useState(false);
    const [error, setError] = useState<RpcError>();
    const pending = useRef(false);
    const cancelRead = useRef(false);
    const activeImport = useRef<string | undefined>(undefined);
    useEffect(() => () => {
        cancelRead.current = true;
        if (activeImport.current !== undefined) void client.repositoryListCancelRegistration(activeImport.current).catch(() => {});
    }, [client]);

    const inspection = preview?.inspection;
    const entries = inspection?.entries ?? [];
    const candidateCount = entries.filter(({ status }) => status === "candidate").length;
    const busy = busyAction !== undefined;
    const begin = (action: "reading" | "adding") => {
        pending.current = true;
        cancelRead.current = false;
        setStopping(false);
        setBusyAction(action);
        setError(undefined);
    };
    const end = () => { pending.current = false; if (current()) setBusyAction(undefined); };
    const read = async (selectedPath: string) => {
        if (pending.current || selectedPath.trim() === "") return;
        setPath(selectedPath);
        setPreview(undefined);
        setResults(undefined);
        setFinished(false);
        begin("reading");
        try {
            const inspection = await client.repositoryListInspectFile(selectedPath);
            if (!current() || cancelRead.current) return;
            setPreview({ path: selectedPath, inspection });
        } catch (caught) { if (current() && !cancelRead.current) setError(normalizeRequestError(caught)); }
        finally { end(); }
    };
    const register = async () => {
        if (pending.current || finished || !preview || candidateCount === 0) return;
        begin("adding");
        const key = crypto.randomUUID();
        activeImport.current = key;
        try {
            const { items } = await client.repositoryListImportFile(preview.path, preview.inspection.fingerprint, key);
            if (current()) setResults(items);
        } catch (caught) {
            if (current()) setError(normalizeRequestError(caught));
        } finally {
            activeImport.current = undefined;
            end();
            if (current()) {
                setFinished(true);
                onChanged();
            }
        }
    };
    const stop = async () => {
        if (busyAction === "reading") {
            cancelRead.current = true;
            setStopping(true);
            return;
        }
        const key = activeImport.current;
        if (key === undefined || stopping) return;
        setStopping(true);
        setError(undefined);
        try {
            const { stopped } = await client.repositoryListCancelRegistration(key);
            if (!stopped && current() && activeImport.current === key) setStopping(false);
        } catch (caught) {
            if (current() && activeImport.current === key) {
                setError(normalizeRequestError(caught));
                setStopping(false);
            }
        }
    };
    const byLine = new Map(results?.map((item) => [item.line, item]));
    const counts = { added: 0, failed: 0, unknown: 0, unprocessed: 0 };
    for (const { status } of results ?? []) if (status !== "existing") counts[status] += 1;
    const labels = { candidate: "To import", duplicate: "Duplicate", invalid: "Invalid", unsupported: "Unsupported", failed: "Failed", added: "Added", unknown: "Result unknown", existing: "Already added", unprocessed: "Not processed" } as const;
    const actions = busy ? <Button disabled={stopping} onClick={() => void stop()} type="button" variant="text">{busyAction === "adding" ? t("Stop import") : t("Stop after current request")}</Button>
        : finished ? <DialogCloseButton variant="text">{t("Close")}</DialogCloseButton>
        : inspection ? <>
            <Button onClick={() => setPreview(undefined)} type="button" variant="text">{t("Back")}</Button>
            <Button disabled={candidateCount === 0} onClick={() => void register()} type="button">{t("Add {count} repositories", { count: candidateCount })}</Button>
        </> : <>
            <DialogCloseButton disabled={choosing} variant="text">{t("Cancel")}</DialogCloseButton>
            <Button disabled={choosing || path.trim() === ""} onClick={() => void read(path)} type="button">{t("Confirm")}</Button>
        </>;
    return <Dialog wide={inspection !== undefined} open dismissible={!busy && !choosing} title={t("Import list")} onClose={onClose} actions={actions}>
        {inspection ? null : <>
            <p>{t("Append repositories from a UTF-8 list. One HTTP(S) URL per line; # starts a comment. Existing repositories are kept.")}</p>
            <FilePickerControl client={client} choosePath={() => client.selectRepositoryListFile("open")} disabled={busy}
                label={t("Repository list file")} value={path} onChoosingChange={setChoosing}
                onPathChange={(next) => { setPath(next); setError(undefined); }} />
        </>}
        {error ? <div role="alert"><p>{t(requestErrorMessage(error.code))}</p><code>{error.code}</code>{error.diagnosticId === undefined ? null : <p><code>{error.diagnosticId}</code></p>}</div> : null}
        {inspection ? <>
            <p>{t("{count} entries · {ignored} blank or comment lines", { count: entries.length, ignored: inspection.ignoredLines })}</p>
            {results === undefined ? null : <div aria-live="polite">
                <p>{t("Added: {added} · Failed: {failed} · Unknown: {unknown} · Not processed: {pending}", { added: counts.added, failed: counts.failed, unknown: counts.unknown, pending: counts.unprocessed })}</p>
            </div>}
            {finished && (counts.unknown > 0 || results === undefined) ? <p>{t("Completed additions are kept. Check the repository list before importing again.")}</p> : null}
            <ScrollArea className="repository-list-entries"><ul>{entries.map((entry) => {
                const outcome = byLine.get(entry.line);
                return <li key={entry.line}>
                    <div><strong>{t("Line {line}", { line: entry.line })}</strong><span>{t(labels[outcome?.status ?? entry.status])}</span></div>
                    {entry.source ? <code>{entry.source.url}</code> : null}
                    {entry.credentialsConfigured ? <span>{t("Uses authentication")}</span> : null}
                    {entry.reason ? <span>{t(requestErrorMessage(entry.reason))}</span> : null}
                    {outcome === undefined || !("code" in outcome) ? null : <span className="repository-list-error">{t(requestErrorMessage(outcome.code))}</span>}
                </li>;
            })}</ul></ScrollArea>
        </> : null}
        {busy ? <div className="repository-list-progress" aria-live="polite"><Progress label={t("Repository list progress")} /><span>{busyAction === "reading" ? t("Loading…") : stopping ? t("Stopping import…") : t("Adding repositories…")}</span></div> : null}
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
    return <Dialog wide open dismissible={!busy} title={t("Export list")} onClose={onClose} actions={<><DialogCloseButton disabled={busy} variant="text">{t("Close")}</DialogCloseButton><Button disabled={busy} onClick={() => void load()} type="button" variant="text">{t("Preview again")}</Button>{saved ? null : <Button disabled={busy || !confirmed || !preview || preview.exportableCount === 0 || !nameValid} onClick={() => void save()} type="button">{t("Save new list file")}</Button>}</>}>
        {error ? <div role="alert"><p>{t(requestErrorMessage((error).code))}</p><code>{(error).code}</code>{error.diagnosticId === undefined ? null : <p><code>{error.diagnosticId}</code></p>}</div> : null}
        {preview ? <>
            <p>{t("{count} repositories can be exported · {excluded} excluded · {bytes} bytes", { count: preview.exportableCount, excluded: preview.excluded.length, bytes: preview.byteCount })}</p>
            <div>{t("This file can contain repository credentials in plain text, including authentication headers. Keep it private.")}</div>
            {preview.excluded.length ? <ScrollArea className="repository-list-entries"><ul>{preview.excluded.map((entry) => <li key={entry.repositoryId}><code>{entry.repositoryId}</code><span>{t(reasonText[entry.reason])}</span></li>)}</ul></ScrollArea> : null}
            <p>{t("Only a new file can be created. Existing files are preserved; choose a different name if the destination exists.")}</p>
            {!saved ? <><TextField disabled={busy} error={!nameValid} errorText={!nameValid ? t("Enter a file name without path separators or reserved characters.") : undefined} id="repository-list-file-name" label={t("New list file name")} maxLength={255} onInput={(value) => { setFileName(value); setError(undefined); }} value={fileName} /><p>{t("Choose the destination folder. A new file with this name will be created there.")}</p></> : null}
            {!saved ? <Checkbox checked={confirmed} disabled={busy} label={t("I have reviewed the export scope and understand the file can contain credentials.")} onChange={setConfirmed} /> : <div><p>{t("Saved {count} repositories ({bytes} bytes).", { count: saved.exportedCount, bytes: saved.byteCount })}</p><code>{saved.sha256}</code></div>}
        </> : null}
        {busy ? <Progress label={t("Repository list progress")} /> : null}
    </Dialog>;
}
