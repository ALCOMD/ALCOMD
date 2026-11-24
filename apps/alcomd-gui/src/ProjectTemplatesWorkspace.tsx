import { BLANK_PROJECT_TEMPLATE_ID } from "@alcomd/sdk";
import type { ProjectTemplatesListResult, ProjectTemplateAttachment, ProjectTemplateDependency, ProjectTemplateEditParams, ProjectTemplateGetResult, ProjectTemplateRecord, ProjectTemplateSource, ProjectTemplatesExportParams, ProjectTemplatesRemoveParams, ProjectTemplatesSaveParams, RpcError } from "@alcomd/sdk";
import { closeIcon, moreVertIcon } from "@alcomd/ui/icons";
import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";

import { AcceptedOperationProgress, operationRequestOutcomeUnknown, useAcceptedOperations, useActionScope } from "./CoreActions";
import { capabilities, useCapability } from "./capabilities";
import { DesktopList, TableLoading } from "./DesktopList";
import { DialogCloseButton, Button, Checkbox, Dialog, Icon, IconButton, Menu, MenuItem, PathField, Select, TextField, type DialogOrigin } from "./Material";
import { FilePicker } from "./FilePicker";
import { PagedItems } from "./PagedItems";
import { SplitButton } from "./SplitButton";
import { UtilityWorkspace } from "./UtilityWorkspace";
import type { ProjectSnapshot, ProjectsListResult } from "./core-models";
import { displayPath } from "./display-path";
import { useTranslation } from "./i18n";
import { useCoreEvents } from "./core-events-ui";
import { navigationLabels } from "./navigation-labels";
import { normalizeRequestError, requestErrorMessage } from "./request-errors";
import type { GuiRpcClient } from "./rpc";
import { CreateUnityVersionField, fixedVersionTemplateLabel, useCreateUnityVersion } from "./use-create-unity-version";

type SourceKind = ProjectTemplateSource["kind"];
type Selection = { kind: "save"; source: SourceKind; template?: ProjectTemplateRecord }
    | { kind: "view" | "edit" | "export" | "remove"; template: ProjectTemplateRecord };
type Request =
    | { kind: "save"; params: Omit<ProjectTemplatesSaveParams, "idempotencyKey"> }
    | { kind: "export"; params: Omit<ProjectTemplatesExportParams, "idempotencyKey"> }
    | { kind: "remove"; params: Omit<ProjectTemplatesRemoveParams, "idempotencyKey"> }
    | { kind: "edit"; params: Omit<ProjectTemplateEditParams, "idempotencyKey"> };
type SelectedProject = ProjectSnapshot & { projectId: string; revision: number };

const sourceLabels = {
    manual: "Create template",
    file: "Import template",
    project: "Create template from project"
};

export function ImportDesktopTemplate({ client, path, onClose }: { client: GuiRpcClient; path: string; onClose(): void }) {
    const events = useCoreEvents();
    return <TemplateDialog client={client} open origin={{ current: null }} initialPath={path}
        selection={{ kind: "save", source: "file" }} catalog={{ templates: [] }}
        onChanged={() => events?.invalidate("templates", true)} onClose={onClose} />;
}

export function ProjectTemplatesWorkspace({ client, navigation }: { client: GuiRpcClient; navigation: ReactNode }) {
    const { t } = useTranslation();
    const [selection, setSelection] = useState<(Selection & { origin: DialogOrigin; key: number; closed: boolean })>();
    const session = useRef(0);
    const createButton = useRef<HTMLElement>(null);
    const createMenu = useRef<HTMLElement>(null);
    const load = useCallback(() => client.projectTemplatesList(), [client]);

    useEffect(() => { setSelection(undefined); }, [client]);

    function select(next: Selection, origin: HTMLElement | null) {
        setSelection({ ...next, key: ++session.current, closed: false, origin: { get current() { return origin?.isConnected ? origin : createButton.current; } } });
    }

    return <UtilityWorkspace table loading={<TableLoading label={t("Templates")} headers={[t("Template name"), t("Template ID"), t("Template source"), t("Actions")]} />} changes={["templates"]} title={t(navigationLabels.resources)} navigation={navigation} load={load}
        tools={() => <SplitButton label={t("Create template")} menuButtonRef={createMenu} menuLabel={t("More template creation options")}
            primaryRef={createButton} onClick={() => select({ kind: "save", source: "manual" }, createButton.current)}>
            <MenuItem label={t("Create template from project")} onClick={() => select({ kind: "save", source: "project" }, createMenu.current)} />
            <MenuItem label={t("Import template")} onClick={() => select({ kind: "save", source: "file" }, createMenu.current)} />
        </SplitButton>}>
        {(value, refresh) => <>
            <PagedItems initialItems={value.templates} initialCursor={value.nextCursor} loadMore={async (cursor) => {
                const next = await client.projectTemplatesList(cursor);
                return { items: next.templates, nextCursor: next.nextCursor };
            }}>
                {(templates) => templates.length === 0 ? <p>{t("No templates available")}</p> : <DesktopList
                    label={t("Templates")} minWidth={640}
                    columns={[
                        { key: "name", header: t("Template name") },
                        { key: "id", header: t("Template ID") },
                        { key: "source", header: t("Template source") },
                        { key: "actions", header: t("Actions") }
                    ]}
                    rows={templates.map((template) => ({ key: template.id, cells: [
                        <strong>{template.id === BLANK_PROJECT_TEMPLATE_ID ? t("Blank") : template.name}</strong>,
                        <span className="template-id">{template.id}</span>,
                        t(template.locked ? "Built-in" : "User template"),
                        <TemplateRowActions onSelect={select} template={template} />
                    ] }))} />}
            </PagedItems>
            {selection === undefined ? null : <TemplateDialog
                client={client}
                key={selection.key}
                onChanged={refresh}
                onClose={() => setSelection((current) => current?.key === selection.key && !current.closed ? { ...current, closed: true } : current)}
                open={!selection.closed}
                origin={selection.origin}
                selection={selection}
                catalog={value}
            />}
        </>}
    </UtilityWorkspace>;
}

function TemplateRowActions({ onSelect, template }: { onSelect(selection: Selection, origin: HTMLElement | null): void; template: ProjectTemplateRecord }) {
    const { t } = useTranslation();
    const canEdit = useCapability(capabilities.templateEdit);
    const anchor = useRef<HTMLElement>(null);
    const [open, setOpen] = useState(false);
    function select(selection: Selection) {
        setOpen(false);
        onSelect(selection, anchor.current);
    }
    return <div className="card-actions">
        <IconButton aria-expanded={open} label={t("More actions for {name}", { name: template.name })}
            onClick={() => setOpen(true)} ref={anchor} type="button"><Icon asset={moreVertIcon} size={24} /></IconButton>
        <Menu anchorRef={anchor} onClose={() => setOpen(false)} open={open}>
            <MenuItem label={t("View template")} onClick={() => select({ kind: "view", template })} />
            {canEdit ? <MenuItem disabled={template.locked} label={t("Edit template")} onClick={() => select({ kind: "edit", template })} /> : null}
            <MenuItem label={t("Export template")} onClick={() => select({ kind: "export", template })} />
            <MenuItem disabled={template.locked} label={t("Replace template")} onClick={() => select({ kind: "save", source: "file", template })} />
            <MenuItem disabled={template.locked} label={t("Remove template")} onClick={() => select({ kind: "remove", template })} />
        </Menu>
    </div>;
}

function TemplateDialog({ client, onChanged, onClose, open, origin, selection, catalog, initialPath }: {
    client: GuiRpcClient;
    onChanged(): void;
    onClose(): void;
    open: boolean;
    origin: DialogOrigin;
    selection: Selection;
    catalog: ProjectTemplatesListResult;
    initialPath?: string;
}) {
    const { t } = useTranslation();
    const canReadDetail = useCapability(capabilities.templateEdit);
    const record = selection.template;
    const [source, setSource] = useState<SourceKind>(selection.kind === "save" ? selection.source : "manual");
    const [id, setId] = useState(record?.id ?? "");
    const [name, setName] = useState(record?.name ?? "");
    const [dependencies, setDependencies] = useState<(ProjectTemplateDependency & { key: string })[]>([]);
    const [detail, setDetail] = useState<ProjectTemplateGetResult>();
    const [detailError, setDetailError] = useState<RpcError>();
    const [detailRetry, setDetailRetry] = useState(0);
    const [addAttachments, setAddAttachments] = useState<(ProjectTemplateAttachment & { key: string })[]>([]);
    const [removeAttachments, setRemoveAttachments] = useState<string[]>([]);
    const [project, setProject] = useState<SelectedProject>();
    const [base, setBase] = useState<ProjectTemplateRecord>();
    const [baseValid, setBaseValid] = useState(false);
    const [includeAssets, setIncludeAssets] = useState(false);
    const [lockPackageVersions, setLockPackageVersions] = useState(false);
    const [sourceFile, setSourceFile] = useState(initialPath ?? "");
    const [destinationFile, setDestinationFile] = useState("");
    const [destinationHint, setDestinationHint] = useState("");
    const [fileName, setFileName] = useState(record === undefined ? "" : record.id + ".alcomdtemplate");
    const [busy, setBusy] = useState(false);
    const [choosing, setChoosing] = useState(false);
    const [error, setError] = useState<RpcError>();
    const [operationId, setOperationId] = useState<string>();
    const pending = useRef(false);
    const attempt = useRef<{ request: Request; idempotencyKey: string } | undefined>(undefined);
    const current = useActionScope(client, open);
    const operations = useAcceptedOperations(client);
    const operation = operationId === undefined ? undefined : operations.snapshot(operationId)?.operation;
    const unity = useCreateUnityVersion(client, base?.editorVersion, open && selection.kind === "save" && source === "manual");
    const detailId = canReadDetail && (selection.kind === "view" || selection.kind === "edit") ? record?.id : undefined;

    useEffect(() => {
        if (!open || detailId === undefined) return;
        let active = true;
        setDetail(undefined);
        setDetailError(undefined);
        void client.projectTemplateGet({ id: detailId }).then((result) => {
            if (!active) return;
            setDetail(result);
            setName(result.template.name);
            setDependencies(result.dependencies.map((entry) => ({ ...entry, key: entry.packageId })));
        }).catch((caught: unknown) => { if (active) setDetailError(normalizeRequestError(caught)); });
        return () => { active = false; };
    }, [client, detailId, detailRetry, open]);

    async function run(action: () => Promise<void>) {
        if (pending.current) return;
        pending.current = true;
        setBusy(true);
        setError(undefined);
        try { await action(); }
        catch (caught: unknown) { if (current()) setError(normalizeRequestError(caught)); }
        finally {
            pending.current = false;
            if (current()) setBusy(false);
        }
    }

    const retrying = attempt.current !== undefined && operationRequestOutcomeUnknown(error);
    const formDisabled = busy || retrying;
    const selectDestination = () => run(async () => {
        const path = await client.selectProjectTemplateFile("save", fileName.trim(), destinationFile || destinationHint);
        if (!current() || path === undefined) return;
        setDestinationFile(path);
        setDestinationHint(path);
    });

    const selectSource = () => run(async () => {
        const path = await client.selectProjectTemplateFile("open", undefined, sourceFile);
        if (!current() || path === undefined) return;
        setSourceFile(path);
    });

    function saveSource(): ProjectTemplateSource | undefined {
        if (source === "file") return sourceFile.trim() === "" ? undefined : { kind: "file", sourceFile };
        if (id.trim() === "" || name.trim() === "") return undefined;
        if (source === "project") return project === undefined ? undefined : {
            kind: "project", projectId: project.projectId, expectedProjectRevision: project.revision,
            id: id.trim(), name: name.trim(), includeAssets, lockPackageVersions
        };
        if (base === undefined || !baseValid || unity.version.trim() === "" || dependencies.some(({ packageId, versionRange }) => packageId.trim() === "" || versionRange.trim() === "")) return undefined;
        return { kind: "manual", id: id.trim(), name: name.trim(), unityVersion: unity.version.trim(),
            baseTemplate: { id: base.id, revision: base.revision },
            dependencies: dependencies.map(({ packageId, versionRange }) => ({ packageId: packageId.trim(), versionRange: versionRange.trim() })) };
    }

    const selectedSource = selection.kind === "save" ? saveSource() : undefined;
    function editRequest(): Request | undefined {
        if (detail === undefined || detail.template.locked || !name.trim()) return undefined;
        const chosen = dependencies.map(({ packageId, versionRange }) => ({ packageId: packageId.trim(), versionRange: versionRange.trim() }));
        const byId = new Map(chosen.map((entry) => [entry.packageId, entry.versionRange]));
        const original = new Map(detail.dependencies.map((entry) => [entry.packageId, entry.versionRange]));
        if (byId.size !== chosen.length || chosen.some(({ packageId, versionRange }) => !packageId || !versionRange)
            || addAttachments.some(({ sourceFile, targetPath }) => !sourceFile || !targetPath.trim().startsWith("Assets/") || targetPath.trim() === "Assets/")) return undefined;
        const setDependencies = chosen.filter(({ packageId, versionRange }) => original.get(packageId) !== versionRange);
        const removeDependencies = detail.dependencies.filter(({ packageId }) => !byId.has(packageId)).map(({ packageId }) => packageId);
        const nameChanged = name.trim() !== detail.template.name;
        if (!nameChanged && !setDependencies.length && !removeDependencies.length && !addAttachments.length && !removeAttachments.length) return undefined;
        return { kind: "edit", params: { id: detail.template.id, expectedRevision: detail.template.revision, confirm: true, changes: {
            ...(nameChanged ? { name: name.trim() } : {}), setDependencies, removeDependencies, removeAttachments,
            addAttachments: addAttachments.map(({ sourceFile, targetPath }) => ({ sourceFile, targetPath: targetPath.trim() }))
        } } };
    }

    const request: Request | undefined = selection.kind === "export" && destinationFile !== "" ? {
        kind: "export", params: { id: selection.template.id, revision: selection.template.revision, destinationFile }
    } : selection.kind === "remove" ? {
        kind: "remove", params: { id: selection.template.id, expectedRevision: selection.template.revision }
    } : selection.kind === "edit" ? editRequest() : selectedSource === undefined ? undefined : {
        kind: "save", params: { source: selectedSource, ...(record === undefined ? {} : { expectedRevision: record.revision }) }
    };

    function submit() {
        if (selection.kind === "view" || operationId !== undefined || choosing) return;
        void run(async () => {
            const submitted = retrying ? attempt.current : request === undefined ? undefined : { request, idempotencyKey: crypto.randomUUID() };
            if (submitted === undefined) return;
            attempt.current = submitted;
            let accepted;
            try {
                if (submitted.request.kind === "remove") {
                    await client.projectTemplatesRemove({ ...submitted.request.params, idempotencyKey: submitted.idempotencyKey });
                    attempt.current = undefined;
                    if (current()) { onChanged(); onClose(); }
                    return;
                }
                accepted = submitted.request.kind === "save"
                    ? await client.projectTemplatesSave({ ...submitted.request.params, idempotencyKey: submitted.idempotencyKey })
                    : submitted.request.kind === "edit"
                        ? await client.projectTemplateEdit({ ...submitted.request.params, idempotencyKey: submitted.idempotencyKey })
                        : await client.projectTemplatesExport({ ...submitted.request.params, idempotencyKey: submitted.idempotencyKey });
            } catch (caught: unknown) {
                if (!operationRequestOutcomeUnknown(normalizeRequestError(caught))) attempt.current = undefined;
                throw caught;
            }
            if (!current()) return;
            operations.accept(accepted.operationId, () => {
                if (current() && submitted.request.kind !== "export") onChanged();
            });
            attempt.current = undefined;
            setOperationId(accepted.operationId);
        });
    }

    const title = selection.kind === "view" ? "View template" : selection.kind === "edit" ? "Edit template" : selection.kind === "remove" ? "Remove template"
        : selection.kind === "export" ? "Export template" : record !== undefined ? "Replace template" : sourceLabels[source];
    const submitLabel = selection.kind === "edit" ? "Save template" : selection.kind === "remove" || selection.kind === "export" || record !== undefined ? title
        : source === "file" ? "Import template" : "Save template";
    const importing = selection.kind === "save" && record === undefined && source === "file" && operationId === undefined;
    const submitDisabled = busy || choosing || !retrying && request === undefined;
    const terminalFailure = operation?.state === "failed" || operation?.state === "cancelled";
    const failedPackage = operation?.state === "failed" && typeof operation.result === "object" && operation.result !== null
        && "packageId" in operation.result && typeof operation.result.packageId === "string" ? operation.result.packageId : undefined;

    const editable = selection.kind === "edit";
    const dependencyFields = <fieldset className="template-declarations">
        <legend>{t("Packages")}</legend>
        {dependencies.map((dependency) => <div className="template-declaration-row" key={dependency.key}>
            <TextField disabled={formDisabled} label={t("Package ID")} onInput={(packageId) => setDependencies((rows) => rows.map((row) =>
                row.key === dependency.key ? { ...row, packageId } : row))} readOnly={selection.kind === "view"} required value={dependency.packageId} />
            <TextField disabled={formDisabled} label={t("Version")} onInput={(versionRange) => setDependencies((rows) => rows.map((row) =>
                row.key === dependency.key ? { ...row, versionRange } : row))} readOnly={selection.kind === "view"} required value={dependency.versionRange} />
            {selection.kind === "view" ? null : <IconButton disabled={formDisabled} label={t("Remove package")} title={t("Remove package")}
                onClick={() => setDependencies((rows) => rows.filter(({ key }) => key !== dependency.key))} type="button"><Icon asset={closeIcon} /></IconButton>}
        </div>)}
        {selection.kind === "view" ? null : <Button disabled={formDisabled} onClick={() => setDependencies((rows) => [...rows, { key: crypto.randomUUID(), packageId: "", versionRange: "" }])}
            type="button" variant="text">{t("Add package")}</Button>}
    </fieldset>;

    return <>
        <Dialog dismissible={!busy && !choosing} onClose={onClose} open={open} returnFocus={origin} title={t(title)} wide={selection.kind !== "save" || source !== "file"}
            actions={importing ? <>
                <DialogCloseButton disabled={busy || choosing} variant="text">{t("Cancel")}</DialogCloseButton>
                <Button disabled={submitDisabled} onClick={submit} type="button">{t("Confirm")}</Button>
            </> : undefined}>
            {!open ? null : operationId !== undefined ? <div className="project-create-flow">
                <AcceptedOperationProgress operationId={operationId} operations={operations} />
                {failedPackage === undefined ? null : <p>{t("Package ID")} <code>{failedPackage}</code></p>}
                {selection.kind === "export" ? <PathField label={t("Destination file")} readOnly value={destinationFile} /> : null}
                <div className="dialog-actions">
                    {terminalFailure ? <Button onClick={() => setOperationId(undefined)} type="button" variant="text">{t("Back")}</Button> : null}
                    <DialogCloseButton>{t(operation !== undefined && ["succeeded", "failed", "cancelled"].includes(operation.state) ? "Close" : "Minimize")}</DialogCloseButton>
                </div>
            </div> : <form className="project-create-flow" onSubmit={(event) => { event.preventDefault(); submit(); }}>
                {selection.kind === "view" || editable ? <>
                    <TextField label={t("Template ID")} readOnly value={id} />
                    <TextField label={t("Display name")} disabled={editable && (formDisabled || detail === undefined)} readOnly={!editable} onInput={setName} value={name} />
                    {record?.editorVersion === undefined ? null : <TextField label={t("Unity version")} readOnly value={detail?.template.editorVersion ?? record.editorVersion} />}
                    {detailId === undefined ? null : detailError !== undefined ? <><TemplateFailure error={detailError} /><Button onClick={() => setDetailRetry((value) => value + 1)}
                        type="button" variant="text">{t("Retry")}</Button></> : detail === undefined ? <p role="status">{t("Loading…")}</p> : <>
                        {dependencyFields}
                        <fieldset className="template-declarations">
                            <legend>{t("Attachments")}</legend>
                            <PagedItems initialItems={detail.attachments} initialCursor={detail.nextCursor ?? undefined} loadMore={async (cursor) => {
                                const next = await client.projectTemplateGet({ id: detail.template.id, cursor });
                                return { items: next.attachments, nextCursor: next.nextCursor ?? undefined };
                            }}>
                                {(attachments, pagination) => <>
                                    {attachments.length === 0 ? <p>{t("No attachments")}</p> : <ul>
                                        {attachments.map((path) => <li key={path}>
                                            {editable ? <Checkbox checked={removeAttachments.includes(path)} disabled={formDisabled} label={t("Remove {name}", { name: path })}
                                                onChange={(checked) => setRemoveAttachments((paths) => checked ? [...paths, path] : paths.filter((entry) => entry !== path))} />
                                                : <code className="template-id">{path}</code>}
                                        </li>)}
                                    </ul>}
                                    {pagination.error === undefined ? null : <TemplateFailure error={pagination.error} />}
                                </>}
                            </PagedItems>
                            {editable ? <>
                                {addAttachments.map((attachment) => <div className="template-declaration-row" key={attachment.key}>
                                    <PathField label={t("Source file")} readOnly value={attachment.sourceFile} />
                                    <TextField label={t("Path in template")} disabled={formDisabled} onInput={(targetPath) => setAddAttachments((rows) => rows.map((row) =>
                                        row.key === attachment.key ? { ...row, targetPath } : row))} required value={attachment.targetPath} />
                                    <IconButton label={t("Remove attachment")} disabled={formDisabled} onClick={() =>
                                        setAddAttachments((rows) => rows.filter((row) => row.key !== attachment.key))} type="button"><Icon asset={closeIcon} /></IconButton>
                                </div>)}
                                <Button disabled={formDisabled} onClick={() => void run(async () => {
                                    const sourceFile = await client.selectFile("templateAttachment");
                                    if (sourceFile === undefined || !current()) return;
                                    const fileName = sourceFile.split(/[\\/]/).pop();
                                    if (fileName) setAddAttachments((rows) => [...rows, { key: crypto.randomUUID(), sourceFile, targetPath: "Assets/" + fileName }]);
                                })} type="button" variant="text">{t("Add attachment")}</Button>
                            </> : null}
                        </fieldset>
                    </>}
                </> : selection.kind === "remove" ? <p>{t("Remove {name} from your templates?", { name })} <code className="template-id">{id}</code></p>
                    : selection.kind === "export" ? <>
                        <TextField disabled={formDisabled} label={t("File name")} onInput={(value) => { setFileName(value); setDestinationFile(""); }} required value={fileName} />
                        <div className="project-target-picker">
                            <PathField label={t("Destination file")} readOnly value={destinationFile} />
                            <Button disabled={formDisabled || fileName.trim() === ""} onClick={() => void selectDestination()} type="button" variant="tonal">{t("Choose directory")}</Button>
                        </div>
                    </> : <>
                        {record === undefined ? null : <>
                            <p>{t("Replace the contents of {name}?", { name })} <code className="template-id">{id}</code></p>
                            <Select disabled={formDisabled} label={t("Template source")} onChange={(value) => setSource(value as SourceKind)}
                                options={Object.entries(sourceLabels).map(([value, label]) => ({ value, label: t(label) }))} value={source} />
                        </>}
                        {importing && initialPath !== undefined ? <PathField label={t("Template file")} value={sourceFile} readOnly />
                            : importing ? <FilePicker client={client} kind="file" disabled={formDisabled} label={t("Template file")} value={sourceFile}
                            choosePath={() => client.selectProjectTemplateFile("open", undefined, sourceFile)} onChoosingChange={setChoosing}
                            onPathChange={(path) => { setSourceFile(path); setError(undefined); }} />
                            : source === "file" ? <div className="project-target-picker">
                            <PathField disabled={formDisabled} label={t("Template file")} value={sourceFile} onInput={(path) => { setSourceFile(path); setError(undefined); }} />
                            <Button disabled={formDisabled} onClick={() => void selectSource()} type="button" variant="tonal">{t("Select")}</Button>
                        </div> : <>
                            <div className="template-declaration-row">
                                <TextField disabled={formDisabled} label={t("Template ID")} maxLength={128} onInput={setId}
                                    placeholder={t("For example, com.example.my-template")} readOnly={record !== undefined} required value={id} />
                                <TextField disabled={formDisabled} label={t("Display name")} onInput={setName} required value={name} />
                            </div>
                            {source === "project" ? <>
                                <TemplateProjectPicker client={client} disabled={formDisabled} onSelect={setProject} value={project?.projectId ?? ""} />
                                <Checkbox checked={includeAssets} disabled={formDisabled} label={t("Include project assets")} onChange={setIncludeAssets} />
                                <Checkbox checked={lockPackageVersions} disabled={formDisabled} label={t("Lock package versions")} onChange={setLockPackageVersions} />
                            </> : <>
                                <TemplateBasePicker client={client} catalog={catalog} disabled={formDisabled} onSelect={setBase} onValid={setBaseValid} value={base} />
                                <CreateUnityVersionField disabled={formDisabled} selection={unity} />
                                {dependencyFields}
                            </>}
                        </>}
                    </>}
                {error === undefined ? null : <TemplateFailure error={error} />}
                {importing ? null : <div className="dialog-actions">
                    <DialogCloseButton disabled={busy || choosing} variant="text">{t(selection.kind === "view" ? "Close" : "Cancel")}</DialogCloseButton>
                    {selection.kind === "view" ? null : <Button disabled={submitDisabled}
                        type="submit">{t(retrying ? "Retry" : submitLabel)}</Button>}
                </div>}
            </form>}
        </Dialog>
        {operations.follow()}
    </>;
}

function TemplateFailure({ error }: { error: RpcError }) {
    const { t } = useTranslation();
    return <div role="alert" data-notification-error={error.code}>
        <p>{t(requestErrorMessage(error.code))}</p><code>{error.code}</code>
        {error.diagnosticId === undefined ? null : <p><code>{error.diagnosticId}</code></p>}
    </div>;
}

function TemplateProjectPicker({ client, disabled, onSelect, value }: {
    client: GuiRpcClient;
    disabled: boolean;
    onSelect(project: SelectedProject | undefined): void;
    value: string;
}) {
    const { t } = useTranslation();
    const [page, setPage] = useState<ProjectsListResult>();
    const [error, setError] = useState<RpcError>();
    const [retry, setRetry] = useState(0);
    useEffect(() => {
        let active = true;
        setPage(undefined);
        setError(undefined);
        void client.projectsList().then((result) => { if (active) setPage(result); })
            .catch((caught: unknown) => { if (active) setError(normalizeRequestError(caught)); });
        return () => { active = false; };
    }, [client, retry]);

    if (error !== undefined) return <><TemplateFailure error={error} /><Button disabled={disabled} onClick={() => setRetry((previous) => previous + 1)}
        type="button" variant="text">{t("Retry")}</Button></>;
    if (page === undefined) return <p role="status">{t("Loading…")}</p>;
    return <PagedItems initialItems={page.projects} initialCursor={page.nextCursor} loadMore={async (cursor) => {
        const next = await client.projectsList(cursor);
        return { items: next.projects, nextCursor: next.nextCursor };
    }}>
        {(projects) => {
            const registered = projects.filter((item): item is SelectedProject => item.projectId !== undefined && item.revision !== undefined);
            return <Select disabled={disabled || registered.length === 0} label={t("Source project")} value={value}
                options={registered.map((item) => ({ value: item.projectId, label: displayPath(item.rootPath) }))}
                onChange={(id) => onSelect(registered.find((item) => item.projectId === id))} />;
        }}
    </PagedItems>;
}

function TemplateBasePicker({ client, catalog, disabled, onSelect, onValid, value }: {
    client: GuiRpcClient;
    catalog: ProjectTemplatesListResult;
    disabled: boolean;
    onSelect(template: ProjectTemplateRecord): void;
    onValid(valid: boolean): void;
    value: ProjectTemplateRecord | undefined;
}) {
    const { t } = useTranslation();
    return <PagedItems initialItems={catalog.templates} initialCursor={catalog.nextCursor} showPagination={false} loadMore={async (cursor) => {
        const next = await client.projectTemplatesList(cursor);
        return { items: next.templates, nextCursor: next.nextCursor };
    }}>
        {(templates, pagination) => <>
            <TemplateBaseOptions disabled={disabled} onSelect={onSelect} onValid={onValid} templates={templates} value={value} />
            {pagination.error === undefined ? null : <p role="alert">{t("Could not load more items. Your loaded items are still available.")}</p>}
            {pagination.cursor === undefined ? null : <Button disabled={disabled || pagination.loading} onClick={() => void pagination.next()} type="button" variant="text">{t(pagination.error === undefined ? "Load more templates" : "Retry loading more")}</Button>}
        </>}
    </PagedItems>;
}

function TemplateBaseOptions({ disabled, onSelect, onValid, templates, value }: {
    disabled: boolean;
    onSelect(template: ProjectTemplateRecord): void;
    onValid(valid: boolean): void;
    templates: ProjectTemplateRecord[];
    value: ProjectTemplateRecord | undefined;
}) {
    const { t } = useTranslation();
    const selected = templates.find((template) => template.id === value?.id);
    const valid = value !== undefined && selected?.revision === value.revision;
    useEffect(() => {
        onValid(valid);
        if (value === undefined) {
            const blank = templates.find(({ id }) => id === BLANK_PROJECT_TEMPLATE_ID);
            if (blank !== undefined) onSelect(blank);
        }
    }, [onSelect, onValid, templates, valid, value]);
    const names = new Map<string, number>();
    for (const template of templates) names.set(template.name, (names.get(template.name) ?? 0) + 1);
    return <>
        <div className="project-create-unity-version"><span>{t("Base template")}</span>
        <Select className="alcomd-select--constrained" disabled={disabled || templates.length === 0} label={t("Base template")} value={valid ? value.id : ""}
            onChange={(id) => { const next = templates.find((template) => template.id === id); if (next) onSelect(next); }}
            options={templates.map((template) => {
                const name = template.id === BLANK_PROJECT_TEMPLATE_ID ? t("Blank") : template.name;
                const label = fixedVersionTemplateLabel(name, template.editorVersion);
                return { value: template.id, label: names.get(template.name) === 1 ? label : `${label} (${template.id})` };
            })} /></div>
        {value !== undefined && !valid ? <p role="alert">{t("The selected template changed. Select it again.")}</p> : null}
    </>;
}
