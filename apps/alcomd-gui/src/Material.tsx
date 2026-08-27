import { materialElements } from "@alcomd/ui";
import { resolveIconUrl, type IconAsset, type IconSize } from "@alcomd/ui/icons";
import {
    createElement,
    forwardRef,
    useEffect,
    useRef,
    type ButtonHTMLAttributes,
    type CSSProperties,
    type FormEvent,
    type HTMLAttributes,
    type ReactNode,
    type Ref
} from "react";

type MaterialElement = HTMLElement & {
    anchorElement?: HTMLElement | null;
    checked?: boolean;
    close?(): void;
    disabled?: boolean;
    indeterminate?: boolean;
    open?: boolean;
    selected?: boolean;
    show?(): void;
    value?: number | string;
};

type MaterialProps = HTMLAttributes<HTMLElement> & Record<string, unknown>;

export function Icon({ asset, className, size = 24, slot }: { asset: IconAsset; className?: string; size?: IconSize; slot?: string }) {
    const style = {
        "--alcomd-icon-size": `${size}px`,
        "--alcomd-icon-url": `url("${resolveIconUrl(asset, size)}")`
    } as CSSProperties;
    return (
        <span
            aria-hidden="true"
            className={["alcomd-icon", className].filter(Boolean).join(" ")}
            data-filled={asset.filled ? "true" : "false"}
            data-icon-name={asset.name}
            data-optical-size={size}
            slot={slot}
            style={style}
        />
    );
}

export interface ButtonProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "ref"> {
    variant?: "filled" | "tonal" | "outlined" | "text";
}

export const Button = forwardRef(function Button(
    { "aria-expanded": ariaExpanded, children, className, variant = "filled", ...props }: ButtonProps,
    ref: Ref<HTMLElement>
) {
    return createElement(materialElements.button[variant], {
        ...props,
        ariaExpanded,
        className: ["alcomd-button--standard", className].filter(Boolean).join(" "),
        ref
    } as MaterialProps, children);
});

export interface IconButtonProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "ref"> {
    label: string;
}

export const IconButton = forwardRef(function IconButton(
    { "aria-controls": ariaControls, "aria-expanded": ariaExpanded, children, className, label, ...props }: IconButtonProps,
    ref: Ref<HTMLElement>
) {
    return createElement(materialElements.iconButton, {
        ...props,
        ariaControls,
        ariaExpanded,
        ariaLabel: label,
        className: ["alcomd-icon-button--standard", className].filter(Boolean).join(" "),
        ref
    } as MaterialProps, children);
});

export function NavigationList({ children, ...props }: HTMLAttributes<HTMLElement>) {
    return createElement(materialElements.list, props as MaterialProps, children);
}

export interface NavigationListItemProps extends Omit<HTMLAttributes<HTMLElement>, "onClick"> {
    disabled?: boolean;
    onClick?(): void;
    selected?: boolean;
}

export const NavigationListItem = forwardRef(function NavigationListItem(
    { children, disabled = false, onClick, selected = false, ...props }: NavigationListItemProps,
    ref: Ref<HTMLElement>
) {
    return createElement(materialElements.listItem, {
        ...props,
        ariaSelected: selected ? "true" : "false",
        disabled,
        onClick,
        ref,
        selected,
        type: "button"
    } as MaterialProps, children);
});

export function Menu({ anchorRef, children, className, onClose, open }: { anchorRef: { current: HTMLElement | null }; children: ReactNode; className?: string; onClose(): void; open: boolean }) {
    const ref = useRef<MaterialElement>(null);
    useEffect(() => {
        const element = ref.current;
        if (element === null) return;
        element.anchorElement = anchorRef.current;
        if (open && !element.open) element.show?.();
        if (!open && element.open) element.close?.();
    }, [anchorRef, open]);
    useEffect(() => {
        const element = ref.current;
        if (element === null) return;
        const close = () => onClose();
        element.addEventListener("closed", close);
        return () => element.removeEventListener("closed", close);
    }, [onClose]);
    return createElement(materialElements.menu, {
        anchorCorner: "end-end",
        className,
        menuCorner: "start-end",
        positioning: "popover",
        ref
    } as MaterialProps, children);
}

export function MenuItem({ className, disabled, label, onClick, title }: { className?: string; disabled?: boolean; label: string; onClick?(): void; title?: string }) {
    return createElement(materialElements.menuItem, {
        className,
        disabled,
        onClick,
        title,
        type: "button"
    } as MaterialProps, label);
}

export interface TextFieldProps {
    "aria-label"?: string;
    "aria-describedby"?: string;
    "aria-invalid"?: boolean;
    className?: string;
    disabled?: boolean;
    error?: boolean;
    errorText?: string;
    id?: string;
    label: string;
    leadingIcon?: ReactNode;
    max?: number;
    maxLength?: number;
    min?: number;
    minLength?: number;
    onInput?(value: string): void;
    placeholder?: string;
    readOnly?: boolean;
    required?: boolean;
    rows?: number;
    supportingText?: string;
    type?: "text" | "number" | "password" | "search" | "textarea" | "url";
    value: number | string;
    variant?: "filled" | "outlined";
}

export function TextField({
    "aria-label": ariaLabel,
    "aria-describedby": ariaDescribedBy,
    "aria-invalid": ariaInvalid,
    className,
    disabled,
    error,
    errorText,
    id,
    label,
    leadingIcon,
    max,
    maxLength,
    min,
    minLength,
    onInput,
    placeholder,
    readOnly,
    required,
    rows,
    supportingText,
    type = "text",
    value,
    variant = "outlined"
}: TextFieldProps) {
    return createElement(materialElements.textField[variant], {
        ariaLabel,
        ariaDescribedBy,
        ariaInvalid,
        className,
        disabled,
        error,
        errorText,
        id,
        label,
        max,
        maxLength,
        min,
        minLength,
        onInput: (event: FormEvent<MaterialElement>) => onInput?.(event.currentTarget.value as string),
        placeholder,
        required,
        readOnly,
        rows,
        supportingText,
        type,
        value
    } as MaterialProps, leadingIcon);
}

export interface SelectOption {
    label: string;
    value: string;
}

export interface SelectProps {
    "aria-describedby"?: string;
    "aria-invalid"?: boolean;
    "aria-label"?: string;
    className?: string;
    disabled?: boolean;
    id?: string;
    label: string;
    onChange?(value: string): void;
    options: readonly SelectOption[];
    required?: boolean;
    supportingText?: string;
    value: string;
    variant?: "filled" | "outlined";
}

export function Select({
    "aria-describedby": ariaDescribedBy,
    "aria-invalid": ariaInvalid,
    "aria-label": ariaLabel,
    className,
    disabled,
    id,
    label,
    onChange,
    options,
    required,
    supportingText,
    value,
    variant = "outlined"
}: SelectProps) {
    return createElement(
        materialElements.select[variant],
        {
            ariaDescribedBy,
            ariaInvalid,
            ariaLabel,
            className,
            disabled,
            id,
            label,
            onChange: (event: FormEvent<MaterialElement>) => onChange?.(event.currentTarget.value as string),
            required,
            supportingText,
            value
        } as MaterialProps,
        options.map((option) => createElement(materialElements.selectOption, {
            key: option.value,
            selected: option.value === value,
            value: option.value
        } as MaterialProps, option.label))
    );
}

export function Switch({
    "aria-describedby": ariaDescribedBy,
    "aria-invalid": ariaInvalid,
    "aria-readonly": ariaReadOnly,
    disabled,
    label,
    onChange,
    selected
}: {
    "aria-describedby"?: string;
    "aria-invalid"?: boolean;
    "aria-readonly"?: boolean;
    disabled?: boolean;
    label: string;
    onChange?(selected: boolean): void;
    selected: boolean;
}) {
    return (
        <label className="material-toggle">
            {createElement(materialElements.switch, {
                ariaDescribedBy,
                ariaInvalid,
                ariaLabel: label,
                ariaReadOnly,
                disabled,
                onChange: (event: FormEvent<MaterialElement>) => onChange?.(Boolean(event.currentTarget.selected)),
                selected
            } as MaterialProps)}
            <span>{label}</span>
        </label>
    );
}

export function Checkbox({ checked, disabled, label, onChange }: { checked: boolean; disabled?: boolean; label: string; onChange?(checked: boolean): void }) {
    return (
        <label className="material-toggle">
            {createElement(materialElements.checkbox, {
                ariaLabel: label,
                checked,
                disabled,
                onChange: (event: FormEvent<MaterialElement>) => onChange?.(Boolean(event.currentTarget.checked))
            } as MaterialProps)}
            <span>{label}</span>
        </label>
    );
}

export function Dialog({ children, onClose, open, title }: { children: ReactNode; onClose(): void; open: boolean; title: string }) {
    const ref = useRef<MaterialElement>(null);
    useEffect(() => {
        const element = ref.current;
        if (element === null) return;
        const close = () => onClose();
        element.addEventListener("closed", close);
        return () => element.removeEventListener("closed", close);
    }, [onClose]);
    return createElement(
        materialElements.dialog,
        { open, ref } as MaterialProps,
        createElement("div", { slot: "headline" }, title),
        createElement("div", { slot: "content" }, children)
    );
}

export function Progress({ label, max = 1, value }: { label: string; max?: number; value?: number }) {
    return createElement(materialElements.progress, {
        "aria-label": label,
        indeterminate: value === undefined,
        max,
        value: value ?? 0
    } as MaterialProps);
}
