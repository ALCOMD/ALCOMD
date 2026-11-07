import { arrowDownwardIcon } from "@alcomd/ui/icons";
import { useEffect, useEffectEvent, useRef, useState, type ComponentProps } from "react";
import { Button, Icon, PathField } from "./Material";
import { useTranslation } from "./i18n";
import type { FileDrag, GuiRpcClient } from "./rpc";
import "./FilePicker.css";

type FilePickerProps = Omit<ComponentProps<typeof PathField>, "onInput"> & {
    client: GuiRpcClient;
    chooseFile(): Promise<string | undefined>;
    onInput(path: string): void;
    onPicked?(path: string): void;
    selectDisabled?: boolean;
    selectLabel?: string;
};

export function FilePicker({ client, chooseFile, onInput, onPicked, selectDisabled, selectLabel, disabled, placeholder, ...field }: FilePickerProps) {
    const { t } = useTranslation();
    const zone = useRef<HTMLElement>(null);
    const selecting = useRef(false);
    const [busy, setBusy] = useState(false);
    const [dragging, setDragging] = useState(false);
    const [failure, setFailure] = useState<"multiple" | "selection">();
    const unavailable = disabled || selectDisabled;
    const receiveDrag = useEffectEvent((event: FileDrag) => {
        const target = event.type === "leave" ? null : document.elementFromPoint(event.x, event.y);
        const inside = !unavailable && !selecting.current && target !== null && zone.current?.contains(target) === true;
        setDragging(inside && event.type === "hover");
        if (!inside || event.type !== "drop") return;
        if (event.paths.length !== 1) {
            setFailure("multiple");
            return;
        }
        setFailure(undefined);
        (onPicked ?? onInput)(event.paths[0]!);
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
        if (unavailable || selecting.current) return;
        selecting.current = true;
        setBusy(true);
        setDragging(false);
        setFailure(undefined);
        try {
            const path = await chooseFile();
            if (zone.current !== null && path !== undefined) (onPicked ?? onInput)(path);
        } catch {
            if (zone.current !== null) setFailure("selection");
        } finally {
            selecting.current = false;
            if (zone.current !== null) setBusy(false);
        }
    };

    const openPicker = () => void select();
    return <div className="file-picker">
        <Button className="file-picker__drop" data-dragging={dragging && !unavailable} disabled={disabled}
            softDisabled={busy || selectDisabled} onClick={openPicker} ref={zone} type="button" variant="text">
            <span className="file-picker__drop-label">
                <Icon asset={arrowDownwardIcon} />
                <span>{t(dragging && !unavailable ? "Release to select the file" : "Drop a file here or click to select")}</span>
            </span>
        </Button>
        <div className="file-picker__path">
            <PathField {...field} disabled={disabled} readOnly={field.readOnly || busy} placeholder={placeholder ?? t("Enter or paste a file path")}
                onInput={(path) => { setFailure(undefined); onInput(path); }} />
            <Button disabled={disabled} softDisabled={busy || selectDisabled} onClick={openPicker} type="button" variant="tonal">{selectLabel ?? t("Select")}</Button>
        </div>
        {failure === undefined ? null : <p className="file-picker__error" role="alert">{t(failure === "multiple" ? "Select one file at a time." : "Could not select the file.")}</p>}
    </div>;
}
