import type {
    UiAction,
    UiDocument,
    UiFieldValue,
    UiNode,
    UiSnapshot,
    UiSubmittedField
} from "@alcomd/sdk";

export interface UiTreeNode {
    node: UiNode;
    children: UiTreeNode[];
    formId?: string;
}

export interface FormDraft {
    sessionId: string;
    snapshotRevision: number;
    formNodeId: string;
    values: Record<string, UiFieldValue | undefined>;
    dirty: boolean;
}

export class PortableUiConsumerError extends Error {
    readonly code: "extension_ui_protocol_unsupported" | "extension_ui_document_invalid" | "extension_ui_snapshot_stale";

    constructor(code: PortableUiConsumerError["code"]) {
        super(code);
        this.code = code;
    }
}

export function buildTree(document: UiDocument): UiTreeNode {
    if (document.protocol !== "portable-v1" || document.nodes.length === 0) {
        throw new PortableUiConsumerError(
            document.protocol === "portable-v1"
                ? "extension_ui_document_invalid"
                : "extension_ui_protocol_unsupported"
        );
    }
    const built = new Map<string, UiTreeNode>();
    let root: UiTreeNode | undefined;
    for (const node of document.nodes) {
        assertKnownNode(node);
        const parent = node.parentId === undefined ? undefined : built.get(node.parentId);
        if (node.parentId !== undefined && parent === undefined) {
            throw new PortableUiConsumerError("extension_ui_document_invalid");
        }
        const formId = node.kind === "form" ? node.nodeId : parent?.formId;
        const current: UiTreeNode = {
            node,
            children: [],
            ...(formId === undefined ? {} : { formId })
        };
        if (parent === undefined) {
            if (root !== undefined || node.kind !== "page") {
                throw new PortableUiConsumerError("extension_ui_document_invalid");
            }
            root = current;
        } else {
            parent.children.push(current);
        }
        built.set(node.nodeId, current);
    }
    if (root === undefined || built.size !== document.nodes.length) {
        throw new PortableUiConsumerError("extension_ui_document_invalid");
    }
    return root;
}

export function acceptSnapshot(
    current: UiSnapshot | undefined,
    next: UiSnapshot,
    replayed: boolean
): UiSnapshot {
    buildTree(next.document);
    if (current === undefined) {
        if (next.snapshotRevision !== 1) {
            throw new PortableUiConsumerError("extension_ui_snapshot_stale");
        }
        return next;
    }
    if (next.sessionId !== current.sessionId) {
        throw new PortableUiConsumerError("extension_ui_snapshot_stale");
    }
    if (next.snapshotRevision > current.snapshotRevision) {
        return next;
    }
    if (replayed && next.snapshotRevision === current.snapshotRevision) {
        return next;
    }
    throw new PortableUiConsumerError("extension_ui_snapshot_stale");
}

export function createFormDrafts(snapshot: UiSnapshot): Record<string, FormDraft> {
    const drafts: Record<string, FormDraft> = {};
    const tree = buildTree(snapshot.document);
    walk(tree, (entry) => {
        if (entry.node.kind === "form") {
            drafts[entry.node.nodeId] = {
                sessionId: snapshot.sessionId,
                snapshotRevision: snapshot.snapshotRevision,
                formNodeId: entry.node.nodeId,
                values: {},
                dirty: false
            };
        }
        if (isEditableField(entry.node) && entry.formId !== undefined) {
            const draft = drafts[entry.formId];
            if (draft === undefined) {
                throw new PortableUiConsumerError("extension_ui_document_invalid");
            }
            draft.values[fieldId(entry.node)] = initialEditableValue(entry.node);
        }
    });
    return drafts;
}

export function updateDraft(
    draft: FormDraft,
    field: string,
    value: UiFieldValue | undefined
): FormDraft {
    if (!(field in draft.values)) {
        throw new PortableUiConsumerError("extension_ui_document_invalid");
    }
    return {
        ...draft,
        values: { ...draft.values, [field]: value },
        dirty: true
    };
}

export function submitFormAction(
    document: UiDocument,
    formId: string,
    draft: FormDraft
): UiAction {
    const form = document.nodes.find((node) => node.kind === "form" && node.nodeId === formId);
    if (form === undefined || form.kind !== "form") {
        throw new PortableUiConsumerError("extension_ui_document_invalid");
    }
    const fields: UiSubmittedField[] = [];
    for (const node of document.nodes) {
        if (!isEditableField(node) || findFormAncestor(document, node) !== formId) {
            continue;
        }
        const value = draft.values[fieldId(node)];
        if (value === undefined || !validFieldValue(node, value)) {
            throw new PortableUiConsumerError("extension_ui_document_invalid");
        }
        fields.push({ fieldId: fieldId(node), value });
    }
    return {
        kind: "submit-form",
        actionId: form.payload.submitActionId,
        values: fields
    };
}

export function hasDirtyDraft(drafts: Record<string, FormDraft>): boolean {
    return Object.values(drafts).some((draft) => draft.dirty);
}

export function orderedNodeKinds(document: UiDocument): string[] {
    const result: string[] = [];
    walk(buildTree(document), (entry) => result.push(entry.node.kind));
    return result;
}

function walk(entry: UiTreeNode, visit: (entry: UiTreeNode) => void): void {
    visit(entry);
    for (const child of entry.children) {
        walk(child, visit);
    }
}

function initialEditableValue(node: UiNode): UiFieldValue | undefined {
    if (!isEditableField(node)) {
        return undefined;
    }
    switch (node.kind) {
        case "switch":
            return { kind: "boolean", value: node.payload.initialValue };
        case "text-field":
            return { kind: "text", value: node.payload.initialValue };
        case "integer-field":
            return node.payload.initialValue === undefined
                ? undefined
                : { kind: "integer", value: node.payload.initialValue };
        case "select":
            return node.payload.initialOptionId === undefined
                ? undefined
                : { kind: "selection", value: node.payload.initialOptionId };
        default:
            return assertNever(node);
    }
}

function isEditableField(node: UiNode): node is Extract<
    UiNode,
    { kind: "switch" | "text-field" | "integer-field" | "select" }
> {
    return (node.kind === "switch"
        || node.kind === "text-field"
        || node.kind === "integer-field"
        || node.kind === "select")
        && !node.payload.disabled
        && !node.payload.readOnly;
}

function fieldId(node: UiNode): string {
    switch (node.kind) {
        case "switch":
        case "text-field":
        case "integer-field":
        case "select":
            return node.payload.fieldId;
        default:
            throw new PortableUiConsumerError("extension_ui_document_invalid");
    }
}

function findFormAncestor(document: UiDocument, node: UiNode): string | undefined {
    let parentId = node.parentId;
    while (parentId !== undefined) {
        const parent = document.nodes.find((candidate) => candidate.nodeId === parentId);
        if (parent === undefined) {
            throw new PortableUiConsumerError("extension_ui_document_invalid");
        }
        if (parent.kind === "form") {
            return parent.nodeId;
        }
        parentId = parent.parentId;
    }
    return undefined;
}

function validFieldValue(
    node: Extract<UiNode, { kind: "switch" | "text-field" | "integer-field" | "select" }>,
    value: UiFieldValue
): boolean {
    switch (node.kind) {
        case "switch":
            return value.kind === "boolean";
        case "text-field":
            return value.kind === "text"
                && (!node.payload.required || value.value.length > 0)
                && utf8Length(value.value) >= node.payload.minLength
                && utf8Length(value.value) <= node.payload.maxLength;
        case "integer-field":
            return value.kind === "integer"
                && Number.isSafeInteger(value.value)
                && value.value >= node.payload.minimum
                && value.value <= node.payload.maximum;
        case "select":
            return value.kind === "selection"
                && node.payload.options.some((option) => option.optionId === value.value);
        default:
            return assertNever(node);
    }
}

function utf8Length(value: string): number {
    return new TextEncoder().encode(value).length;
}

function assertKnownNode(node: UiNode): void {
    switch (node.kind) {
        case "page":
        case "section":
        case "stack":
        case "group":
        case "form":
        case "list":
        case "list-item":
        case "text":
        case "status":
        case "key-value":
        case "progress":
        case "divider":
        case "button":
        case "switch":
        case "text-field":
        case "integer-field":
        case "select":
            return;
        default:
            return assertNever(node);
    }
}

function assertNever(value: never): never {
    void value;
    throw new PortableUiConsumerError("extension_ui_document_invalid");
}
