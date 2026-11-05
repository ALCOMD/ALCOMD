import type { ProjectTemplateConstraint, ProjectTemplateDeclaration, ProjectTemplateDependency, ProjectTemplateRecord, RpcError } from "@alcomd/sdk";
import { moreVertIcon } from "@alcomd/ui/icons";
import { useCallback, useEffect, useRef, useState, type FormEvent, type ReactNode } from "react";

import { useActionScope } from "./CoreActions";
import { DesktopList } from "./DesktopList";
import { Button, Dialog, Icon, IconButton, Menu, MenuItem, TextField, type DialogOrigin } from "./Material";
import { PagedItems } from "./PagedItems";
import { UtilityWorkspace } from "./UtilityWorkspace";
import { useTranslation } from "./i18n";
import { navigationLabels } from "./navigation-labels";
import { normalizeRequestError, requestErrorMessage } from "./request-errors";
import type { GuiRpcClient } from "./rpc";

type Selection = { kind: "create" } | { kind: "view" | "edit" | "remove"; template: ProjectTemplateRecord };
type Row<T> = T & { key: string };

function rows<T extends object>(values: T[]): Row<T>[] {
    return values.map((value) => ({ ...value, key: crypto.randomUUID() }));
}

function splitVersions(value: string): string[] {
    return value.split(",").map((item) => item.trim()).filter(Boolean);
}

export function ProjectTemplatesWorkspace({ client, navigation }: { client: GuiRpcClient; navigation: ReactNode }) {
    const { t } = useTranslation();
    const [selection, setSelection] = useState<(Selection & { origin: DialogOrigin; key: number; closed: boolean })>();
    const session = useRef(0);
    const createButton = useRef<HTMLElement>(null);
    const load = useCallback(() => client.projectTemplatesList(), [client]);

    useEffect(() => { setSelection(undefined); }, [client]);

    function select(next: Selection, origin: HTMLElement | null) {
        setSelection({ ...next, key: ++session.current, closed: false, origin: { get current() { return origin?.isConnected ? origin : createButton.current; } } });
    }

    return <UtilityWorkspace changes={["templates"]} title={t(navigationLabels.resources)} navigation={navigation} load={load}
        tools={() => <Button ref={createButton} type="button" onClick={() => select({ kind: "create" }, createButton.current)}>{t("Create template")}</Button>}>
        {(value, refresh) => <>
            <PagedItems initialItems={value.templates} initialCursor={value.nextCursor} loadMore={async (cursor) => {
                const next = await client.projectTemplatesList(cursor);
                return { items: next.templates, nextCursor: next.nextCursor };
            }}>
                {(templates) => templates.length === 0 ? <p>{t("No templates available")}</p> : <DesktopList
                    label={t("Templates")} minWidth={640}
                    columns={[
                        { key: "name", header: t("Template") },
                        { key: "id", header: t("Template ID") },
                        { key: "actions", header: t("Actions") }
                    ]}
                    rows={templates.map((template) => {
                        const { id, name } = template.declaration;
                        return { key: id, cells: [
                            <><strong>{name}</strong>{template.locked ? <small>{t("Built-in")}</small> : null}</>,
                            <span className="template-id">{id}</span>,
                            <TemplateRowActions onSelect={select} template={template} />
                        ] };
                    })} />}
            </PagedItems>
            {selection === undefined ? null : <TemplateEditor
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
    function select(kind: "view" | "edit" | "remove") {
        setOpen(false);
        onSelect({ kind, template }, anchor.current);
    }
    return <div className="card-actions">
        <IconButton aria-expanded={open} label={t("More actions for {name}", { name: template.declaration.name })}
            onClick={() => setOpen(true)} ref={anchor} type="button"><Icon asset={moreVertIcon} size={24} /></IconButton>
        <Menu anchorRef={anchor} onClose={() => setOpen(false)} open={open}>
            <MenuItem label={t(template.locked ? "View template" : "Edit template")} onClick={() => select(template.locked ? "view" : "edit")} />
            <MenuItem disabled={template.locked} label={t("Remove template")} onClick={() => select("remove")} />
        </Menu>
    </div>;
}

function TemplateEditor({ client, onChanged, onClose, open, origin, selection }: {
    client: GuiRpcClient;
    onChanged(): void;
    onClose(): void;
    open: boolean;
    origin: DialogOrigin;
    selection: Selection;
}) {
    const { t } = useTranslation();
    const record = selection.kind === "create" ? undefined : selection.template;
    const view = selection.kind === "view";
    const removing = selection.kind === "remove";
    const [id, setId] = useState(record?.declaration.id ?? "");
    const [name, setName] = useState(record?.declaration.name ?? "");
    const [series, setSeries] = useState(record?.declaration.unity.series.join(", ") ?? "");
    const [presets, setPresets] = useState(record?.declaration.unity.presets.join(", ") ?? "");
    const [dependencies, setDependencies] = useState(() => rows(record?.declaration.dependencies ?? []));
    const [constraints, setConstraints] = useState(() => rows(record?.declaration.constraints ?? []));
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState<RpcError>();
    const pending = useRef(false);
    const current = useActionScope(client, open);

    async function submit(event: FormEvent) {
        event.preventDefault();
        if (view || pending.current) return;
        pending.current = true;
        setBusy(true);
        setError(undefined);
        try {
            if (removing && record !== undefined) {
                await client.projectTemplatesRemove({ id: record.declaration.id, expectedRevision: record.revision });
            } else {
                const declaration: ProjectTemplateDeclaration = {
                    id: id.trim(),
                    name: name.trim(),
                    unity: { series: splitVersions(series), presets: splitVersions(presets) },
                    dependencies: dependencies.map(({ packageId, versionRange }) => ({ packageId: packageId.trim(), versionRange: versionRange.trim() })),
                    constraints: constraints.map(({ unityVersion, packageId, versionRange }) => ({
                        unityVersion: unityVersion.trim(), packageId: packageId.trim(), versionRange: versionRange.trim()
                    }))
                };
                await client.projectTemplatesSave({ declaration, ...(record === undefined ? {} : { expectedRevision: record.revision }) });
            }
            if (current()) {
                onChanged();
                onClose();
            }
        } catch (caught: unknown) {
            if (current()) setError(normalizeRequestError(caught));
        } finally {
            pending.current = false;
            if (current()) setBusy(false);
        }
    }

    const title = selection.kind === "create" ? "Create template"
        : view ? "View template" : removing ? "Remove template" : "Edit template";
    const valid = id.trim().length > 0 && name.trim().length > 0
        && dependencies.every(({ packageId, versionRange }) => packageId.trim().length > 0 && versionRange.trim().length > 0)
        && constraints.every(({ unityVersion, packageId, versionRange }) => unityVersion.trim().length > 0 && packageId.trim().length > 0 && versionRange.trim().length > 0);

    return <Dialog dismissible={!busy} onClose={onClose} open={open} returnFocus={origin} title={t(title)} wide>
        {open ? <form className="project-create-flow" onSubmit={(event) => void submit(event)}>
            {removing ? <p>{t("Remove {name} from your templates?", { name })} <code className="template-id">{id}</code></p> : <>
                <TextField disabled={busy} label={t("Template ID")} maxLength={128} onInput={setId} readOnly={record !== undefined} required value={id}
                    supportingText={record === undefined ? t("For example, com.example.my-template") : undefined} />
                <TextField disabled={busy} label={t("Name")} onInput={setName} readOnly={view} required value={name} />
                <TextField disabled={busy} label={t("Unity version series")} onInput={setSeries} placeholder="2022.3, 6000.0" readOnly={view} value={series} />
                <TextField disabled={busy} label={t("Unity version presets")} onInput={setPresets} readOnly={view} value={presets}
                    supportingText={view ? undefined : t("For example, 2022.3.22f1, 2022.3.6f1")}/>
                <fieldset className="template-declarations">
                    <legend>{t("VPM dependencies")}</legend>
                    {dependencies.map((dependency) => <div className="template-declaration-row" key={dependency.key}>
                        <DependencyFields busy={busy} dependency={dependency} readOnly={view} onInput={(patch) => {
                            setDependencies((current) => current.map((row) => row.key === dependency.key ? { ...row, ...patch } : row));
                        }} />
                        {view ? null : <Button disabled={busy} onClick={() => setDependencies((current) => current.filter(({ key }) => key !== dependency.key))} type="button" variant="text">{t("Remove")}</Button>}
                    </div>)}
                    {view ? null : <Button disabled={busy} onClick={() => setDependencies((current) => [...current, ...rows([{ packageId: "", versionRange: "" }])])} type="button" variant="text">{t("Add dependency")}</Button>}
                </fieldset>
                <fieldset className="template-declarations">
                    <legend>{t("Package constraints by Unity version")}</legend>
                    {constraints.map((constraint) => <div className="template-declaration-row" key={constraint.key}>
                        <TextField disabled={busy} label={t("Unity version")} onInput={(unityVersion) => {
                            setConstraints((current) => current.map((row) => row.key === constraint.key ? { ...row, unityVersion } : row));
                        }} readOnly={view} required value={constraint.unityVersion} />
                        <DependencyFields busy={busy} dependency={constraint} readOnly={view} onInput={(patch) => {
                            setConstraints((current) => current.map((row) => row.key === constraint.key ? { ...row, ...patch } : row));
                        }} />
                        {view ? null : <Button disabled={busy} onClick={() => setConstraints((current) => current.filter(({ key }) => key !== constraint.key))} type="button" variant="text">{t("Remove")}</Button>}
                    </div>)}
                    {view ? null : <Button disabled={busy} onClick={() => setConstraints((current) => [...current, ...rows<ProjectTemplateConstraint>([{ unityVersion: "", packageId: "", versionRange: "" }])])} type="button" variant="text">{t("Add constraint")}</Button>}
                </fieldset>
            </>}
            {error === undefined ? null : <div role="alert"><p>{t(requestErrorMessage(error.code))}</p><code>{error.code}</code></div>}
            <div className="dialog-actions">
                <Button disabled={busy} onClick={onClose} type="button" variant="text">{t(view ? "Close" : "Cancel")}</Button>
                {view ? null : <Button disabled={busy || !removing && !valid} type="submit">{t(removing ? "Remove template" : "Save template")}</Button>}
            </div>
        </form> : null}
    </Dialog>;
}

function DependencyFields({ busy, dependency, onInput, readOnly }: {
    busy: boolean;
    dependency: ProjectTemplateDependency;
    onInput(patch: Partial<ProjectTemplateDependency>): void;
    readOnly: boolean;
}) {
    const { t } = useTranslation();
    return <>
        <TextField disabled={busy} label={t("Package ID")} onInput={(packageId) => onInput({ packageId })} readOnly={readOnly} required value={dependency.packageId} />
        <TextField disabled={busy} label={t("Version range")} onInput={(versionRange) => onInput({ versionRange })} readOnly={readOnly} required value={dependency.versionRange} />
    </>;
}
