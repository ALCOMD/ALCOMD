import { useGuiWrite } from "./gui-write-attempt";
import { BLANK_PROJECT_TEMPLATE_ID } from "@alcomd/sdk";
import { requestErrorMessage } from "./request-errors";
import type { BackupRestoreParams, ProjectsCreateParams, ProjectTemplateRef, ProjectTemplatesListResult, RpcError } from "@alcomd/sdk";
import { useCallback, useEffect, useRef, useState } from "react";

import type { Operation } from "./core-models";
import { DialogCloseButton, Button, Dialog, PathField, Select, TextField } from "./Material";
import { AcceptedOperationProgress, OperationFollow, operationRequestOutcomeUnknown, operationResultProjectId, useAcceptedOperations, useActionScope } from "./CoreActions";
import { useCoreEvents, useWorkspaceInvalidation } from "./core-events-ui";
import { capabilities, useCapability } from "./capabilities";
import { LoadingState } from "./LoadingState";
import type { GuiRpcClient } from "./rpc";
import { useTranslation } from "./i18n";
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
    const writeRequest = useGuiWrite(client, open);
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
    const { directory: targetParent, setDirectory: setTargetParent, error: directoryError, projectTarget } = useProjectDirectory(client, open);
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
            setSelection((previous) => {
                if (previous !== undefined) return previous;
                const blank = next.records.find(({ id }) => id === BLANK_PROJECT_TEMPLATE_ID);
                return blank === undefined ? undefined : { id: blank.id, revision: blank.revision };
            });
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
        const currentPath = targetParent || await client.settingsGet().then((value) => value.directories.projectDirectory, () => undefined);
        if (!ownsDraft()) return;
        const directory = await client.selectDirectory(currentPath);
        if (ownsDraft() && directory !== undefined) setTargetParent(directory);
    });
    const retrying = operationRequestOutcomeUnknown(error) && attempt.current?.client === client;
    const ready = retrying || ((!canUseTemplates || selection !== undefined) && !staleSelection && name.trim().length > 0 && targetParent.length > 0 && unity.version.trim().length > 0);
    const formDisabled = busy || retrying;
    const create = () => run(async (ownsDraft) => {
        if (!ready) return;
        const generation = draft.current;
        const input = {
            target: projectTarget(name.trim()),
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

        });
        if (ownsDraft()) {
            attempt.current = undefined;
            onClose();
        }
    });
    const cancelOperation = () => run(async () => {
        if (operation === undefined) return;
        const result = await writeRequest("operationCancel", operation.operationId, operation.revision);
        if (result.operation.operationId !== operation.operationId) throw { code: "internal_error" };
    });

    const terminal = operation !== undefined && !ACTIVE_OPERATION_STATES.has(operation.state);
    const nameCounts = new Map<string, number>();
    for (const { name } of catalog?.templates ?? []) {
        nameCounts.set(name, (nameCounts.get(name) ?? 0) + 1);
    }
    const options = [
        ...(catalog?.templates ?? []).map(({ id, name, editorVersion }) => {
            const displayName = id === BLANK_PROJECT_TEMPLATE_ID ? t("Blank") : name;
            const label = editorVersion === undefined ? displayName : displayName + " · " + editorVersion;
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
                            options={options} value={staleSelection ? "" : selection?.id ?? ""} />
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
                        <DialogCloseButton disabled={busy} variant="text">{t("Cancel")}</DialogCloseButton>
                        <Button disabled={busy || !ready} onClick={() => void create()} type="button">{t("Create project")}</Button>
                    </div>
                </> : <>
                    <AcceptedOperationProgress operationId={operationId} operations={operations} />
                    <div className="dialog-actions">
                        {!terminal ? <Button disabled={busy || operation === undefined} onClick={() => void cancelOperation()} type="button" variant="text">{t("Cancel creation")}</Button> : null}
                        <DialogCloseButton disabled={busy}>{t("Close")}</DialogCloseButton>
                    </div>
                </>}
                {directoryError === undefined ? null : <div role="alert"><strong>{t("Default project location could not be loaded.")}</strong><p>{t(requestErrorMessage(directoryError.code))}</p><code>{directoryError.code}</code>{directoryError.diagnosticId === undefined ? null : <p><code>{directoryError.diagnosticId}</code></p>}</div>}
                {error === undefined ? null : <div role="alert" data-notification-error={error.code}><p>{t(requestErrorMessage(error.code))}</p><code>{error.code}</code>{error.diagnosticId === undefined ? null : <p><code>{error.diagnosticId}</code></p>}{operationId === undefined && operationRequestOutcomeUnknown(error) ? <p>{t("Retry with the same details to recover the original creation request.")}</p> : null}</div>}
            </div>
        </Dialog>
        {operations.follow()}
    </>;
}

export function RestoreProjectDialog({ client, onClose, onCompleted, onViewOperation, open }: ProjectDialogProps & { onViewOperation?(operationId: string): void }) {
    const { t } = useTranslation();
    const writeRequest = useGuiWrite(client, open);
    const [archivePath, setArchivePath] = useState("");
    const [projectName, setProjectName] = useState("");
    const [destinationPath, setDestinationPath] = useState("");
    const [operationId, setOperationId] = useState<string>();
    const [operation, setOperation] = useState<Operation>();
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState<RpcError>();
    const inFlight = useRef(false);
    const attempt = useRef<BackupRestoreParams | undefined>(undefined);
    const isCurrent = useActionScope(client, open);
    const canRestore = useCapability(capabilities.backupsRestore);
    const outcomeUnknown = operationRequestOutcomeUnknown(error);
    const terminal = operation !== undefined && !ACTIVE_OPERATION_STATES.has(operation.state);
    const restored = operation?.state === "succeeded" ? operationResultProjectId(operation) : undefined;

    const changeArchive = (path: string) => {
        setArchivePath(path);
        setError(undefined);
    };

    const chooseArchive = async () => {
        if (inFlight.current || outcomeUnknown) return;
        inFlight.current = true;
        setBusy(true);
        try {
            const settings = archivePath.length === 0 ? await client.settingsGet().catch(() => undefined) : undefined;
            if (!isCurrent()) return;
            const selected = await client.selectFile("backupZip", settings?.directories.backupDirectory ?? archivePath);
            if (isCurrent() && selected !== undefined) changeArchive(selected);
        } catch (caught: unknown) {
            if (isCurrent()) setError(safeError(caught));
        } finally {
            inFlight.current = false;
            if (isCurrent()) setBusy(false);
        }
    };

    const restore = async () => {
        if (!canRestore || inFlight.current || archivePath.trim().length === 0) return;
        const params = outcomeUnknown ? attempt.current : {
            archivePath,
            ...(destinationPath.trim().length === 0
                ? projectName.trim().length === 0 ? {} : { projectName: projectName.trim() }
                : { destinationPath }),
            idempotencyKey: crypto.randomUUID(),
        };
        if (params === undefined) return;
        attempt.current = params;
        inFlight.current = true;
        setBusy(true);
        setError(undefined);
        try {
            const accepted = await client.backupRestore(params);
            if (isCurrent()) setOperationId(accepted.operationId);
        } catch (caught: unknown) {
            if (isCurrent()) setError(safeError(caught));
        } finally {
            inFlight.current = false;
            if (isCurrent()) setBusy(false);
        }
    };

    const cancelOperation = async () => {
        if (operation === undefined || inFlight.current || operation.state === "cancelling") return;
        inFlight.current = true;
        setBusy(true);
        try {
            const result = await writeRequest("operationCancel", operation.operationId, operation.revision);
            if (isCurrent()) setOperation(result.operation);
        } catch (caught: unknown) {
            if (isCurrent()) setError(safeError(caught));
        } finally {
            inFlight.current = false;
            if (isCurrent()) setBusy(false);
        }
    };

    const suggestedName = archivePath.split(/[\\/]/).at(-1)?.replace(/\.zip$/i, "") ?? "";
    return <Dialog dismissible={!busy} onClose={onClose} open={open} title={t("Restore project")}>
        <div className="project-create-flow">
            {operationId === undefined ? <>
                <div className="project-target-picker">
                    <PathField disabled={busy || outcomeUnknown} label={t("Backup")} onInput={changeArchive} value={archivePath} />
                    <Button disabled={busy || outcomeUnknown} onClick={() => void chooseArchive()} type="button" variant="tonal">{t("Select")}</Button>
                </div>
                <TextField disabled={busy || outcomeUnknown || destinationPath.trim().length > 0} label={t("Project name")} onInput={(value) => { setProjectName(value); setError(undefined); }} placeholder={suggestedName} value={projectName} />
                <PathField disabled={busy || outcomeUnknown} label={t("Destination")} onInput={(value) => { setDestinationPath(value); setError(undefined); }} value={destinationPath} />
                <div className="dialog-actions">
                    <DialogCloseButton disabled={busy} variant="text">{t("Cancel")}</DialogCloseButton>
                    <Button disabled={!canRestore || busy || archivePath.trim().length === 0} onClick={() => void restore()} type="button">{t(outcomeUnknown ? "Retry" : "Restore project")}</Button>
                </div>
            </> : <>
                <OperationFollow client={client} onSnapshot={setOperation} operationId={operationId} title={t("Restore project")} />
                {operation?.state === "succeeded" ? <p>{t("Task completed.")}</p> : null}
                <div className="dialog-actions">
                    {operation === undefined || terminal ? null : <Button disabled={busy || operation.state === "cancelling"} onClick={() => void cancelOperation()} type="button" variant="text">{t("Cancel restore")}</Button>}
                    {onViewOperation === undefined ? null : <Button onClick={() => onViewOperation(operationId)} type="button" variant="text">{t("View operation")}</Button>}
                    {restored === undefined ? null : <Button onClick={() => onCompleted(restored)} type="button">{t("Open project")}</Button>}
                    <DialogCloseButton disabled={busy}>{t(terminal ? "Close" : "Minimize")}</DialogCloseButton>
                </div>
            </>}
            {error === undefined ? null : <div role="alert" data-notification-error={error.code}><p>{t(requestErrorMessage(error.code))}</p><code>{error.code}</code>{error.diagnosticId === undefined ? null : <p><code>{error.diagnosticId}</code></p>}{outcomeUnknown ? t("The result of this request is unknown. Check Tasks and refresh before reviewing it again.") : null}</div>}
        </div>
    </Dialog>;
}

function safeError(caught: unknown): RpcError {
    if (typeof caught === "object" && caught !== null && "code" in caught && typeof caught.code === "string") {
        return { code: caught.code, message: "The request could not be completed.", ...("diagnosticId" in caught && typeof caught.diagnosticId === "string" ? { diagnosticId: caught.diagnosticId } : {}) };
    }
    return { code: "internal_error", message: "The request could not be completed." };
}
