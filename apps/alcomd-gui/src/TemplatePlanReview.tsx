import type { TemplateBundleInspection, TemplatePlan, TemplateRecord } from "./core-models";
import { useTranslation } from "./i18n";

type ReviewContext = {
    plan: TemplatePlan;
    templateId: string;
    displayName: string;
    templateVersion: string;
};

export type TemplateReview = ReviewContext & (
    { action: "import" | "override"; expectedRevision: number; replacing: boolean }
    | { action: "derive"; sourceProjectId: string; sourceProjectRevision: number }
    | { action: "create_project"; templateRevision: number; targetParent: string; targetLeaf: string }
);

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
const SHA256 = /^[0-9a-f]{64}$/;

function requireEvidence(valid: boolean): asserts valid {
    if (!valid) throw { code: "internal_error" };
}

function validRevision(value: unknown, allowAbsent = false): value is number {
    return typeof value === "number" && Number.isSafeInteger(value) && value >= (allowAbsent ? 0 : 1);
}

function validHash(value: unknown): value is string { return typeof value === "string" && SHA256.test(value); }
function validId(value: unknown): value is string { return typeof value === "string" && UUID.test(value); }
function opaqueIdentity(value: unknown): boolean { return typeof value === "string" && value.length > 0 && value.length <= 256; }

function validateBase(plan: TemplatePlan) {
    requireEvidence(validId(plan.planId) && validId(plan.templateId) && plan.state === "unapplied" && validHash(plan.planFingerprint));
}

export function reviewTemplateImport(plan: TemplatePlan, inspection: TemplateBundleInspection, existing: TemplateRecord | undefined, replaceRequested: boolean): TemplateReview {
    validateBase(plan);
    const replacing = existing !== undefined;
    requireEvidence(plan.action === (replacing ? "override" : "import")
        && (!replacing || replaceRequested || existing.bundleSha256 === inspection.bundleSha256)
        && plan.templateId === inspection.templateId
        && validRevision(plan.expectedRevision, true) && plan.expectedRevision === (existing?.revision ?? 0)
        && opaqueIdentity(plan.sourceFilesystemIdentity)
        && (replacing ? validHash(plan.oldBundleSha256) && plan.oldBundleSha256 === existing.bundleSha256 : plan.oldBundleSha256 === null)
        && validHash(plan.newBundleSha256) && plan.newBundleSha256 === inspection.bundleSha256
        && validHash(plan.manifestFingerprint) && plan.manifestFingerprint === inspection.manifestFingerprint);
    return { plan, action: replacing ? "override" : "import", templateId: inspection.templateId, displayName: inspection.displayName, templateVersion: inspection.templateVersion, expectedRevision: plan.expectedRevision as number, replacing };
}

export function reviewTemplateDerive(plan: TemplatePlan, request: { templateId: string; projectId: string; expectedProjectRevision: number; displayName: string; templateVersion: string }): TemplateReview {
    validateBase(plan);
    requireEvidence(plan.action === "derive" && plan.templateId === request.templateId
        && validId(plan.sourceProjectId) && plan.sourceProjectId === request.projectId
        && validRevision(plan.sourceProjectRevision) && plan.sourceProjectRevision === request.expectedProjectRevision
        && validHash(plan.sourceProjectFingerprint) && plan.includePolicyVersion === 1);
    return { plan, action: "derive", templateId: request.templateId, displayName: request.displayName, templateVersion: request.templateVersion, sourceProjectId: request.projectId, sourceProjectRevision: request.expectedProjectRevision };
}

export function reviewTemplateCreation(plan: TemplatePlan, template: TemplateRecord, targetParent: string, targetLeaf: string): TemplateReview {
    validateBase(plan);
    requireEvidence(plan.action === "create_project" && plan.templateId === template.templateId
        && validRevision(plan.templateRevision) && plan.templateRevision === template.revision
        && validHash(plan.bundleSha256) && plan.bundleSha256 === template.bundleSha256
        && validHash(plan.manifestFingerprint) && plan.manifestFingerprint === template.manifestFingerprint
        && validHash(plan.payloadTreeSha256) && validHash(plan.packageChangeSetFingerprint) && validHash(plan.projectSummaryFingerprint)
        && opaqueIdentity(plan.parentFilesystemIdentity)
        && typeof plan.targetLeaf === "string" && plan.targetLeaf.length > 0 && plan.targetLeaf === targetLeaf
        && plan.targetMustBeAbsent === true
        && Array.isArray(plan.resourceDigests) && plan.resourceDigests.length <= 4096 && plan.resourceDigests.every(validHash) && new Set(plan.resourceDigests).size === plan.resourceDigests.length);
    return { plan, action: "create_project", templateId: template.templateId, displayName: template.displayName, templateVersion: template.templateVersion, templateRevision: template.revision, targetParent, targetLeaf };
}

export function TemplatePlanReview({ review }: { review: TemplateReview }) {
    const { t } = useTranslation();
    const action = review.action === "import" ? t("Import template") : review.action === "override" ? t("Replace an existing matching template") : review.action === "derive" ? t("Derive from project") : t("Create project");
    return <>
        <dl className="dialog-summary">
            <div><dt>{t("Action:")}</dt><dd>{action}</dd></div>
            <div><dt>{t("Template ID")}</dt><dd><code>{review.templateId}</code></dd></div>
            <div><dt>{review.action === "derive" ? t("Display name") : t("Template")}</dt><dd>{review.displayName}</dd></div>
            <div><dt>{review.action === "derive" ? t("Template version") : t("Version")}</dt><dd>{review.templateVersion}</dd></div>
            {review.action === "import" || review.action === "override" ? <>
                <div><dt>{t("Expected revision")}</dt><dd>{review.expectedRevision}</dd></div>
                <div><dt>{t("Replace existing template")}</dt><dd>{review.replacing ? t("Yes") : t("No")}</dd></div>
            </> : review.action === "derive" ? <>
                <div><dt>{t("Source project")}</dt><dd><code>{review.sourceProjectId}</code></dd></div>
                <div><dt>{t("Source project revision")}</dt><dd>{review.sourceProjectRevision}</dd></div>
            </> : review.action === "create_project" ? <>
                <div><dt>{t("Template revision")}</dt><dd>{review.templateRevision}</dd></div>
                <div><dt>{t("Destination")}</dt><dd><code>{review.targetParent}</code><br /><code>{review.targetLeaf}</code></dd></div>
                <div><dt>{t("Destination must be absent")}</dt><dd>{t("Yes")}</dd></div>
            </> : null}
        </dl>
        <p>{t("Plan fingerprint:")}<code title={review.plan.planFingerprint}>{review.plan.planFingerprint.slice(0, 12)}</code></p>
    </>;
}
