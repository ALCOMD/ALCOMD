import { requestErrorMessage } from "./request-errors";
import type { ProjectsCreateParams, ProjectTemplateRef, ProjectTemplatesListResult, RpcError } from "@alcomd/sdk";
import { useCallback, useEffect, useRef, useState } from "react";

import type { BackupApplyRestoreResult, BackupRecord, BackupRestorePlan, Operation } from "./core-models";
import { Button, Dialog, PathField, Select, TextField } from "./Material";
import { AcceptedOperationProgress, OperationFollow, operationRequestOutcomeUnknown, operationResultProjectId, useAcceptedOperations, useActionScope } from "./CoreActions";
import { useCoreEvents, useWorkspaceInvalidation } from "./core-events-ui";
import { capabilities, useCapability } from "./capabilities";
import { LoadingState, useDelayedPending } from "./LoadingState";
import type { GuiRpcClient } from "./rpc";
import { useTranslation } from "./i18n";
import type { SupportedLocale } from "./i18n-types";
import { displayChildPath } from "./display-path";
import { useProjectDirectory } from "./use-project-directory";
import { CreateUnityVersionField, useCreateUnityVersion } from "./use-create-unity-version";

interface ProjectDialogProps {
    client: GuiRpcClient;
    onClose(): void;
    onCompleted(projectId: string): void;
    open: boolean;
}

const ACTIVE_OPERATION_STATES = new Set([
    "queued",
    "planning",
    "waiting_for_input",
    "running",
    "cancelling",
    "recovering",
    "interrupted"
]);

export function CreateProjectDialog({ client, onChanged, onClose, onCompleted, open }: ProjectDialogProps & { onChanged(): Promise<void> }) {
    const { t } = useTranslation();
    const events = useCoreEvents();
    const canUseTemplates = useCapability(capabilities.projectTemplates);
    const [catalog, setCatalog] = useState<ProjectTemplatesListResult>();
    const [catalogBusy, setCatalogBusy] = useState(false);
    const [catalogError, setCatalogError] = useState<RpcError>();
    const [failedCursor, setFailedCursor] = useState<string>();
    const catalogRequest = useRef<object | undefined>(undefined);
    const catalogDepth = useRef(1);
    const [selection, setSelection] = useState<ProjectTemplateRef>();
    const selected = catalog?.templates.find(({ id }) => id === selection?.id);
    const staleSelection = selection !== undefined && selected?.revision !== selection.revision;
    const unity = useCreateUnityVersion(client, selected?.editorVersion, open);
    const { directory: targetParent, setDirectory: setTargetParent, error: directoryError } = useProjectDirectory(client, open);
    const [name, setName] = useState("");
    const [operationId, setOperationId] = useState<string>();
    const operations = useAcceptedOperations(client);
    const operation = operations.snapshot(operationId ?? "")?.operation;
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState<RpcError>();
    const draft = useRef(0);
    const draftRequest = useRef<object | undefined>(undefined);
    const attempt = useRef<{ client: GuiRpcClient; fingerprint: string; params: ProjectsCreateParams } | undefined>(undefined);
    const current = useActionScope(client);
    const callbacks = useRef({ onChanged, onCompleted, open });
    callbacks.current = { onChanged, onCompleted, open };

    const readCatalog = useCallback(async (cursor?: string) => {
        if (!open || !canUseTemplates) return;
        if (cursor !== undefined && catalogRequest.current !== undefined) return;
        const token = {};
        const depth = catalogDepth.current;
        catalogRequest.current = token;
        setCatalogBusy(true);
        setCatalogError(undefined);
        setFailedCursor(undefined);
        const load = async () => {
            let page = await client.projectTemplatesList(cursor);
            const records = [...page.templates];
            let pageCount = 1;
            const visited = new Set<string>();
            while (cursor === undefined && pageCount < depth && page.nextCursor !== undefined) {
                if (visited.has(page.nextCursor)) throw new Error("Repeated project template cursor");
                visited.add(page.nextCursor);
                page = await client.projectTemplatesList(page.nextCursor);
                records.push(...page.templates);
                pageCount += 1;
            }
            return { records, nextCursor: page.nextCursor, pageCount };
        };
        try {
            const next = await (events === undefined ? load() : events.consistentRead(["templates"], load));
            if (catalogRequest.current !== token) return;
            catalogDepth.current = cursor === undefined ? next.pageCount : depth + 1;
            setCatalog((previous) => ({
                templates: cursor === undefined ? next.records : [...(previous?.templates ?? []), ...next.records],
                nextCursor: next.nextCursor
            }));
        } catch (caught: unknown) {
            if (catalogRequest.current !== token) return;
            setCatalogError(safeError(caught));
            setFailedCursor(cursor);
            throw caught;
        } finally {
            if (catalogRequest.current === token) {
                catalogRequest.current = undefined;
                setCatalogBusy(false);
            }
        }
    }, [canUseTemplates, client, events, open]);
    const retryCatalog = useWorkspaceInvalidation(open && canUseTemplates ? ["templates"] : [], readCatalog);

    useEffect(() => {
        draft.current += 1;
        draftRequest.current = undefined;
        setName("");
        setSelection(undefined);
        setOperationId(undefined);
        setBusy(false);
        setError(undefined);
    }, [client, open]);

    useEffect(() => {
        setCatalog(undefined);
        setCatalogError(undefined);
        setFailedCursor(undefined);
        setCatalogBusy(false);
        catalogDepth.current = 1;
        void readCatalog().catch(() => {});
        return () => { catalogRequest.current = undefined; };
    }, [readCatalog]);

    const run = async (request: (ownsDraft: () => boolean) => Promise<void>) => {
        if (draftRequest.current !== undefined) return;
        const generation = draft.current;
        const token = {};
        draftRequest.current = token;
        const ownsDraft = () => current() && generation === draft.current && draftRequest.current === token;
        setBusy(true);
        setError(undefined);
        try { await request(ownsDraft); }
        catch (caught: unknown) { if (ownsDraft()) setError(safeError(caught)); }
        finally {
            if (ownsDraft()) {
                draftRequest.current = undefined;
                setBusy(false);
            }
        }
    };

    const close = () => {
        if (draftRequest.current !== undefined) return;
        draft.current += 1;
        onClose();
    };
    const chooseParent = () => run(async (ownsDraft) => {
        const directory = await client.selectDirectory();
        if (ownsDraft() && directory !== undefined) setTargetParent(directory);
    });
    const retrying = operationRequestOutcomeUnknown(error) && attempt.current?.client === client;
    const ready = retrying || (!staleSelection && name.trim().length > 0 && targetParent.length > 0 && unity.version.trim().length > 0);
    const formDisabled = busy || retrying;
    const create = () => run(async (ownsDraft) => {
        if (!ready) return;
        const generation = draft.current;
        const input = {
            name: name.trim(),
            targetParent,
            unityVersion: unity.version.trim(),
            ...(selection === undefined ? {} : { template: selection })
        };
        const fingerprint = JSON.stringify(input);
        if (!retrying && (attempt.current?.client !== client || attempt.current.fingerprint !== fingerprint)) {
            attempt.current = { client, fingerprint, params: { ...input, idempotencyKey: crypto.randomUUID() } };
        }
        const previous = attempt.current;
        if (previous === undefined || previous.client !== client) return;
        let accepted;
        try {
            accepted = await client.projectsCreate(previous.params);
        } catch (caught: unknown) {
            const failure = safeError(caught);
            if (ownsDraft() && !operationRequestOutcomeUnknown(failure)) attempt.current = undefined;
            if (ownsDraft() && selection !== undefined && failure.code === "revision_conflict") void readCatalog().catch(() => {});
            throw caught;
        }
        operations.accept(accepted.operationId, (completed) => {
            const projectId = operationResultProjectId(completed);
            if (projectId === undefined || projectId !== accepted.projectId) throw { code: "internal_error" };
            void callbacks.current.onChanged().catch(() => {});
            if (current() && generation === draft.current && callbacks.current.open) callbacks.current.onCompleted(projectId);
        });
        if (ownsDraft()) {
            attempt.current = undefined;
            setOperationId(accepted.operationId);
        }
    });
    const cancelOperation = () => run(async () => {
        if (operation === undefined) return;
        const result = await client.operationCancel(operation.operationId, operation.revision);
        if (result.operation.operationId !== operation.operationId) throw { code: "internal_error" };
    });

    const terminal = operation !== undefined && !ACTIVE_OPERATION_STATES.has(operation.state);
    const nameCounts = new Map<string, number>();
    for (const { name } of catalog?.templates ?? []) {
        nameCounts.set(name, (nameCounts.get(name) ?? 0) + 1);
    }
    const options = [
        { label: t("No template"), value: "none" },
        ...(catalog?.templates ?? []).map(({ id, name, editorVersion }) => {
            const label = editorVersion === undefined ? name : name + " · " + editorVersion;
            return { label: nameCounts.get(name) === 1 ? label : label + " (" + id + ")", value: id };
        })
    ];
    return <>
        <Dialog dismissible={!busy} onClose={close} open={open} title={t("Create project")}>
            <div className="project-create-flow">
                {operationId === undefined ? <>
                    {canUseTemplates ? <>
                        <Select disabled={formDisabled} aria-label={t("Project template")} id="project-create-template" label={t("Template")}
                            onChange={(id) => {
                                const next = catalog?.templates.find((record) => record.id === id);
                                setSelection(next === undefined ? undefined : { id, revision: next.revision });
                            }}
                            options={options} value={staleSelection ? "" : selection?.id ?? "none"} />
                        {staleSelection && !retrying ? <p role="alert">{t("The selected template changed. Select it again.")}</p> : null}
                        {catalogError !== undefined && failedCursor === undefined ? <div role="alert">
                            <p>{t("Templates could not be loaded.")}</p>
                            <Button disabled={busy || catalogBusy} onClick={retryCatalog} type="button" variant="text">{t("Retry")}</Button>
                        </div> : null}
                        {failedCursor === undefined ? null : <p role="alert">{t("Could not load more items. Your loaded items are still available.")}</p>}
                        {catalog?.nextCursor === undefined ? null : <div className="pagination-controls">
                            <Button disabled={busy || catalogBusy} onClick={() => void readCatalog(catalog.nextCursor).catch(() => {})} type="button" variant="text">{t(failedCursor === undefined ? "Load more templates" : "Retry loading more")}</Button>
                        </div>}
                    </> : null}
                    <CreateUnityVersionField disabled={formDisabled} selection={unity} />
                    <TextField disabled={formDisabled} aria-label={t("Create project name")} label={t("Project name")} onInput={setName} value={name} />
                    <div className="project-target-picker">
                        <PathField disabled={formDisabled} aria-label={t("Create target parent")} label={t("Parent directory")} onInput={setTargetParent} value={targetParent} />
                        <Button disabled={formDisabled} onClick={() => void chooseParent()} type="button" variant="tonal">{t("Choose directory")}</Button>
                    </div>
                    <div className="dialog-actions">
                        <Button disabled={busy} onClick={close} type="button" variant="text">{t("Cancel")}</Button>
                        <Button disabled={busy || !ready} onClick={() => void create()} type="button">{t("Create project")}</Button>
                    </div>
                </> : <>
                    <AcceptedOperationProgress operationId={operationId} operations={operations} />
                    <div className="dialog-actions">
                        {!terminal ? <Button disabled={busy || operation === undefined} onClick={() => void cancelOperation()} type="button" variant="text">{t("Cancel creation")}</Button> : null}
                        <Button disabled={busy} onClick={close} type="button">{t("Close")}</Button>
                    </div>
                </>}
                {directoryError === undefined ? null : <div role="alert"><strong>{t("Default project location could not be loaded.")}</strong><p>{t(requestErrorMessage(directoryError.code))}</p><code>{directoryError.code}</code>{directoryError.diagnosticId === undefined ? null : <p><code>{directoryError.diagnosticId}</code></p>}</div>}
                {error === undefined ? null : <div role="alert"><p>{t(requestErrorMessage(error.code))}</p><code>{error.code}</code>{error.diagnosticId === undefined ? null : <p><code>{error.diagnosticId}</code></p>}{operationId === undefined && operationRequestOutcomeUnknown(error) ? <p>{t("Retry with the same details to recover the original creation request.")}</p> : null}</div>}
            </div>
        </Dialog>
        {operations.follow()}
    </>;
}

export function RestoreProjectDialog({ backup, client, onClose, onCompleted, onViewOperation, open }: ProjectDialogProps & { backup?: BackupRecord; onViewOperation?(operationId: string): void }) {
    const { t, locale } = useTranslation();
    const [backups, setBackups] = useState<BackupRecord[]>([]);
    const [backupId, setBackupId] = useState("");
    const { directory: targetParent, setDirectory: setTargetParent, error: directoryError } = useProjectDirectory(client, open);
    const [targetLeaf, setTargetLeaf] = useState("");
    const [plan, setPlan] = useState<BackupRestorePlan>();
    const [operation, setOperation] = useState<Operation>();
    const [accepted, setAccepted] = useState<BackupApplyRestoreResult>();
    const [submitted, setSubmitted] = useState(false);
    const [cursor, setCursor] = useState<string>();
    const [readVersion, setReadVersion] = useState(0);
    const [readFailed, setReadFailed] = useState(false);
    const [loading, setLoading] = useState(true);
    const delayedLoading = useDelayedPending(open && loading);
    const [visible, setVisible] = useState(false);
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState<RpcError>();
    const completionReported = useRef(false);
    const pending = useRef(false);
    const isCurrent = useActionScope(client, open);
    const canRestore = useCapability(capabilities.backupsRestore);

    useEffect(() => {
        if (!open) setVisible(false);
        else if (!loading || delayedLoading) setVisible(true);
    }, [delayedLoading, loading, open]);

    useEffect(() => {
        if (!open) {
            setBackups([]);
            setBackupId("");
            setTargetLeaf("");
            setPlan(undefined);
            setOperation(undefined);
            setAccepted(undefined);
            setSubmitted(false);
            setLoading(true);
            setBusy(false);
            setError(undefined);
            completionReported.current = false;
            return;
        }
        let active = true;
        setLoading(true);
        setReadFailed(false);
        setError(undefined);
        void (backup === undefined ? client.backupsList() : Promise.resolve({ backups: [backup], nextCursor: undefined })).then((result) => {
            if (!active) return;
            setBackups(result.backups);
            setBackupId(result.backups[0]?.backupId ?? "");
            setCursor(result.nextCursor);
        }).catch((caught: unknown) => {
            if (active) { setError(safeError(caught)); setReadFailed(true); }
        }).finally(() => {
            if (active) setLoading(false);
        });
        return () => { active = false; };
    }, [backup, client, open, readVersion]);

    const loadMore = async () => {
        if (cursor === undefined || pending.current) return;
        pending.current = true; setBusy(true); setError(undefined);
        try {
            const page = await client.backupsList(undefined, cursor);
            if (!isCurrent()) return;
            setBackups((current) => [...current, ...page.backups]); setCursor(page.nextCursor);
        } catch (caught: unknown) { if (isCurrent()) setError(safeError(caught)); }
        finally { pending.current = false; if (isCurrent()) setBusy(false); }
    };

    const chooseParent = async () => {
        if (pending.current) return;
        pending.current = true;
        setBusy(true);
        setError(undefined);
        try {
            const selected = await client.selectDirectory();
            if (isCurrent() && selected !== undefined) setTargetParent(selected);
        } catch (caught: unknown) {
            if (isCurrent()) setError(safeError(caught));
        } finally {
            pending.current = false; if (isCurrent()) setBusy(false);
        }
    };

    const review = async () => {
        if (!canRestore || pending.current) return;
        pending.current = true;
        setBusy(true);
        setError(undefined);
        try {
            const next = await client.backupPlanRestore(backupId, targetParent, targetLeaf.trim());
            if (isCurrent()) { setPlan(next); setSubmitted(false); }
        } catch (caught: unknown) {
            if (isCurrent()) setError(safeError(caught));
        } finally {
            pending.current = false; if (isCurrent()) setBusy(false);
        }
    };

    const apply = async () => {
        if (!canRestore || pending.current || submitted || plan === undefined) return;
        pending.current = true; setSubmitted(true);
        setBusy(true);
        setError(undefined);
        try {
            const result = await client.backupApplyRestore(plan.planId);
            if (isCurrent()) setAccepted(result);
        } catch (caught: unknown) {
            if (isCurrent()) setError(safeError(caught));
        } finally {
            pending.current = false; if (isCurrent()) setBusy(false);
        }
    };

    const cancelOperation = async () => {
        if (operation === undefined || pending.current || operation.state === "cancelling") return;
        pending.current = true;
        setBusy(true);
        setError(undefined);
        try {
            const result = await client.operationCancel(operation.operationId, operation.revision);
            if (isCurrent()) setOperation(result.operation);
        } catch (caught: unknown) {
            if (isCurrent()) setError(safeError(caught));
        } finally {
            pending.current = false; if (isCurrent()) setBusy(false);
        }
    };

    const selectedBackup = backups.find((candidate) => candidate.backupId === backupId);
    const terminal = operation !== undefined && !ACTIVE_OPERATION_STATES.has(operation.state);
    return (
        <Dialog dismissible={!busy} onClose={onClose} open={open && visible} title={t("Restore project")}>
            <div className="project-create-flow">
                {loading ? <LoadingState title={t("Loading backups…")} /> : null}
                {loading ? null : readFailed ? <div className="dialog-actions"><Button onClick={onClose} type="button" variant="text">{t("Close")}</Button><Button onClick={() => setReadVersion((value) => value + 1)} type="button">{t("Retry")}</Button></div> : backups.length === 0 ? (
                    <div className="dialog-empty" role="status">
                        <strong>{t("No managed backups available")}</strong>
                        <span>{t("Create a project backup before restoring a project.")}</span>
                        <div className="dialog-actions"><Button onClick={onClose} type="button">{t("Close")}</Button></div>
                    </div>
                ) : accepted !== undefined ? <>
                    <OperationFollow client={client} onSnapshot={setOperation} onTerminal={(next) => {
                        if (next.state !== "succeeded" || completionReported.current) return;
                        completionReported.current = true; onCompleted(accepted.projectId);
                    }} operationId={accepted.operationId} title={t("Restore project")} />
                    <div className="dialog-actions">
                        {operation === undefined || terminal ? null : <Button disabled={busy || operation.state === "cancelling"} onClick={() => void cancelOperation()} type="button" variant="text">{t("Cancel restore")}</Button>}
                        {onViewOperation === undefined ? null : <Button disabled={busy} onClick={() => onViewOperation(accepted.operationId)} type="button" variant="text">{t("View operation")}</Button>}
                        <Button disabled={busy} onClick={onClose} type="button">{t("Close")}</Button>
                    </div>
                </> : plan === undefined ? (
                    <>
                        {backup === undefined ? <Select aria-label={t("Project backup")} disabled={busy} label={t("Backup")} onChange={(value) => { setBackupId(value); setError(undefined); }} options={backups.map((backup) => ({ label: backupLabel(backup, locale), value: backup.backupId }))} value={backupId} /> : <p>{backupLabel(backup, locale)}</p>}
                        {cursor === undefined ? null : <Button disabled={busy} onClick={() => void loadMore()} type="button" variant="text">{t("Load more")}</Button>}
                        <div className="project-target-picker">
                            <PathField aria-label={t("Restore target parent")} disabled={busy} label={t("Parent directory")} onInput={(value) => { setTargetParent(value); setError(undefined); }} value={targetParent} />
                            <Button disabled={busy} onClick={() => void chooseParent()} type="button" variant="tonal">{t("Choose directory")}</Button>
                        </div>
                        <TextField aria-label={t("Restore project name")} disabled={busy} label={t("Project name")} onInput={(value) => { setTargetLeaf(value); setError(undefined); }} value={targetLeaf} />
                        <div className="dialog-actions">
                            <Button disabled={busy} onClick={onClose} type="button" variant="text">{t("Cancel")}</Button>
                            <Button disabled={!canRestore || busy || backupId.length === 0 || targetParent.length === 0 || targetLeaf.trim().length === 0} onClick={() => void review()} type="button">{t("Review restore")}</Button>
                        </div>
                    </>
                ) : (
                    <>
                        <div className="dialog-summary">
                            <div><span>{t("Backup")}</span><strong>{selectedBackup === undefined ? backupId : backupLabel(selectedBackup, locale)}</strong></div>
                            <div><span>{t("Destination")}</span><code>{displayChildPath(plan.target.parent, plan.target.leaf)}</code></div>
                            <div><span>{t("Package resolution")}</span><strong>{plan.packagesRequireResolve ? t("Required") : t("Not required")}</strong></div>
                            <div><span>{t("Plan")}</span><code>{plan.planFingerprint.slice(0, 12)}</code></div>
                        </div>
                        <p>{t("ALCOMD Core will validate the managed archive, restore transactionally, and register the completed project.")}</p>
                        {plan.packagesRequireResolve ? <p role="alert">{t("VPM packages require a separate resolve after restoration.")}</p> : null}
                        <div className="dialog-actions">
                            <Button disabled={busy || (submitted && operationRequestOutcomeUnknown(error))} onClick={() => { setPlan(undefined); setSubmitted(false); setError(undefined); }} type="button" variant="text">{t("Back")}</Button>
                            <Button disabled={!canRestore || busy || submitted} onClick={() => void apply()} type="button">{t("Restore project")}</Button>
                            {submitted ? <Button onClick={onClose} type="button" variant="text">{t("Close")}</Button> : null}
                        </div>
                    </>
                )}
                {directoryError === undefined ? null : <div role="alert"><strong>{t("Default project location could not be loaded.")}</strong><p>{t(requestErrorMessage((directoryError).code))}</p><code>{(directoryError).code}</code>{directoryError.diagnosticId === undefined ? null : <p><code>{directoryError.diagnosticId}</code></p>}</div>}
                {error === undefined ? null : <div role="alert"><strong>{t("Restore project")}</strong><p>{t(requestErrorMessage((error).code))}</p><code>{(error).code}</code>{error.diagnosticId === undefined ? null : <p><code>{error.diagnosticId}</code></p>}{submitted && operationRequestOutcomeUnknown(error) ? t("The result of this request is unknown. Check Tasks and refresh before reviewing it again.") : undefined}</div>}
            </div>
        </Dialog>
    );
}

function backupLabel(backup: BackupRecord, locale: SupportedLocale): string {
    const date = new Intl.DateTimeFormat(locale, { year: "numeric", month: "2-digit", day: "2-digit", timeZone: "UTC" }).format(new Date(backup.createdAtMs));
    return `${date} · ${backup.backupId.slice(0, 8)}`;
}

function safeError(caught: unknown): RpcError {
    if (typeof caught === "object" && caught !== null && "code" in caught && typeof caught.code === "string") {
        return { code: caught.code, message: "The request could not be completed.", ...("diagnosticId" in caught && typeof caught.diagnosticId === "string" ? { diagnosticId: caught.diagnosticId } : {}) };
    }
    return { code: "internal_error", message: "The request could not be completed." };
}
