import type { RpcError } from "@alcomd/sdk";
import { useEffect, useRef, useState } from "react";

import type { BackupRecord, BackupRestorePlan, Operation, TemplatePlan, TemplateRecord } from "./core-models";
import { Button, Dialog, Select, TextField } from "./Material";
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

export function CreateProjectDialog({ client, onClose, onCompleted, open }: ProjectDialogProps) {
    const { t } = useTranslation();
    const [templates, setTemplates] = useState<TemplateRecord[]>([]);
    const [templateId, setTemplateId] = useState("");
    const [targetParent, setTargetParent] = useState("");
    const [targetLeaf, setTargetLeaf] = useState("");
    const [plan, setPlan] = useState<TemplatePlan>();
    const [operation, setOperation] = useState<Operation>();
    const [loading, setLoading] = useState(true);
    const delayedLoading = useDelayedPending(open && loading);
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState<RpcError>();
    const completionReported = useRef(false);

    useEffect(() => {
        if (!open) {
            setTemplates([]);
            setTemplateId("");
            setTargetParent("");
            setTargetLeaf("");
            setPlan(undefined);
            setOperation(undefined);
            setLoading(true);
            setBusy(false);
            setError(undefined);
            completionReported.current = false;
            return;
        }
        let active = true;
        setLoading(true);
        setError(undefined);
        void client.templatesList().then((result) => {
            if (!active) return;
            setTemplates(result.templates);
            setTemplateId(result.templates[0]?.templateId ?? "");
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
                if (next.state !== "succeeded" || completionReported.current) return;
                const projectId = operationProjectId(next);
                if (projectId === undefined) {
                    setError({ code: "internal_error", message: "The request could not be completed." });
                    return;
                }
                completionReported.current = true;
                onCompleted(projectId);
            }).catch((caught: unknown) => {
                if (active) setError(safeError(caught));
            });
        }, 300);
        return () => {
            active = false;
            window.clearTimeout(timer);
        };
    }, [client, onCompleted, operation]);

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
        const template = templates.find((candidate) => candidate.templateId === templateId);
        if (template === undefined) return;
        setBusy(true);
        setError(undefined);
        try {
            setPlan(await client.templatePlanCreateProject(template.templateId, template.revision, targetParent, targetLeaf.trim()));
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
            const accepted = await client.templateApplyCreateProject(plan.planId);
            const next = await client.operationGet(accepted.operationId);
            setOperation(next);
            if (next.state === "succeeded" && !completionReported.current) {
                const projectId = operationProjectId(next);
                if (projectId === undefined) {
                    setError({ code: "internal_error", message: "The request could not be completed." });
                } else {
                    completionReported.current = true;
                    onCompleted(projectId);
                }
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

    const selectedTemplate = templates.find((candidate) => candidate.templateId === templateId);
    const terminal = operation !== undefined && !ACTIVE_OPERATION_STATES.has(operation.state);
    return (
        <Dialog onClose={onClose} open={open && (!loading || delayedLoading)} title={t("Create project")}>
            <div className="project-create-flow">
                {loading ? <LoadingState title={t("Loading templates…")} /> : null}
                {loading ? null : templates.length === 0 ? (
                    <div className="dialog-empty" role="status">
                        <strong>{t("No templates available")}</strong>
                        <span>{t("Import or create a template before creating a project.")}</span>
                        <div className="dialog-actions"><Button onClick={onClose} type="button">{t("Close")}</Button></div>
                    </div>
                ) : plan === undefined ? (
                    <>
                        <Select aria-label={t("Project template")} label={t("Template")} onChange={setTemplateId} options={templates.map((template) => ({ label: `${template.displayName} · ${template.templateVersion}`, value: template.templateId }))} value={templateId} />
                        <div className="project-target-picker">
                            <TextField aria-label={t("Create target parent")} label={t("Parent directory")} onInput={setTargetParent} value={targetParent} />
                            <Button disabled={busy} onClick={() => void chooseParent()} type="button" variant="tonal">{t("Choose directory")}</Button>
                        </div>
                        <TextField aria-label={t("Create project name")} label={t("Project name")} onInput={setTargetLeaf} value={targetLeaf} />
                        <div className="dialog-actions">
                            <Button disabled={busy} onClick={onClose} type="button" variant="text">{t("Cancel")}</Button>
                            <Button disabled={busy || templateId.length === 0 || targetParent.length === 0 || targetLeaf.trim().length === 0} onClick={() => void review()} type="button">{t("Review creation")}</Button>
                        </div>
                    </>
                ) : operation === undefined ? (
                    <>
                        <div className="dialog-summary">
                            <div><span>{t("Template")}</span><strong>{selectedTemplate?.displayName ?? templateId}</strong></div>
                            <div><span>{t("Version")}</span><strong>{selectedTemplate?.templateVersion ?? t("Unknown")}</strong></div>
                            <div><span>{t("Destination")}</span><code>{targetParent}\{targetLeaf.trim()}</code></div>
                            <div><span>{t("Plan")}</span><code>{plan.planFingerprint.slice(0, 12)}</code></div>
                        </div>
                        <p>{t("ALCOMD Core will validate the frozen template, create the project transactionally, and register the completed project.")}</p>
                        <div className="dialog-actions">
                            <Button disabled={busy} onClick={() => setPlan(undefined)} type="button" variant="text">{t("Back")}</Button>
                            <Button disabled={busy} onClick={() => void apply()} type="button">{t("Create project")}</Button>
                        </div>
                    </>
                ) : (
                    <>
                        <p role="status">{operationFeedback(operation, "creation", t)}</p>
                        <div className="dialog-actions">
                            {!terminal ? <Button disabled={busy} onClick={() => void cancelOperation()} type="button" variant="text">{t("Cancel creation")}</Button> : null}
                            <Button disabled={!terminal} onClick={onClose} type="button">{t("Close")}</Button>
                        </div>
                    </>
                )}
                {error === undefined ? null : <p className="inline-error" role="alert">{t("Project creation failed: {code}", { code: error.code })}</p>}
            </div>
        </Dialog>
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
                        <p role="status">{operationFeedback(operation, "restore", t)}</p>
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

function operationProjectId(operation: Operation): string | undefined {
    if (typeof operation.result !== "object" || operation.result === null) return undefined;
    const projectId = (operation.result as { projectId?: unknown }).projectId;
    return typeof projectId === "string" && projectId.length > 0 ? projectId : undefined;
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

function operationFeedback(operation: Operation, kind: "creation" | "restore", t: Translate): string {
    const states: Record<string, string> = {
        queued: t("Queued"), planning: t("Planning"), waiting_for_input: t("Waiting for input"),
        running: t("Running"), cancelling: t("Cancelling"), recovering: t("Recovering"),
        interrupted: t("Interrupted"), succeeded: t("Succeeded"), failed: t("Failed"), cancelled: t("Cancelled")
    };
    const state = states[operation.state] ?? operation.state;
    const phase = operation.progress?.phase;
    if (kind === "creation") return phase === undefined
        ? t("Project creation {state}.", { state })
        : t("Project creation {state}: {phase}.", { state, phase });
    return phase === undefined
        ? t("Project restore {state}.", { state })
        : t("Project restore {state}: {phase}.", { state, phase });
}
