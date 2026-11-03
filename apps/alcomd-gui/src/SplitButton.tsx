import { keyboardArrowDownIcon } from "@alcomd/ui/icons";
import { useState, type ReactNode, type Ref } from "react";

import { Button, Icon, Menu } from "./Material";

export interface SplitButtonProps {
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
}

export function SplitButton({ children, disabled, icon, label, menuButtonRef, menuLabel, onClick, primaryRef, title }: SplitButtonProps) {
    const [open, setOpen] = useState(false);
    return <div aria-label={label} className="alcomd-split-button" role="group">
        <Button className="alcomd-split-button-primary" disabled={disabled} onClick={onClick} ref={primaryRef} title={title} type="button">{icon}{label}</Button>
        <Button aria-expanded={open} aria-haspopup="menu" aria-label={menuLabel} className="alcomd-split-button-menu" onClick={() => setOpen(!open)} onKeyDown={(event) => {
            if (event.key === "ArrowDown") { event.preventDefault(); setOpen(true); }
        }} ref={menuButtonRef} type="button">
            <Icon asset={keyboardArrowDownIcon} size={20} />
        </Button>
        <Menu anchorRef={menuButtonRef} className="alcomd-split-button-options" onClose={() => setOpen(false)} open={open}>{children}</Menu>
    </div>;
}
