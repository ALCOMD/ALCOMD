import type { RpcError } from "@alcomd/sdk";
import { useEffect, useRef, useState } from "react";

import type { BackupApplyRestoreResult, BackupRecord, BackupRestorePlan, Operation, TemplateRecord } from "./core-models";
import { Button, Dialog, Select, TextField } from "./Material";
import { OperationFollow, TemplateOperationProgress, templateApplyResultUnknown, templateOperationProjectId, useActionScope, useTemplateOperations } from "./CoreActions";
import { Reminder, RequestReminder } from "./Reminder";
import { capabilities, useCapability } from "./capabilities";
import { reviewTemplateCreation, TemplatePlanReview, type TemplateReview } from "./TemplatePlanReview";
import { LoadingState, useDelayedPending } from "./LoadingState";
import type { GuiRpcClient } from "./rpc";
import { useTranslation } from "./i18n";
import type { SupportedLocale } from "./i18n-types";
import { displayPath } from "./display-path";
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
    const [templates, setTemplates] = useState<TemplateRecord[]>([]);
    const [templateCursor, setTemplateCursor] = useState<string>();
    const [templateLoadError, setTemplateLoadError] = useState<RpcError>();
    const [templateReadVersion, setTemplateReadVersion] = useState(0);
    const [moreTemplatesLoading, setMoreTemplatesLoading] = useState(false);
    const [moreTemplatesFailed, setMoreTemplatesFailed] = useState(false);
    const templateSession = useRef(0);
    const templatePageRequest = useRef<object | undefined>(undefined);
    const [templateId, setTemplateId] = useState("");
    const unity = useCreateUnityVersion(client, templates.find((template) => template.templateId === templateId), open);
    const { directory: targetParent, setDirectory: setTargetParent, error: directoryError } = useProjectDirectory(client, open);
    const [targetLeaf, setTargetLeaf] = useState("");
    const [reviewed, setReviewed] = useState<TemplateReview>();
    const [submitted, setSubmitted] = useState(false);
    const [operationId, setOperationId] = useState<string>();
    const operations = useTemplateOperations(client);
    const operation = operations.snapshot(operationId ?? "")?.operation;
    const current = useActionScope(client);
    const draftRequest = useRef<object | undefined>(undefined);
    const callbacks = useRef({ onChanged, onCompleted, open });
    callbacks.current = { onChanged, onCompleted, open };
    const [loading, setLoading] = useState(true);
    const delayedLoading = useDelayedPending(open && loading);
    const [dialogShown, setDialogShown] = useState(false);
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState<RpcError>();

    useEffect(() => {
        if (!open) setDialogShown(false);
        else if (!loading || delayedLoading) setDialogShown(true);
    }, [open, loading, delayedLoading]);

    useEffect(() => {
        const generation = ++templateSession.current;
        draftRequest.current = undefined;
        setReviewed(undefined);
        setSubmitted(false);
        setOperationId(undefined);
        setBusy(false);
        setTargetLeaf("");
        templatePageRequest.current = undefined;
        setTemplateCursor(undefined);
        setTemplateLoadError(undefined);
        setMoreTemplatesLoading(false);
        setMoreTemplatesFailed(false);
        if (!open) {
            setTemplates([]);
            setTemplateId("");
            setLoading(true);
            setError(undefined);
            return;
        }
        let active = true;
        setLoading(true);
        setError(undefined);
        void client.templatesList().then((result) => {
            if (!active || generation !== templateSession.current) return;
            setTemplates(result.templates);
            setTemplateCursor(result.nextCursor);
            setTemplateId(result.templates[0]?.templateId ?? "");
        }).catch((caught: unknown) => {
            if (active && generation === templateSession.current) setTemplateLoadError(safeError(caught));
        }).finally(() => {
            if (active && generation === templateSession.current) setLoading(false);
        });
        return () => {
            active = false;
            if (generation === templateSession.current) templateSession.current += 1;
            templatePageRequest.current = undefined;
        };
    }, [client, open, templateReadVersion]);

    const loadMoreTemplates = async () => {
        if (templateCursor === undefined || loading || templatePageRequest.current !== undefined) return;
        const generation = templateSession.current;
        const request = {};
        templatePageRequest.current = request;
        setMoreTemplatesLoading(true);
        try {
            const next = await client.templatesList(templateCursor);
            if (generation !== templateSession.current || templatePageRequest.current !== request) return;
            setTemplates((current) => [...current, ...next.templates]);
            setTemplateCursor(next.nextCursor);
            setMoreTemplatesFailed(false);
        } catch {
            if (generation === templateSession.current && templatePageRequest.current === request) setMoreTemplatesFailed(true);
        } finally {
            if (generation === templateSession.current && templatePageRequest.current === request) {
                templatePageRequest.current = undefined;
                setMoreTemplatesLoading(false);
            }
        }
    };

    const close = () => {
        if (draftRequest.current !== undefined) return;
        templateSession.current += 1;
        templatePageRequest.current = undefined;
        onClose();
    };

    const chooseParent = async () => {
        if (draftRequest.current !== undefined) return;
        const generation = templateSession.current;
        const request = {};
        draftRequest.current = request;
        const ownsDraft = () => current() && generation === templateSession.current && draftRequest.current === request;
        setBusy(true); setError(undefined);
        try {
            const selected = await client.selectDirectory();
            if (ownsDraft() && selected !== undefined) setTargetParent(selected);
        } catch (caught: unknown) { if (ownsDraft()) setError(safeError(caught)); }
        finally { if (ownsDraft()) { draftRequest.current = undefined; setBusy(false); } }
    };

    const review = async () => {
        const template = templates.find((candidate) => candidate.templateId === templateId);
        if (template === undefined || draftRequest.current !== undefined) return;
        const generation = templateSession.current;
        const request = {};
        draftRequest.current = request;
        const ownsDraft = () => current() && generation === templateSession.current && draftRequest.current === request;
        setBusy(true); setError(undefined);
        try {
            const plan = await client.templatePlanCreateProject(template.templateId, template.revision, targetParent, targetLeaf.trim(), unity.version.trim());
            if (!ownsDraft()) return;
            setReviewed(reviewTemplateCreation(plan, template, targetParent, targetLeaf.trim(), unity.version.trim()));
            setSubmitted(false);
        } catch (caught: unknown) { if (ownsDraft()) setError(safeError(caught)); }
        finally { if (ownsDraft()) { draftRequest.current = undefined; setBusy(false); } }
    };

    const apply = async () => {
        if (reviewed === undefined || submitted || draftRequest.current !== undefined) return;
        const generation = templateSession.current;
        const request = {};
        draftRequest.current = request;
        const ownsDraft = () => current() && generation === templateSession.current && draftRequest.current === request;
        setSubmitted(true); setBusy(true); setError(undefined);
        try {
            const accepted = await client.templateApplyCreateProject(reviewed.plan.planId);
            operations.accept(accepted.operationId, async (next) => {
                if (!current()) return;
                const projectId = templateOperationProjectId(next);
                if (projectId === undefined) throw { code: "internal_error" };
                await callbacks.current.onChanged();
                if (current() && generation === templateSession.current && callbacks.current.open) callbacks.current.onCompleted(projectId);
            });
            if (ownsDraft()) setOperationId(accepted.operationId);
        } catch (caught: unknown) { if (ownsDraft()) setError(safeError(caught)); }
        finally { if (ownsDraft()) { draftRequest.current = undefined; setBusy(false); } }
    };

    const cancelOperation = async () => {
        if (operation === undefined || draftRequest.current !== undefined) return;
        const generation = templateSession.current;
        const request = {};
        draftRequest.current = request;
        const ownsDraft = () => current() && generation === templateSession.current && draftRequest.current === request;
        setBusy(true); setError(undefined);
        try {
            const result = await client.operationCancel(operation.operationId, operation.revision);
            if (result.operation.operationId !== operation.operationId) throw { code: "internal_error" };
        } catch (caught: unknown) { if (ownsDraft()) setError(safeError(caught)); }
        finally { if (ownsDraft()) { draftRequest.current = undefined; setBusy(false); } }
    };

    const terminal = operation !== undefined && !ACTIVE_OPERATION_STATES.has(operation.state);
    return (
        <><Dialog dismissible={!busy} onClose={close} open={open && (dialogShown || !loading || delayedLoading)} title={t("Create project")}>
            <div className="project-create-flow">
                {loading ? <LoadingState title={t("Loading templates…")} /> : null}
                {loading ? null : templateLoadError !== undefined ? (
                    <div className="dialog-empty" role="alert">
                        <strong>{t("Templates could not be loaded.")}</strong>
                        <code>{templateLoadError.code}</code>
                        <div className="dialog-actions"><Button onClick={close} type="button" variant="text">{t("Close")}</Button><Button onClick={() => setTemplateReadVersion((current) => current + 1)} type="button">{t("Retry")}</Button></div>
                    </div>
                ) : templates.length === 0 ? (
                    <div className="dialog-empty" role="status">
                        <strong>{t("No templates available")}</strong>
                        <span>{t("Import or create a template before creating a project.")}</span>
                        <div className="dialog-actions"><Button onClick={close} type="button">{t("Close")}</Button></div>
                    </div>
                ) : reviewed === undefined ? (
                    <>
                        <Select disabled={busy} aria-label={t("Project template")} id="project-create-template" label={t("Template")} onChange={setTemplateId} options={templates.map((template) => ({ label: `${template.displayName} · ${template.templateVersion}`, value: template.templateId }))} value={templateId} />
                        {templateCursor === undefined ? null : <div className="pagination-controls" aria-busy={moreTemplatesLoading}>
                            {moreTemplatesFailed ? <p className="inline-error" role="alert">{t("Could not load more items. Your loaded items are still available.")}</p> : null}
                            <Button disabled={busy || moreTemplatesLoading} onClick={() => void loadMoreTemplates()} type="button" variant="text">{moreTemplatesFailed ? t("Retry loading more") : t("Load more templates")}</Button>
                        </div>}
                        <div className="project-target-picker">
                            <TextField disabled={busy} aria-label={t("Create target parent")} label={t("Parent directory")} onInput={setTargetParent} value={targetParent} />
                            <Button disabled={busy} onClick={() => void chooseParent()} type="button" variant="tonal">{t("Choose directory")}</Button>
                        </div>
                        <CreateUnityVersionField disabled={busy} selection={unity} />
                        <TextField disabled={busy} aria-label={t("Create project name")} label={t("Project name")} onInput={setTargetLeaf} value={targetLeaf} />
                        <div className="dialog-actions">
                            <Button disabled={busy} onClick={close} type="button" variant="text">{t("Cancel")}</Button>
                            <Button disabled={busy || templateId.length === 0 || targetParent.length === 0 || targetLeaf.trim().length === 0 || unity.version.trim().length === 0} onClick={() => void review()} type="button">{t("Review creation")}</Button>
                        </div>
                    </>
                ) : operationId === undefined ? (
                    <>
                        <TemplatePlanReview review={reviewed} />
                        <p>{t("ALCOMD Core will validate the frozen template, create the project transactionally, and register the completed project.")}</p>
                        <div className="dialog-actions">
                            <Button disabled={busy} onClick={() => { setReviewed(undefined); setSubmitted(false); setError(undefined); }} type="button" variant="text">{t("Back")}</Button>
                            <Button disabled={busy || submitted} onClick={() => void apply()} type="button">{t("Create project")}</Button>
                        </div>
                    </>
                ) : (
                    <>
                        <TemplateOperationProgress operationId={operationId} operations={operations} />
                        <div className="dialog-actions">
                            {!terminal ? <Button disabled={busy || operation === undefined} onClick={() => void cancelOperation()} type="button" variant="text">{t("Cancel creation")}</Button> : null}
                            <Button disabled={busy} onClick={close} type="button">{t("Close")}</Button>
                        </div>
                    </>
                )}
                {directoryError === undefined ? null : <RequestReminder error={directoryError} title={t("Default project location could not be loaded.")} />}
                {error === undefined ? null : <RequestReminder error={error}>{submitted && templateApplyResultUnknown(error) ? t("The result of this request is unknown. Check Tasks and refresh before reviewing it again.") : undefined}</RequestReminder>}
            </div>
        </Dialog>{operations.follow()}</>
    );
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
                            <TextField aria-label={t("Restore target parent")} disabled={busy} label={t("Parent directory")} onInput={(value) => { setTargetParent(value); setError(undefined); }} value={targetParent} />
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
                            <div><span>{t("Destination")}</span><code>{displayPath(plan.target.parent)}\{plan.target.leaf}</code></div>
                            <div><span>{t("Package resolution")}</span><strong>{plan.packagesRequireResolve ? t("Required") : t("Not required")}</strong></div>
                            <div><span>{t("Plan")}</span><code>{plan.planFingerprint.slice(0, 12)}</code></div>
                        </div>
                        <p>{t("ALCOMD Core will validate the managed archive, restore transactionally, and register the completed project.")}</p>
                        {plan.packagesRequireResolve ? <Reminder tone="warning">{t("VPM packages require a separate resolve after restoration.")}</Reminder> : null}
                        <div className="dialog-actions">
                            <Button disabled={busy || (submitted && templateApplyResultUnknown(error))} onClick={() => { setPlan(undefined); setSubmitted(false); setError(undefined); }} type="button" variant="text">{t("Back")}</Button>
                            <Button disabled={!canRestore || busy || submitted} onClick={() => void apply()} type="button">{t("Restore project")}</Button>
                            {submitted ? <Button onClick={onClose} type="button" variant="text">{t("Close")}</Button> : null}
                        </div>
                    </>
                )}
                {directoryError === undefined ? null : <RequestReminder error={directoryError} title={t("Default project location could not be loaded.")} />}
                {error === undefined ? null : <RequestReminder error={error} title={t("Restore project")}>{submitted && templateApplyResultUnknown(error) ? t("The result of this request is unknown. Check Tasks and refresh before reviewing it again.") : undefined}</RequestReminder>}
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
