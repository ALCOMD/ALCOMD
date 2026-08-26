import { materialElements } from "@alcomd/ui";
import {
    createElement,
    forwardRef,
    useEffect,
    useRef,
    type ButtonHTMLAttributes,
    type ChangeEvent,
    type FormEvent,
    type HTMLAttributes,
    type ReactNode,
    type Ref
} from "react";

type MaterialElement = HTMLElement & {
    checked?: boolean;
    disabled?: boolean;
    indeterminate?: boolean;
    open?: boolean;
    selected?: boolean;
    value?: number | string;
};

type MaterialProps = HTMLAttributes<HTMLElement> & Record<string, unknown>;

export interface ButtonProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "ref"> {
    variant?: "filled" | "tonal" | "text";
}

export const Button = forwardRef(function Button(
    { "aria-expanded": ariaExpanded, children, variant = "filled", ...props }: ButtonProps,
    ref: Ref<HTMLElement>
) {
    return createElement(materialElements.button[variant], { ...props, ariaExpanded, ref } as MaterialProps, children);
});

export interface IconButtonProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "ref"> {
    label: string;
}

export const IconButton = forwardRef(function IconButton(
    { children, label, ...props }: IconButtonProps,
    ref: Ref<HTMLElement>
) {
    return createElement(materialElements.iconButton, {
        ...props,
        "aria-label": label,
        ref
    } as MaterialProps, children);
});

export interface TextFieldProps {
    className?: string;
    disabled?: boolean;
    label: string;
    maxLength?: number;
    onInput?(value: string): void;
    required?: boolean;
    supportingText?: string;
    type?: "text" | "number" | "password" | "url";
    value: string;
}

export function TextField({ className, disabled, label, maxLength, onInput, required, supportingText, type = "text", value }: TextFieldProps) {
    return createElement(materialElements.textField, {
        className,
        disabled,
        label,
        maxLength,
        onInput: (event: FormEvent<MaterialElement>) => onInput?.(event.currentTarget.value as string),
        required,
        supportingText,
        type,
        value
    } as MaterialProps);
}

export interface SelectOption {
    label: string;
    value: string;
}

export interface SelectProps {
    className?: string;
    disabled?: boolean;
    label: string;
    onChange?(value: string): void;
    options: readonly SelectOption[];
    value: string;
}

export function Select({ className, disabled, label, onChange, options, value }: SelectProps) {
    return createElement(
        materialElements.select,
        {
            className,
            disabled,
            label,
            onChange: (event: ChangeEvent<MaterialElement>) => onChange?.(event.currentTarget.value as string),
            value
        } as MaterialProps,
        options.map((option) => createElement(materialElements.selectOption, {
            key: option.value,
            selected: option.value === value,
            value: option.value
        } as MaterialProps, option.label))
    );
}

export function Switch({ disabled, label, onChange, selected }: { disabled?: boolean; label: string; onChange?(selected: boolean): void; selected: boolean }) {
    return (
        <label className="material-toggle">
            {createElement(materialElements.switch, {
                disabled,
                onChange: (event: ChangeEvent<MaterialElement>) => onChange?.(Boolean(event.currentTarget.selected)),
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
                checked,
                disabled,
                onChange: (event: ChangeEvent<MaterialElement>) => onChange?.(Boolean(event.currentTarget.checked))
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

export function Progress({ label, value }: { label: string; value?: number }) {
    return createElement(materialElements.progress, {
        "aria-label": label,
        indeterminate: value === undefined,
        value: value ?? 0
    } as MaterialProps);
}
