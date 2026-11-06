export interface ProjectTemplateRef {
    id: string;
    revision: number;
}

export interface ProjectTemplateDependency {
    packageId: string;
    versionRange: string;
}

export interface ProjectTemplateRecord {
    id: string;
    name: string;
    editorVersion?: string;
    revision: number;
    locked: boolean;
}

export type ProjectTemplateSource =
    | { kind: "file"; sourceFile: string }
    | { kind: "project"; projectId: string; expectedProjectRevision: number; id: string; name: string; includeAssets?: boolean; lockPackageVersions?: boolean }
    | { kind: "manual"; id: string; name: string; unityVersion: string; dependencies: ProjectTemplateDependency[] };

export interface ProjectsCreateParams {
    name: string;
    targetParent: string;
    unityVersion: string;
    idempotencyKey: string;
    template?: ProjectTemplateRef;
}

export interface ProjectsCreateResult {
    operationId: string;
    projectId: string;
    replayed: boolean;
}

export interface ProjectTemplatesListParams {
    limit?: number;
    cursor?: string;
}

export interface ProjectTemplatesListResult {
    templates: ProjectTemplateRecord[];
    nextCursor?: string;
}

export interface ProjectTemplatesSaveParams {
    source: ProjectTemplateSource;
    expectedRevision?: number;
    idempotencyKey: string;
}

export interface ProjectTemplateOperationAccepted {
    operationId: string;
    replayed: boolean;
}

export type ProjectTemplatesSaveResult = ProjectTemplateOperationAccepted;

export interface ProjectTemplateSaveOperationResult {
    template: ProjectTemplateRecord;
}

export interface ProjectTemplatesExportParams {
    id: string;
    revision: number;
    destinationFile: string;
    idempotencyKey: string;
}

export type ProjectTemplatesExportResult = ProjectTemplateOperationAccepted;

export interface ProjectTemplateExportOperationResult {
    id: string;
    revision: number;
}

export interface ProjectTemplatesRemoveParams {
    id: string;
    expectedRevision: number;
}

export interface ProjectTemplatesRemoveResult {
    id: string;
    revision: number;
}
