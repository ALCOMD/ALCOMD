import type { FormEvent, ReactNode } from "react";
import { useEffect, useState } from "react";
import type { UiAction, UiFieldValue, UiNode, UiSnapshot } from "@alcomd/sdk";

import {
    PortableUiConsumerError,
    buildTree,
    createFormDrafts,
    submitFormAction,
    updateDraft,
    type FormDraft,
    type UiTreeNode
} from "./portable-ui";
import { Button, Progress, Select, Switch, TextField } from "./Material";

interface PortableUiRendererProps {
    snapshot: UiSnapshot;
    busy: boolean;
    onAction(action: UiAction): Promise<void>;
    onDirtyChange(dirty: boolean): void;
}

export function PortableUiRenderer({
    snapshot,
    busy,
    onAction,
    onDirtyChange
}: PortableUiRendererProps) {
    const [drafts, setDrafts] = useState<Record<string, FormDraft>>(
        () => createFormDrafts(snapshot)
    );
    const [formError, setFormError] = useState<{ formId: string; message: string }>();
    const tree = buildTree(snapshot.document);

    useEffect(() => {
        setDrafts(createFormDrafts(snapshot));
        setFormError(undefined);
        onDirtyChange(false);
    }, [snapshot.sessionId, snapshot.snapshotRevision, onDirtyChange]);

    const changeField = (
        formId: string | undefined,
        fieldId: string,
        value: UiFieldValue | undefined
    ) => {
        if (formId === undefined) {
            throw new PortableUiConsumerError("extension_ui_document_invalid");
        }
        setFormError(undefined);
        setDrafts((current) => {
            const draft = current[formId];
            if (draft === undefined) {
                throw new PortableUiConsumerError("extension_ui_document_invalid");
            }
            return { ...current, [formId]: updateDraft(draft, fieldId, value) };
        });
        onDirtyChange(true);
    };

    const submit = async (event: FormEvent<HTMLFormElement>, formId: string) => {
        event.preventDefault();
        const draft = drafts[formId];
        if (draft === undefined) {
            setFormError({
                formId,
                message: "The form is no longer current. Refresh the extension page."
            });
            return;
        }
        try {
            setFormError(undefined);
            await onAction(submitFormAction(snapshot.document, formId, draft));
        } catch (error: unknown) {
            if (error instanceof PortableUiConsumerError) {
                setFormError({
                    formId,
                    message: "Check the editable fields before submitting."
                });
                return;
            }
            throw error;
        }
    };

    const renderNode = (entry: UiTreeNode, inheritedDisabled = false): ReactNode => {
        const { node } = entry;
        const children = entry.children.map((child) => renderNode(child, inheritedDisabled));
        const key = node.nodeId;
        switch (node.kind) {
            case "page":
                return (
                    <section className="portable-page" aria-labelledby={`${key}-title`} key={key}>
                        <h2 id={`${key}-title`}>{node.payload.title}</h2>
                        {children}
                    </section>
                );
            case "section":
                return (
                    <section className="portable-section" aria-labelledby={`${key}-label`} key={key}>
                        <h3 id={`${key}-label`}>{node.payload.label}</h3>
                        {children}
                    </section>
                );
            case "stack":
                return (
                    <div className={`portable-stack portable-stack--${node.payload.orientation}`} key={key}>
                        {children}
                    </div>
                );
            case "group":
                return node.payload.label === undefined ? (
                    <div className="portable-group" key={key}>{children}</div>
                ) : (
                    <section className="portable-group" aria-labelledby={`${key}-label`} key={key}>
                        <h4 id={`${key}-label`}>{node.payload.label}</h4>
                        {children}
                    </section>
                );
            case "form": {
                const disabled = inheritedDisabled || node.payload.disabled || busy;
                const formChildren = entry.children.map((child) => renderNode(child, disabled));
                return (
                    <form
                        className="portable-form"
                        key={key}
                        onSubmit={(event) => void submit(event, node.nodeId)}
                    >
                        <fieldset disabled={disabled}>
                            {formChildren}
                            <Button disabled={disabled} type="submit">
                                {node.payload.submitLabel}
                            </Button>
                        </fieldset>
                        {formError?.formId !== node.nodeId ? null : (
                            <p className="field-error" role="alert">{formError.message}</p>
                        )}
                    </form>
                );
            }
            case "list":
                return (
                    <section className="portable-list" aria-label={node.payload.label} key={key}>
                        <ul>{children}</ul>
                    </section>
                );
            case "list-item":
                return (
                    <li className="portable-list-item" key={key}>
                        {node.payload.label === undefined ? null : <strong>{node.payload.label}</strong>}
                        {children}
                    </li>
                );
            case "text":
                return <p className={`tone tone--${node.payload.tone}`} key={key}>{node.payload.text}</p>;
            case "status":
                return (
                    <p className={`portable-status tone--${node.payload.tone}`} role="status" key={key}>
                        {node.payload.label}
                    </p>
                );
            case "key-value":
                return (
                    <dl className="portable-key-value" key={key}>
                        <dt>{node.payload.label}</dt>
                        <dd>{node.payload.value}</dd>
                    </dl>
                );
            case "progress": {
                const progress = node.payload.value.mode === "determinate"
                    ? node.payload.value.basisPoints
                    : undefined;
                return (
                    <div className="portable-progress" key={key}>
                        <span>{node.payload.label}</span>
                        <Progress label={node.payload.label} max={10_000} value={progress} />
                    </div>
                );
            }
            case "divider":
                return <hr className="portable-divider" key={key} />;
            case "button":
                return (
                    <Button
                        disabled={inheritedDisabled || node.payload.disabled || busy}
                        key={key}
                        onClick={() => void onAction({
                            kind: "activate",
                            actionId: node.payload.actionId
                        })}
                        variant="tonal"
                    >
                        {node.payload.label}
                    </Button>
                );
            case "switch":
                return renderSwitch(node, entry.formId, drafts, inheritedDisabled || busy, changeField);
            case "text-field":
                return renderTextField(node, entry.formId, drafts, inheritedDisabled || busy, changeField);
            case "integer-field":
                return renderIntegerField(node, entry.formId, drafts, inheritedDisabled || busy, changeField);
            case "select":
                return renderSelect(node, entry.formId, drafts, inheritedDisabled || busy, changeField);
            default:
                return assertNever(node);
        }
    };

    return <div className="portable-document">{renderNode(tree)}</div>;
}

type FieldNode = Extract<UiNode, { kind: "switch" | "text-field" | "integer-field" | "select" }>;
type ChangeField = (formId: string | undefined, fieldId: string, value: UiFieldValue | undefined) => void;

function renderSwitch(
    node: Extract<FieldNode, { kind: "switch" }>,
    formId: string | undefined,
    drafts: Record<string, FormDraft>,
    inheritedDisabled: boolean,
    changeField: ChangeField
): ReactNode {
    const current = fieldValue(drafts, formId, node.payload.fieldId);
    const checked = current?.kind === "boolean" ? current.value : node.payload.initialValue;
    const validationId = invalidValidationId(node);
    return (
        <div className="portable-switch" key={node.nodeId}>
            <Switch
                aria-describedby={validationId}
                aria-invalid={validationId === undefined ? undefined : true}
                aria-readonly={node.payload.readOnly}
                disabled={inheritedDisabled || node.payload.disabled || node.payload.readOnly}
                label={node.payload.label}
                onChange={(selected) => {
                    if (!node.payload.readOnly) {
                        changeField(formId, node.payload.fieldId, {
                            kind: "boolean",
                            value: selected
                        });
                    }
                }}
                selected={checked}
            />
            {renderValidation(node)}
        </div>
    );
}

function renderTextField(
    node: Extract<FieldNode, { kind: "text-field" }>,
    formId: string | undefined,
    drafts: Record<string, FormDraft>,
    inheritedDisabled: boolean,
    changeField: ChangeField
): ReactNode {
    const current = fieldValue(drafts, formId, node.payload.fieldId);
    const value = current?.kind === "text" ? current.value : node.payload.initialValue;
    const validationId = invalidValidationId(node);
    const validationMessage = node.payload.validation?.state === "invalid"
        ? node.payload.validation.message
        : undefined;
    return (
        <div className="portable-field" key={node.nodeId}>
            <TextField
                aria-describedby={validationId}
                aria-invalid={validationId === undefined ? undefined : true}
                disabled={inheritedDisabled || node.payload.disabled}
                error={validationId !== undefined}
                errorText={validationMessage}
                id={node.payload.fieldId}
                label={node.payload.label}
                maxLength={node.payload.maxLength}
                minLength={node.payload.minLength}
                onInput={(nextValue) => {
                    changeField(formId, node.payload.fieldId, {
                        kind: "text",
                        value: nextValue
                    });
                }}
                readOnly={node.payload.readOnly}
                required={node.payload.required}
                rows={node.payload.multiline ? 4 : undefined}
                type={node.payload.multiline ? "textarea" : "text"}
                value={value}
            />
        </div>
    );
}

function renderIntegerField(
    node: Extract<FieldNode, { kind: "integer-field" }>,
    formId: string | undefined,
    drafts: Record<string, FormDraft>,
    inheritedDisabled: boolean,
    changeField: ChangeField
): ReactNode {
    const current = fieldValue(drafts, formId, node.payload.fieldId);
    const value = current?.kind === "integer" ? current.value : (node.payload.initialValue ?? "");
    const validationId = invalidValidationId(node);
    const validationMessage = node.payload.validation?.state === "invalid"
        ? node.payload.validation.message
        : undefined;
    return (
        <div className="portable-field" key={node.nodeId}>
            <TextField
                aria-describedby={validationId}
                aria-invalid={validationId === undefined ? undefined : true}
                disabled={inheritedDisabled || node.payload.disabled}
                error={validationId !== undefined}
                errorText={validationMessage}
                id={node.payload.fieldId}
                label={node.payload.label}
                max={node.payload.maximum}
                min={node.payload.minimum}
                onInput={(input) => {
                    const parsed = Number(input);
                    changeField(
                        formId,
                        node.payload.fieldId,
                        input === "" || !Number.isSafeInteger(parsed)
                            ? undefined
                            : { kind: "integer", value: parsed }
                    );
                }}
                readOnly={node.payload.readOnly}
                required={node.payload.required}
                type="number"
                value={value}
            />
        </div>
    );
}

function renderSelect(
    node: Extract<FieldNode, { kind: "select" }>,
    formId: string | undefined,
    drafts: Record<string, FormDraft>,
    inheritedDisabled: boolean,
    changeField: ChangeField
): ReactNode {
    const current = fieldValue(drafts, formId, node.payload.fieldId);
    const value = current?.kind === "selection" ? current.value : (node.payload.initialOptionId ?? "");
    const validationId = invalidValidationId(node);
    return (
        <div className="portable-field" key={node.nodeId}>
            <Select
                aria-describedby={validationId}
                aria-invalid={validationId === undefined ? undefined : true}
                disabled={inheritedDisabled || node.payload.disabled || node.payload.readOnly}
                id={node.payload.fieldId}
                label={node.payload.label}
                onChange={(nextValue) => {
                    if (!node.payload.readOnly) {
                        changeField(formId, node.payload.fieldId, {
                            kind: "selection",
                            value: nextValue
                        });
                    }
                }}
                options={[
                    ...(node.payload.initialOptionId === undefined ? [{ label: "Select…", value: "" }] : []),
                    ...node.payload.options.map((option) => ({ label: option.label, value: option.optionId }))
                ]}
                required={node.payload.required}
                value={value}
            />
            {renderValidation(node)}
        </div>
    );
}

function fieldValue(
    drafts: Record<string, FormDraft>,
    formId: string | undefined,
    fieldId: string
): UiFieldValue | undefined {
    return formId === undefined ? undefined : drafts[formId]?.values[fieldId];
}

function invalidValidationId(node: FieldNode): string | undefined {
    return node.payload.validation?.state === "invalid"
        ? `${node.nodeId}-validation`
        : undefined;
}

function renderValidation(node: FieldNode): ReactNode {
    return node.payload.validation?.state === "invalid" ? (
        <span className="field-error" id={`${node.nodeId}-validation`}>
            {node.payload.validation.message}
        </span>
    ) : null;
}

function assertNever(value: never): never {
    void value;
    throw new PortableUiConsumerError("extension_ui_document_invalid");
}
