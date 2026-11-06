import type { ProjectTemplateDependency, ProjectTemplateRecord, ProjectTemplateSource, ProjectTemplatesExportParams, ProjectTemplatesSaveParams, RpcError } from "@alcomd/sdk";
import { closeIcon, moreVertIcon } from "@alcomd/ui/icons";
import { useCallback, useEffect, useRef, useState, type FormEvent, type ReactNode } from "react";

import { AcceptedOperationProgress, operationRequestOutcomeUnknown, useAcceptedOperations, useActionScope } from "./CoreActions";
import { DesktopList } from "./DesktopList";
import { Button, Checkbox, Dialog, Icon, IconButton, Menu, MenuItem, PathField, Select, TextField, type DialogOrigin } from "./Material";
import { FilePicker } from "./FilePicker";
import { PagedItems } from "./PagedItems";
import { SplitButton } from "./SplitButton";
import { UtilityWorkspace } from "./UtilityWorkspace";
import type { ProjectSnapshot, ProjectsListResult } from "./core-models";
import { displayPath } from "./display-path";
import { useTranslation } from "./i18n";
import { navigationLabels } from "./navigation-labels";
import { normalizeRequestError, requestErrorMessage } from "./request-errors";
import type { GuiRpcClient } from "./rpc";
import { CreateUnityVersionField, useCreateUnityVersion } from "./use-create-unity-version";

type SourceKind = ProjectTemplateSource["kind"];
type Selection = { kind: "save"; source: SourceKind; template?: ProjectTemplateRecord }
    | { kind: "view" | "export" | "remove"; template: ProjectTemplateRecord };
type Request =
    | { kind: "save"; params: Omit<ProjectTemplatesSaveParams, "idempotencyKey"> }
    | { kind: "export"; params: Omit<ProjectTemplatesExportParams, "idempotencyKey"> };
type SelectedProject = ProjectSnapshot & { projectId: string; revision: number };

const sourceLabels = {
    manual: "Create template",
    file: "Import template",
    project: "Create template from project"
};

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

    return <UtilityWorkspace changes={["templates"]} title={t(navigationLabels.resources)} navigation={navigation} load={load}
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
                        <strong>{template.name}</strong>,
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
            />}
        </>}
    </UtilityWorkspace>;
}

function TemplateRowActions({ onSelect, template }: { onSelect(selection: Selection, origin: HTMLElement | null): void; template: ProjectTemplateRecord }) {
    const { t } = useTranslation();
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
            <MenuItem label={t("Export template")} onClick={() => select({ kind: "export", template })} />
            <MenuItem disabled={template.locked} label={t("Replace template")} onClick={() => select({ kind: "save", source: "file", template })} />
            <MenuItem disabled={template.locked} label={t("Remove template")} onClick={() => select({ kind: "remove", template })} />
        </Menu>
    </div>;
}

function TemplateDialog({ client, onChanged, onClose, open, origin, selection }: {
    client: GuiRpcClient;
    onChanged(): void;
    onClose(): void;
    open: boolean;
    origin: DialogOrigin;
    selection: Selection;
}) {
    const { t } = useTranslation();
    const record = selection.template;
    const [source, setSource] = useState<SourceKind>(selection.kind === "save" ? selection.source : "manual");
    const [id, setId] = useState(record?.id ?? "");
    const [name, setName] = useState(record?.name ?? "");
    const [dependencies, setDependencies] = useState<(ProjectTemplateDependency & { key: string })[]>([]);
    const [project, setProject] = useState<SelectedProject>();
    const [includeAssets, setIncludeAssets] = useState(false);
    const [lockPackageVersions, setLockPackageVersions] = useState(false);
    const [sourceFile, setSourceFile] = useState("");
    const [destinationFile, setDestinationFile] = useState("");
    const [fileName, setFileName] = useState(record === undefined ? "" : record.id + ".alcomdtemplate");
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState<RpcError>();
    const [operationId, setOperationId] = useState<string>();
    const pending = useRef(false);
    const attempt = useRef<{ request: Request; idempotencyKey: string } | undefined>(undefined);
    const current = useActionScope(client, open);
    const operations = useAcceptedOperations(client);
    const operation = operationId === undefined ? undefined : operations.snapshot(operationId)?.operation;
    const unity = useCreateUnityVersion(client, undefined, open && selection.kind === "save" && source === "manual");

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
        const path = await client.selectProjectTemplateFile("save", fileName.trim());
        if (!current() || path === undefined) return;
        setDestinationFile(path);
    });

    function saveSource(): ProjectTemplateSource | undefined {
        if (source === "file") return sourceFile === "" ? undefined : { kind: "file", sourceFile };
        if (id.trim() === "" || name.trim() === "") return undefined;
        if (source === "project") return project === undefined ? undefined : {
            kind: "project", projectId: project.projectId, expectedProjectRevision: project.revision,
            id: id.trim(), name: name.trim(), includeAssets, lockPackageVersions
        };
        if (unity.version.trim() === "" || dependencies.some(({ packageId, versionRange }) => packageId.trim() === "" || versionRange.trim() === "")) return undefined;
        return { kind: "manual", id: id.trim(), name: name.trim(), unityVersion: unity.version.trim(),
            dependencies: dependencies.map(({ packageId, versionRange }) => ({ packageId: packageId.trim(), versionRange: versionRange.trim() })) };
    }

    const selectedSource = selection.kind === "save" ? saveSource() : undefined;
    const request: Request | undefined = selection.kind === "export" && destinationFile !== "" ? {
        kind: "export", params: { id: selection.template.id, revision: selection.template.revision, destinationFile }
    } : selectedSource === undefined ? undefined : {
        kind: "save", params: { source: selectedSource, ...(record === undefined ? {} : { expectedRevision: record.revision }) }
    };

    function submit(event: FormEvent) {
        event.preventDefault();
        if (selection.kind === "view" || operationId !== undefined) return;
        void run(async () => {
            if (selection.kind === "remove") {
                await client.projectTemplatesRemove({ id: selection.template.id, expectedRevision: selection.template.revision });
                if (current()) { onChanged(); onClose(); }
                return;
            }
            const submitted = retrying ? attempt.current : request === undefined ? undefined : { request, idempotencyKey: crypto.randomUUID() };
            if (submitted === undefined) return;
            attempt.current = submitted;
            let accepted;
            try {
                accepted = submitted.request.kind === "save"
                    ? await client.projectTemplatesSave({ ...submitted.request.params, idempotencyKey: submitted.idempotencyKey })
                    : await client.projectTemplatesExport({ ...submitted.request.params, idempotencyKey: submitted.idempotencyKey });
            } catch (caught: unknown) {
                if (!operationRequestOutcomeUnknown(normalizeRequestError(caught))) attempt.current = undefined;
                throw caught;
            }
            if (!current()) return;
            operations.accept(accepted.operationId, () => {
                if (current() && submitted.request.kind === "save") { onChanged(); onClose(); }
            });
            attempt.current = undefined;
            setOperationId(accepted.operationId);
        });
    }

    const title = selection.kind === "view" ? "View template" : selection.kind === "remove" ? "Remove template"
        : selection.kind === "export" ? "Export template" : record !== undefined ? "Replace template" : sourceLabels[source];
    const submitLabel = selection.kind === "remove" || selection.kind === "export" || record !== undefined ? title
        : source === "file" ? "Import template" : "Save template";
    const terminalFailure = operation?.state === "failed" || operation?.state === "cancelled";
    const failedPackage = operation?.state === "failed" && typeof operation.result === "object" && operation.result !== null
        && "packageId" in operation.result && typeof operation.result.packageId === "string" ? operation.result.packageId : undefined;

    return <>
        <Dialog dismissible={!busy} onClose={onClose} open={open} returnFocus={origin} title={t(title)} wide>
            {!open ? null : operationId !== undefined ? <div className="project-create-flow">
                <AcceptedOperationProgress operationId={operationId} operations={operations} />
                {failedPackage === undefined ? null : <p>{t("Package ID")} <code>{failedPackage}</code></p>}
                {selection.kind === "export" ? <PathField label={t("Destination file")} readOnly value={destinationFile} /> : null}
                <div className="dialog-actions">
                    {terminalFailure ? <Button onClick={() => setOperationId(undefined)} type="button" variant="text">{t("Back")}</Button> : null}
                    <Button onClick={onClose} type="button">{t("Close")}</Button>
                </div>
            </div> : <form className="project-create-flow" onSubmit={submit}>
                {selection.kind === "view" ? <>
                    <TextField label={t("Template ID")} readOnly value={selection.template.id} />
                    <TextField label={t("Display name")} readOnly value={selection.template.name} />
                    {selection.template.editorVersion === undefined ? null : <TextField label={t("Unity version")} readOnly value={selection.template.editorVersion} />}
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
                        {source === "file" ? <FilePicker client={client} disabled={formDisabled} label={t("Template file")} value={sourceFile}
                            chooseFile={() => client.selectProjectTemplateFile("open")} onInput={(path) => { setSourceFile(path); setError(undefined); }} /> : <>
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
                                <CreateUnityVersionField disabled={formDisabled} selection={unity} />
                                <fieldset className="template-declarations">
                                    <legend>{t("Packages")}</legend>
                                    {dependencies.map((dependency) => <div className="template-declaration-row" key={dependency.key}>
                                        <TextField disabled={formDisabled} label={t("Package ID")} onInput={(packageId) => setDependencies((rows) => rows.map((row) =>
                                            row.key === dependency.key ? { ...row, packageId } : row))} required value={dependency.packageId} />
                                        <TextField disabled={formDisabled} label={t("Version")} onInput={(versionRange) => setDependencies((rows) => rows.map((row) =>
                                            row.key === dependency.key ? { ...row, versionRange } : row))} required value={dependency.versionRange} />
                                        <IconButton disabled={formDisabled} label={t("Remove package")} title={t("Remove package")}
                                            onClick={() => setDependencies((rows) => rows.filter(({ key }) => key !== dependency.key))} type="button"><Icon asset={closeIcon} /></IconButton>
                                    </div>)}
                                    <Button disabled={formDisabled} onClick={() => setDependencies((rows) => [...rows, { key: crypto.randomUUID(), packageId: "", versionRange: "" }])}
                                        type="button" variant="text">{t("Add package")}</Button>
                                </fieldset>
                            </>}
                        </>}
                    </>}
                {error === undefined ? null : <TemplateFailure error={error} />}
                <div className="dialog-actions">
                    <Button disabled={busy} onClick={onClose} type="button" variant="text">{t(selection.kind === "view" ? "Close" : "Cancel")}</Button>
                    {selection.kind === "view" ? null : <Button disabled={busy || !retrying && request === undefined && selection.kind !== "remove"}
                        type="submit">{t(retrying ? "Retry" : submitLabel)}</Button>}
                </div>
            </form>}
        </Dialog>
        {operations.follow()}
    </>;
}

function TemplateFailure({ error }: { error: RpcError }) {
    const { t } = useTranslation();
    return <div role="alert">
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
