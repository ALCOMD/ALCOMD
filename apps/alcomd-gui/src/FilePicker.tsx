import { arrowDownwardIcon } from "@alcomd/ui/icons";
import { useContext, useEffect, useEffectEvent, useRef, useState, type ComponentProps } from "react";
import { Button, Dialog, DialogCloseButton, DialogCloseContext, Icon, PathField } from "./Material";
import { useTranslation } from "./i18n";
import type { FileDrag, GuiRpcClient } from "./rpc";
import "./FilePicker.css";

type FilePickerProps = Pick<ComponentProps<typeof PathField>, "label" | "value" | "maxLength" | "disabled"> & {
    client: GuiRpcClient;
    chooseFile(): Promise<string | undefined>;
    onInput(path: string): void;
    onEnter?(): void;
};

type FilePickerFieldProps = Omit<ComponentProps<typeof PathField>, "onInput"> & Pick<FilePickerProps, "client" | "chooseFile" | "onInput"> & {
    onPicked?(path: string): void;
    onPickerOpenChange?(open: boolean): void;
    onRetry?(): void;
    selectDisabled?: boolean;
};

export function FilePickerField({ client, chooseFile, onInput, onPicked, onPickerOpenChange, onRetry, selectDisabled, disabled, ...field }: FilePickerFieldProps) {
    const { t } = useTranslation();
    const [draft, setDraft] = useState<string>();
    const trigger = useRef<HTMLElement>(null);
    const close = () => {
        setDraft(undefined);
        trigger.current?.focus();
        onPickerOpenChange?.(false);
    };
    const open = () => {
        if (onRetry !== undefined) {
            onRetry();
            return;
        }
        onPickerOpenChange?.(true);
        setDraft(field.value);
    };
    return <>
        <div className="file-picker__path">
            <PathField {...field} disabled={disabled} onInput={onInput} />
            <Button ref={trigger} disabled={disabled} softDisabled={selectDisabled} onClick={open} type="button" variant="tonal">{t(onRetry === undefined ? "Select" : "Retry")}</Button>
        </div>
        {draft === undefined ? null : <FilePickerDialog client={client} chooseFile={chooseFile} label={field.label}
            maxLength={field.maxLength} value={draft} onInput={setDraft} onCancel={close} onConfirm={() => {
                close();
                (onPicked ?? onInput)(draft);
            }} />}
    </>;
}

export function FilePicker({ client, chooseFile, onInput, disabled, onEnter, ...field }: FilePickerProps) {
    const { t } = useTranslation();
    const dialog = useContext(DialogCloseContext);
    const zone = useRef<HTMLElement>(null);
    const selecting = useRef(false);
    const [busy, setBusy] = useState(false);
    const [dragging, setDragging] = useState(false);
    const [failure, setFailure] = useState<"multiple" | "selection">();
    const receiveDrag = useEffectEvent((event: FileDrag) => {
        const target = event.type === "leave" ? null : document.elementFromPoint(event.x, event.y);
        const inside = !disabled && !selecting.current && target !== null && zone.current?.contains(target) === true;
        setDragging(inside && event.type === "hover");
        if (!inside || event.type !== "drop") return;
        if (event.paths.length !== 1) {
            setFailure("multiple");
            return;
        }
        setFailure(undefined);
        onInput(event.paths[0]!);
    });

    useEffect(() => {
        let active = true;
        let unsubscribe: (() => void) | undefined;
        void client.subscribeFileDrag?.((event) => { if (active) receiveDrag(event); }).then((stop) => {
            if (active) unsubscribe = stop;
            else stop();
        }).catch(() => {});
        return () => { active = false; unsubscribe?.(); };
    }, [client]);

    const select = async () => {
        if (disabled || selecting.current) return;
        const unlock = dialog?.lockForFilePicker();
        if (dialog !== undefined && unlock === undefined) return;
        selecting.current = true;
        setBusy(true);
        setDragging(false);
        setFailure(undefined);
        try {
            const path = await chooseFile();
            if (zone.current !== null && path !== undefined) onInput(path);
        } catch {
            if (zone.current !== null) setFailure("selection");
        } finally {
            selecting.current = false;
            unlock?.();
            if (zone.current !== null) setBusy(false);
        }
    };

    return <div className="file-picker">
        <Button className="file-picker__drop" data-dragging={dragging && !disabled} disabled={disabled}
            softDisabled={busy} onClick={() => void select()} ref={zone} type="button" variant="text">
            <span className="file-picker__drop-label">
                <Icon asset={arrowDownwardIcon} />
                <span>{t(dragging && !disabled ? "Release to select the file" : "Drop a file here or click to select")}</span>
            </span>
        </Button>
        <PathField {...field} disabled={disabled} readOnly={busy} placeholder={t("Enter or paste a file path")}
            onKeyDown={onEnter === undefined ? undefined : (event) => {
                if (event.key !== "Enter" || event.nativeEvent.isComposing || event.keyCode === 229) return;
                event.preventDefault();
                if (!disabled && !selecting.current) onEnter();
            }}
            onInput={(path) => { setFailure(undefined); onInput(path); }} />
        {failure === undefined ? null : <p className="file-picker__error" role="alert">{t(failure === "multiple" ? "Select one file at a time." : "Could not select the file.")}</p>}
    </div>;
}

function FilePickerDialog({ onCancel, onConfirm, ...field }: Omit<FilePickerProps, "disabled" | "onEnter"> & {
    onCancel(): void;
    onConfirm(): void;
}) {
    const { t } = useTranslation();
    const button = useRef<HTMLElement>(null);
    const canConfirm = field.value.trim() !== "";
    useEffect(() => { button.current?.focus(); }, []);
    return <Dialog open title={t("Select file")} onClose={onCancel} actions={
        <DialogCloseContext.Consumer>{(dialog) => <>
            <DialogCloseButton variant="text">{t("Cancel")}</DialogCloseButton>
            <Button ref={button} disabled={!canConfirm || dialog?.filePickerOpen} onClick={() => dialog?.requestClose(onConfirm)} type="button">{t("Confirm")}</Button>
        </>}</DialogCloseContext.Consumer>
    }>
        <DialogCloseContext.Consumer>{(dialog) => <FilePicker {...field}
            onEnter={() => { if (canConfirm) dialog?.requestClose(onConfirm); }} />}</DialogCloseContext.Consumer>
    </Dialog>;
}
