import { arrowDownwardIcon } from "@alcomd/ui/icons";
import { useEffect, useEffectEvent, useLayoutEffect, useRef, useState, type ComponentProps } from "react";
import { useTranslation } from "./i18n";
import { Button, Icon, PathField } from "./Material";
import type { FileDrag, GuiRpcClient } from "./rpc";
import "./FilePicker.css";

type FilePickerProps = Omit<ComponentProps<typeof PathField>, "onInput"> & {
    client: Pick<GuiRpcClient, "subscribeFileDrag">;
    kind: "file" | "directory";
    choosePath(): Promise<string | undefined>;
    onPathChange(path: string): void;
    onChoosingChange(choosing: boolean): void;
};

const messages = {
    file: { idle: "Drop a file here or click to select", hover: "Release to select the file", placeholder: "Enter or paste a file path", multiple: "Select one file at a time.", selection: "Could not select the file." },
    directory: { idle: "Drop a folder here or click to select", hover: "Release to select the folder", placeholder: "Enter or paste a folder path", multiple: "Select one folder at a time.", selection: "Could not select the folder." }
} as const;

export function FilePicker({ client, kind, choosePath, onPathChange, onChoosingChange, disabled, readOnly, placeholder, ...field }: FilePickerProps) {
    const { t } = useTranslation();
    const zone = useRef<HTMLElement>(null);
    const pending = useRef(false);
    const mounted = useRef(false);
    const generation = useRef(0);
    const latest = useRef({ onPathChange, onChoosingChange });
    const [choosing, setChoosing] = useState(false);
    const [dragging, setDragging] = useState(false);
    const [failure, setFailure] = useState<"multiple" | "selection">();
    const unavailable = Boolean(disabled || readOnly);
    const labels = messages[kind];

    useLayoutEffect(() => {
        latest.current = { onPathChange, onChoosingChange };
    });
    useLayoutEffect(() => {
        mounted.current = true;
        generation.current += 1;
        return () => { mounted.current = false; generation.current += 1; };
    }, [client, kind, unavailable]);

    const receiveDrag = useEffectEvent((event: FileDrag) => {
        const target = event.type === "leave" ? null : document.elementFromPoint(event.x, event.y);
        const inside = !unavailable && !pending.current && target !== null && zone.current?.contains(target) === true;
        setDragging(inside && event.type === "hover");
        if (!inside || event.type !== "drop" || event.paths.length === 0) return;
        if (event.paths.length !== 1) {
            setFailure("multiple");
            return;
        }
        setFailure(undefined);
        onPathChange(event.paths[0]!);
    });

    useEffect(() => {
        let active = true;
        let stop: (() => void) | undefined;
        void client.subscribeFileDrag?.((event) => { if (active) receiveDrag(event); }).then((unsubscribe) => {
            if (active) stop = unsubscribe;
            else unsubscribe();
        }).catch(() => { if (active) setDragging(false); });
        return () => { active = false; stop?.(); };
    }, [client]);

    const select = async () => {
        if (unavailable || pending.current) return;
        pending.current = true;
        const request = generation.current;
        setChoosing(true);
        onChoosingChange(true);
        setDragging(false);
        setFailure(undefined);
        try {
            const path = await choosePath();
            if (mounted.current && request === generation.current && path !== undefined) latest.current.onPathChange(path);
        } catch {
            if (mounted.current && request === generation.current) setFailure("selection");
        } finally {
            pending.current = false;
            if (mounted.current) {
                setChoosing(false);
                latest.current.onChoosingChange(false);
            }
        }
    };

    const hovering = dragging && !unavailable && !choosing;
    return <div className="file-picker" aria-busy={choosing}>
        <Button className="file-picker__drop" data-dragging={hovering ? "true" : undefined} disabled={disabled}
            softDisabled={choosing || readOnly} onClick={() => void select()} ref={zone} type="button" variant="text">
            <span className="file-picker__drop-label"><Icon asset={arrowDownwardIcon} /><span>{t(hovering ? labels.hover : labels.idle)}</span></span>
        </Button>
        <div className="file-picker__path">
            <PathField {...field} disabled={disabled} readOnly={readOnly || choosing} placeholder={placeholder ?? t(labels.placeholder)}
                onInput={(path) => { setFailure(undefined); onPathChange(path); }} />
            <Button disabled={disabled} softDisabled={choosing || readOnly} onClick={() => void select()} type="button" variant="tonal">{t("Select")}</Button>
        </div>
        {failure === undefined ? null : <p className="file-picker__error" role="alert">{t(labels[failure])}</p>}
    </div>;
}
