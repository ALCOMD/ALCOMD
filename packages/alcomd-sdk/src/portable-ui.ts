export const PORTABLE_UI_CAPABILITY = "extensions.ui.portable.v1" as const;

export type PortableUiProtocol = "portable-v1";
export type UiTone = "neutral" | "info" | "success" | "warning" | "danger";

export interface RpcError {
    code: string;
    message: string;
    diagnosticId?: string;
    data?: unknown;
}

export interface ExtensionUiDeclaration {
    protocol: PortableUiProtocol;
}

export interface ExtensionRecord {
    extensionId: string;
    version: string;
    apiMajor: number;
    packageDigest: string;
    publisherFingerprint: string;
    trustDecision: "official" | "user_approved_for_extension";
    desiredState: "installed_disabled" | "enabled" | "uninstalling";
    quarantineState: "clear" | "quarantined";
    runtimeState: "stopped" | "starting" | "running" | "stopping" | "crashed";
    grantRevision: number;
    lifecycleGeneration: number;
    revision: number;
    ui?: ExtensionUiDeclaration;
}

export interface ExtensionResult {
    extension: ExtensionRecord;
}

export type UiValidation =
    | { state: "valid" }
    | { state: "invalid"; message: string };

interface UiNodeBase<Kind extends string, Payload> {
    kind: Kind;
    nodeId: string;
    parentId?: string;
    order: number;
    payload: Payload;
}

export type UiNode =
    | UiNodeBase<"page", { title: string }>
    | UiNodeBase<"section", { label: string }>
    | UiNodeBase<"stack", { orientation: "vertical" | "horizontal" }>
    | UiNodeBase<"group", { label?: string }>
    | UiNodeBase<"form", { submitActionId: string; submitLabel: string; disabled: boolean }>
    | UiNodeBase<"list", { label?: string }>
    | UiNodeBase<"list-item", { label?: string }>
    | UiNodeBase<"text", { text: string; tone: UiTone }>
    | UiNodeBase<"status", { label: string; tone: UiTone }>
    | UiNodeBase<"key-value", { label: string; value: string }>
    | UiNodeBase<"progress", {
        label: string;
        value: { mode: "indeterminate" } | { mode: "determinate"; basisPoints: number };
    }>
    | UiNodeBase<"divider", Record<string, never>>
    | UiNodeBase<"button", { label: string; actionId: string; disabled: boolean }>
    | UiNodeBase<"switch", {
        fieldId: string;
        label: string;
        initialValue: boolean;
        disabled: boolean;
        readOnly: boolean;
        validation?: UiValidation;
    }>
    | UiNodeBase<"text-field", {
        fieldId: string;
        label: string;
        initialValue: string;
        required: boolean;
        minLength: number;
        maxLength: number;
        multiline: boolean;
        disabled: boolean;
        readOnly: boolean;
        validation?: UiValidation;
    }>
    | UiNodeBase<"integer-field", {
        fieldId: string;
        label: string;
        initialValue?: number;
        required: boolean;
        minimum: number;
        maximum: number;
        disabled: boolean;
        readOnly: boolean;
        validation?: UiValidation;
    }>
    | UiNodeBase<"select", {
        fieldId: string;
        label: string;
        initialOptionId?: string;
        required: boolean;
        options: Array<{ optionId: string; label: string }>;
        disabled: boolean;
        readOnly: boolean;
        validation?: UiValidation;
    }>;

export interface UiDocument {
    protocol: PortableUiProtocol;
    title: string;
    nodes: UiNode[];
}

export interface UiSnapshot {
    sessionId: string;
    snapshotRevision: number;
    document: UiDocument;
}

export interface ExtensionUiSession {
    sessionId: string;
    extensionId: string;
    locale: string;
    idleTimeoutMs: number;
    absoluteTimeoutMs: number;
}

export interface ExtensionUiOpenParams {
    extensionId: string;
    locale: string;
}

export interface ExtensionUiOpenResult {
    session: ExtensionUiSession;
    snapshot: UiSnapshot;
}

export interface ExtensionUiRefreshParams {
    sessionId: string;
    expectedSnapshotRevision: number;
}

export interface ExtensionUiSnapshotResult {
    snapshot: UiSnapshot;
}

export type UiFieldValue =
    | { kind: "boolean"; value: boolean }
    | { kind: "text"; value: string }
    | { kind: "integer"; value: number }
    | { kind: "selection"; value: string };

export interface UiSubmittedField {
    fieldId: string;
    value: UiFieldValue;
}

export type UiAction =
    | { kind: "activate"; actionId: string }
    | { kind: "submit-form"; actionId: string; values: UiSubmittedField[] };

export interface ExtensionUiDispatchParams {
    sessionId: string;
    expectedSnapshotRevision: number;
    sequence: number;
    requestId: string;
    action: UiAction;
}

export interface ExtensionUiDispatchResult {
    snapshot: UiSnapshot;
    replayed: boolean;
}

export interface ExtensionUiCloseParams {
    sessionId: string;
}

export interface ExtensionUiCloseResult {
    closed: boolean;
}
