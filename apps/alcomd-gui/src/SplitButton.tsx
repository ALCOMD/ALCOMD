import { keyboardArrowDownIcon } from "@alcomd/ui/icons";
import { useEffect, useState, type ReactNode, type Ref } from "react";

import { Button, Icon, Menu } from "./Material";

export interface SplitButtonProps {
    "aria-busy"?: boolean;
    children: ReactNode;
    /** Primary action availability; menu items retain their own permissions. */
    disabled?: boolean;
    icon?: ReactNode;
    label: string;
    menuButtonRef: { current: HTMLElement | null };
    menuLabel: string;
    onClick(): void;
    primaryRef?: Ref<HTMLElement>;
    title?: string;
    widthLabels?: readonly string[];
}

export function SplitButton({ "aria-busy": busy, children, disabled, icon, label, menuButtonRef, menuLabel, onClick, primaryRef, title, widthLabels }: SplitButtonProps) {
    const [open, setOpen] = useState(false);
    useEffect(() => {
        if (!open) return;
        const dismiss = (event: KeyboardEvent) => {
            // An all-disabled menu has no focusable item to receive Escape.
            if (event.key !== "Escape" || event.defaultPrevented) return;
            setOpen(false);
            menuButtonRef.current?.focus();
        };
        window.addEventListener("keydown", dismiss);
        return () => window.removeEventListener("keydown", dismiss);
    }, [open, menuButtonRef]);
    return <div aria-label={label} className="alcomd-split-button" role="group">
        <Button aria-busy={busy} className="alcomd-split-button-primary" disabled={disabled} onClick={onClick} ref={primaryRef} title={title} type="button" widthLabels={widthLabels}>{icon}{label}</Button>
        <Button aria-expanded={open} aria-haspopup="menu" aria-label={menuLabel} className="alcomd-split-button-menu" onClick={() => setOpen(!open)} onKeyDown={(event) => {
            if (event.key === "ArrowDown") { event.preventDefault(); setOpen(true); }
        }} ref={menuButtonRef} type="button">
            <Icon asset={keyboardArrowDownIcon} size={20} />
        </Button>
        <Menu anchorRef={menuButtonRef} className="alcomd-split-button-options" onClose={() => setOpen(false)} open={open}>{children}</Menu>
    </div>;
}
