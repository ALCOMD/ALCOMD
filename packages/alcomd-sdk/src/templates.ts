export interface ProjectTemplateRef {
    id: string;
    revision: number;
}

export interface ProjectTemplateUnity {
    series: string[];
    presets: string[];
}

export interface ProjectTemplateDependency {
    packageId: string;
    versionRange: string;
}

export interface ProjectTemplateConstraint extends ProjectTemplateDependency {
    unityVersion: string;
}

export interface ProjectTemplateDeclaration {
    id: string;
    name: string;
    unity: ProjectTemplateUnity;
    dependencies: ProjectTemplateDependency[];
    constraints: ProjectTemplateConstraint[];
}

export interface ProjectTemplateRecord {
    declaration: ProjectTemplateDeclaration;
    revision: number;
    locked: boolean;
}

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
    declaration: ProjectTemplateDeclaration;
    expectedRevision?: number;
}

export interface ProjectTemplatesSaveResult {
    template: ProjectTemplateRecord;
}

export interface ProjectTemplatesRemoveParams {
    id: string;
    expectedRevision: number;
}

export interface ProjectTemplatesRemoveResult {
    id: string;
    revision: number;
}
