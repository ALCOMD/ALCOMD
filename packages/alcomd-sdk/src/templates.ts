/** Creation targets a complete Editor version; that Editor need not be installed. */
export interface TemplatePlanCreateProjectParams {
    templateId: string;
    expectedTemplateRevision: number;
    targetParent: string;
    targetLeaf: string;
    unityVersion: string;
}

/** Template Bundle v1 declares one compatible Unity major/minor line. */
export interface TemplateUnityCompatibility {
    major: number;
    minor: number;
}
