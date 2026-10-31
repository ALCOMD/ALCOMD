import type { RpcError } from "@alcomd/sdk";
import { useEffect, useRef, useState } from "react";

import type { BackupRecord, BackupRestorePlan, Operation, TemplateRecord } from "./core-models";
import { Button, Dialog, Select, TextField } from "./Material";
import { TemplateOperationProgress, templateApplyResultUnknown, templateOperationProjectId, useTemplateDraft, useTemplateOperations } from "./CoreActions";
import { reviewTemplateCreation, TemplatePlanReview, type TemplateReview } from "./TemplatePlanReview";
import { LoadingState, useDelayedPending } from "./LoadingState";
import type { GuiRpcClient } from "./rpc";
import { useTranslation } from "./i18n";
import type { SupportedLocale, Translate } from "./i18n-types";

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
    const [targetParent, setTargetParent] = useState("");
    const [targetLeaf, setTargetLeaf] = useState("");
    const [reviewed, setReviewed] = useState<TemplateReview>();
    const [submitted, setSubmitted] = useState(false);
    const [operationId, setOperationId] = useState<string>();
    const operations = useTemplateOperations(client);
    const operation = operations.snapshot(operationId ?? "")?.operation;
    const current = useTemplateDraft(client);
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
        setTargetParent("");
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
            const plan = await client.templatePlanCreateProject(template.templateId, template.revision, targetParent, targetLeaf.trim());
            if (!ownsDraft()) return;
            setReviewed(reviewTemplateCreation(plan, template, targetParent, targetLeaf.trim()));
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
                        <TextField disabled={busy} aria-label={t("Create project name")} label={t("Project name")} onInput={setTargetLeaf} value={targetLeaf} />
                        <div className="dialog-actions">
                            <Button disabled={busy} onClick={close} type="button" variant="text">{t("Cancel")}</Button>
                            <Button disabled={busy || templateId.length === 0 || targetParent.length === 0 || targetLeaf.trim().length === 0} onClick={() => void review()} type="button">{t("Review creation")}</Button>
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
                {error === undefined ? null : <div className="inline-error" role="alert"><p>{t("Project creation request failed: {code}", { code: error.code })}</p>{submitted && templateApplyResultUnknown(error) ? <p>{t("The result of this request is unknown. Check Tasks and refresh before reviewing it again.")}</p> : null}</div>}
            </div>
        </Dialog>{operations.follow()}</>
    );
}

export function RestoreProjectDialog({ client, onClose, onCompleted, open }: ProjectDialogProps) {
    const { t, locale } = useTranslation();
    const [backups, setBackups] = useState<BackupRecord[]>([]);
    const [backupId, setBackupId] = useState("");
    const [targetParent, setTargetParent] = useState("");
    const [targetLeaf, setTargetLeaf] = useState("");
    const [plan, setPlan] = useState<BackupRestorePlan>();
    const [operation, setOperation] = useState<Operation>();
    const [restoredProjectId, setRestoredProjectId] = useState<string>();
    const [loading, setLoading] = useState(true);
    const delayedLoading = useDelayedPending(open && loading);
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState<RpcError>();
    const completionReported = useRef(false);

    useEffect(() => {
        if (!open) {
            setBackups([]);
            setBackupId("");
            setTargetParent("");
            setTargetLeaf("");
            setPlan(undefined);
            setOperation(undefined);
            setRestoredProjectId(undefined);
            setLoading(true);
            setBusy(false);
            setError(undefined);
            completionReported.current = false;
            return;
        }
        let active = true;
        setLoading(true);
        setError(undefined);
        void client.backupsList().then((result) => {
            if (!active) return;
            setBackups(result.backups);
            setBackupId(result.backups[0]?.backupId ?? "");
        }).catch((caught: unknown) => {
            if (active) setError(safeError(caught));
        }).finally(() => {
            if (active) setLoading(false);
        });
        return () => { active = false; };
    }, [client, open]);

    useEffect(() => {
        if (operation === undefined || !ACTIVE_OPERATION_STATES.has(operation.state)) return;
        let active = true;
        const timer = window.setTimeout(() => {
            void client.operationGet(operation.operationId).then((next) => {
                if (!active) return;
                setOperation(next);
                if (next.state !== "succeeded" || restoredProjectId === undefined || completionReported.current) return;
                completionReported.current = true;
                onCompleted(restoredProjectId);
            }).catch((caught: unknown) => {
                if (active) setError(safeError(caught));
            });
        }, 300);
        return () => {
            active = false;
            window.clearTimeout(timer);
        };
    }, [client, onCompleted, operation, restoredProjectId]);

    const chooseParent = async () => {
        setBusy(true);
        setError(undefined);
        try {
            const selected = await client.selectDirectory();
            if (selected !== undefined) setTargetParent(selected);
        } catch (caught: unknown) {
            setError(safeError(caught));
        } finally {
            setBusy(false);
        }
    };

    const review = async () => {
        setBusy(true);
        setError(undefined);
        try {
            setPlan(await client.backupPlanRestore(backupId, targetParent, targetLeaf.trim()));
        } catch (caught: unknown) {
            setError(safeError(caught));
        } finally {
            setBusy(false);
        }
    };

    const apply = async () => {
        if (plan === undefined) return;
        setBusy(true);
        setError(undefined);
        try {
            const accepted = await client.backupApplyRestore(plan.planId);
            setRestoredProjectId(accepted.projectId);
            const next = await client.operationGet(accepted.operationId);
            setOperation(next);
            if (next.state === "succeeded" && !completionReported.current) {
                completionReported.current = true;
                onCompleted(accepted.projectId);
            }
        } catch (caught: unknown) {
            setError(safeError(caught));
        } finally {
            setBusy(false);
        }
    };

    const cancelOperation = async () => {
        if (operation === undefined) return;
        setBusy(true);
        setError(undefined);
        try {
            const result = await client.operationCancel(operation.operationId, operation.revision);
            setOperation(result.operation);
        } catch (caught: unknown) {
            setError(safeError(caught));
        } finally {
            setBusy(false);
        }
    };

    const selectedBackup = backups.find((candidate) => candidate.backupId === backupId);
    const terminal = operation !== undefined && !ACTIVE_OPERATION_STATES.has(operation.state);
    return (
        <Dialog onClose={onClose} open={open && (!loading || delayedLoading)} title={t("Restore project")}>
            <div className="project-create-flow">
                {loading ? <LoadingState title={t("Loading backups…")} /> : null}
                {loading ? null : backups.length === 0 ? (
                    <div className="dialog-empty" role="status">
                        <strong>{t("No managed backups available")}</strong>
                        <span>{t("Create a project backup before restoring a project.")}</span>
                        <div className="dialog-actions"><Button onClick={onClose} type="button">{t("Close")}</Button></div>
                    </div>
                ) : plan === undefined ? (
                    <>
                        <Select aria-label={t("Project backup")} label={t("Backup")} onChange={setBackupId} options={backups.map((backup) => ({ label: backupLabel(backup, locale), value: backup.backupId }))} value={backupId} />
                        <div className="project-target-picker">
                            <TextField aria-label={t("Restore target parent")} label={t("Parent directory")} onInput={setTargetParent} value={targetParent} />
                            <Button disabled={busy} onClick={() => void chooseParent()} type="button" variant="tonal">{t("Choose directory")}</Button>
                        </div>
                        <TextField aria-label={t("Restore project name")} label={t("Project name")} onInput={setTargetLeaf} value={targetLeaf} />
                        <div className="dialog-actions">
                            <Button disabled={busy} onClick={onClose} type="button" variant="text">{t("Cancel")}</Button>
                            <Button disabled={busy || backupId.length === 0 || targetParent.length === 0 || targetLeaf.trim().length === 0} onClick={() => void review()} type="button">{t("Review restore")}</Button>
                        </div>
                    </>
                ) : operation === undefined ? (
                    <>
                        <div className="dialog-summary">
                            <div><span>{t("Backup")}</span><strong>{selectedBackup === undefined ? backupId : backupLabel(selectedBackup, locale)}</strong></div>
                            <div><span>{t("Destination")}</span><code>{plan.target.parent}\{plan.target.leaf}</code></div>
                            <div><span>{t("Package resolution")}</span><strong>{plan.packagesRequireResolve ? t("Required") : t("Not required")}</strong></div>
                            <div><span>{t("Plan")}</span><code>{plan.planFingerprint.slice(0, 12)}</code></div>
                        </div>
                        <p>{t("ALCOMD Core will validate the managed archive, restore transactionally, and register the completed project.")}</p>
                        <div className="dialog-actions">
                            <Button disabled={busy} onClick={() => setPlan(undefined)} type="button" variant="text">{t("Back")}</Button>
                            <Button disabled={busy} onClick={() => void apply()} type="button">{t("Restore project")}</Button>
                        </div>
                    </>
                ) : (
                    <>
                        <p role="status">{operationFeedback(operation, t)}</p>
                        <div className="dialog-actions">
                            {!terminal ? <Button disabled={busy} onClick={() => void cancelOperation()} type="button" variant="text">{t("Cancel restore")}</Button> : null}
                            <Button disabled={!terminal} onClick={onClose} type="button">{t("Close")}</Button>
                        </div>
                    </>
                )}
                {error === undefined ? null : <p className="inline-error" role="alert">{t("Project restore failed: {code}", { code: error.code })}</p>}
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

function operationFeedback(operation: Operation, t: Translate): string {
    const states: Record<string, string> = {
        queued: t("Queued"), planning: t("Planning"), waiting_for_input: t("Waiting for input"),
        running: t("Running"), cancelling: t("Cancelling"), recovering: t("Recovering"),
        interrupted: t("Interrupted"), succeeded: t("Succeeded"), failed: t("Failed"), cancelled: t("Cancelled")
    };
    const state = states[operation.state] ?? operation.state;
    const phase = operation.progress?.phase;
    return phase === undefined
        ? t("Project restore {state}.", { state })
        : t("Project restore {state}: {phase}.", { state, phase });
}
