import { arrowDownwardIcon } from "@alcomd/ui/icons";
import { useEffect, useEffectEvent, useLayoutEffect, useRef, useState, type ComponentProps } from "react";
import { useTranslation } from "./i18n";
import { Button, Icon, PathField } from "./Material";
import type { FileDrag, GuiRpcClient } from "./rpc";
import "./FilePicker.css";

export type FilePickerControlProps = Omit<ComponentProps<typeof PathField>, "onInput"> & {
    client: Pick<GuiRpcClient, "subscribeFileDrag">;
    kind?: "file" | "directory";
    choosePath(): Promise<string | undefined>;
    onPathChange(path: string): void;
    onChoosingChange?(choosing: boolean): void;
};

const messages = {
    file: { idle: "Drop a file here or click to select", hover: "Release to select the file", placeholder: "Enter or paste a file path", multiple: "Select one file at a time.", selection: "Could not select the file." },
    directory: { idle: "Drop a folder here or click to select", hover: "Release to select the folder", placeholder: "Enter or paste a folder path", multiple: "Select one folder at a time.", selection: "Could not select the folder." }
} as const;

export function FilePickerControl({ client, kind = "file", choosePath, onPathChange, onChoosingChange, disabled, readOnly, placeholder, ...field }: FilePickerControlProps) {
    const { t } = useTranslation();
    const dropZone = useRef<HTMLElement>(null);
    const choosing = useRef(false);
    const [busy, setBusy] = useState(false);
    const [dragging, setDragging] = useState(false);
    const [failure, setFailure] = useState<"multiple" | "selection">();
    const unavailable = disabled || readOnly;
    const labels = messages[kind];
    const latest = useRef({ onPathChange, unavailable });

    useLayoutEffect(() => {
        latest.current = { onPathChange, unavailable };
    }, [onPathChange, unavailable]);

    const receiveDrag = useEffectEvent((event: FileDrag) => {
        const target = event.type === "leave" ? null : document.elementFromPoint(event.x, event.y);
        const inside = !unavailable && !choosing.current && target !== null && dropZone.current?.contains(target);
        setDragging(event.type === "hover" && Boolean(inside));
        if (event.type !== "drop" || !inside) return;

        const [path] = event.paths;
        if (path === undefined) return;
        if (event.paths.length > 1) {
            setFailure("multiple");
            return;
        }
        setFailure(undefined);
        onPathChange(path);
    });

    useEffect(() => {
        let active = true;
        let unsubscribe: (() => void) | undefined;
        void client.subscribeFileDrag?.((event) => {
            if (active) receiveDrag(event);
        }).then((stop) => {
            if (active) unsubscribe = stop;
            else stop();
        }).catch(() => {
            if (active) setDragging(false);
        });
        return () => {
            active = false;
            unsubscribe?.();
        };
    }, [client]);

    const openPicker = async () => {
        if (unavailable || choosing.current) return;
        choosing.current = true;
        setBusy(true);
        onChoosingChange?.(true);
        setDragging(false);
        setFailure(undefined);
        try {
            const path = await choosePath();
            if (path !== undefined && dropZone.current !== null && !latest.current.unavailable) {
                latest.current.onPathChange(path);
            }
        } catch {
            if (dropZone.current !== null) setFailure("selection");
        } finally {
            choosing.current = false;
            onChoosingChange?.(false);
            if (dropZone.current !== null) setBusy(false);
        }
    };

    const showDrag = dragging && !unavailable && !busy;
    return <div className="file-picker" aria-busy={busy}>
        <Button className="file-picker__drop" data-dragging={showDrag ? "true" : undefined} disabled={disabled}
            softDisabled={busy || readOnly} onClick={openPicker} ref={dropZone} type="button" variant="text">
            <span className="file-picker__drop-label">
                <Icon asset={arrowDownwardIcon} />
                <span>{t(showDrag ? labels.hover : labels.idle)}</span>
            </span>
        </Button>
        <div className="file-picker__path">
            <PathField {...field} disabled={disabled} readOnly={readOnly || busy} placeholder={placeholder ?? t(labels.placeholder)}
                onInput={(path) => { setFailure(undefined); onPathChange(path); }} />
            <Button disabled={disabled} softDisabled={busy || readOnly} onClick={openPicker} type="button" variant="tonal">{t("Select")}</Button>
        </div>
        {failure === undefined ? null : <p className="file-picker__error" role="alert">{t(labels[failure])}</p>}
    </div>;
}
