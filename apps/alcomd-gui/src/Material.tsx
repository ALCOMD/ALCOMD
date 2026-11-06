import { createScrollArea, materialElements, reducedMotion, subscribeMotion } from "@alcomd/ui";
import { refreshIcon, resolveIconGeometry, searchIcon, type IconAsset, type IconSize } from "@alcomd/ui/icons";
import { displayPath } from "./display-path";
import {
    Children,
    createContext,
    createElement,
    forwardRef,
    isValidElement,
    useEffect,
    useLayoutEffect,
    useRef,
    useState,
    useSyncExternalStore,
    type ButtonHTMLAttributes,
    type CSSProperties,
    type FormEvent,
    type FocusEvent as ReactFocusEvent,
    type HTMLAttributes,
    type MouseEvent as ReactMouseEvent,
    type KeyboardEvent as ReactKeyboardEvent,
    type ReactNode,
    type Ref
} from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "./i18n";
import { LocalPageReadContext } from "./PageRead";
import { registerDialogSurface } from "./dialog-surface";

type MaterialElement = HTMLElement & {
    anchorElement?: HTMLElement | null;
    checked?: boolean;
    close?(): void;
    disabled?: boolean;
    indeterminate?: boolean;
    open?: boolean;
    selected?: boolean;
    select?(value: string): void;
    show?(): void;
    updateComplete?: Promise<boolean>;
    value?: number | string;
};

type MaterialProps = HTMLAttributes<HTMLElement> & Record<string, unknown>;
export const DialogOpenContext = createContext<boolean | undefined>(undefined);

export interface DialogOrigin {
    readonly current: HTMLElement | null;
}

export function Icon({ asset, className, size = 24, slot }: { asset: IconAsset; className?: string; size?: IconSize; slot?: string }) {
    const geometry = resolveIconGeometry(asset, size);
    // Material owns slotted sizing; this token only supplies the standalone icon size.
    return createElement(materialElements.icon, {
        "aria-hidden": "true",
        className: ["alcomd-icon", className].filter(Boolean).join(" "),
        "data-filled": asset.filled ? "true" : "false",
        "data-icon-name": asset.name,
        "data-optical-size": size,
        slot,
        style: { "--md-icon-size": size + "px" } as CSSProperties
    }, <svg viewBox={geometry.viewBox} aria-hidden="true" focusable="false"><path d={geometry.path} /></svg>);
}

export interface ButtonProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "ref"> {
    variant?: "filled" | "tonal" | "outlined" | "text";
    /** Material's disabled state that keeps the current keyboard focus. */
    softDisabled?: boolean;
    /** Reserve the widest known label before the first state transition. */
    widthLabels?: readonly string[];
}

export const Button = forwardRef(function Button(
    { "aria-expanded": ariaExpanded, children, className, variant = "filled", widthLabels, ...props }: ButtonProps,
    ref: Ref<HTMLElement>
) {
    const nodes = Children.toArray(children);
    const isSlotted = (node: ReactNode) => isValidElement<{ slot?: string }>(node) && node.props.slot !== undefined;
    const content = widthLabels === undefined ? children : <>
        {nodes.filter(isSlotted)}
        <span className="alcomd-button-label">
            {widthLabels.map((label) => <span aria-hidden="true" className="alcomd-button-label-measure" key={label}>{label}</span>)}
            <span>{nodes.filter((node) => !isSlotted(node))}</span>
        </span>
    </>;
    return createElement(materialElements.button[variant], {
        ...props,
        ariaExpanded,
        className: ["alcomd-button--standard", className].filter(Boolean).join(" "),
        ref
    } as MaterialProps, content);
});

export interface IconButtonProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "ref"> {
    label: string;
}

export function SegmentedButtons({ id, label, options, value, onChange }: {
    id?: string;
    label: string;
    options: readonly { value: string; label: string; disabled?: boolean; title?: string }[];
    value: string;
    onChange(value: string): void;
}) {
    const ref = useRef<HTMLElement>(null);
    const current = useRef(value);
    current.current = value;
    useEffect(() => {
        const element = ref.current;
        if (element === null) return;
        const select = (event: Event) => {
            const detail = (event as CustomEvent<{ index: number; selected: boolean }>).detail;
            const option = options[detail.index];
            if (option !== undefined && !option.disabled && detail.selected && option.value !== current.current) onChange(option.value);
            // The route remains authoritative, including a cancelled dirty-form navigation.
            queueMicrotask(() => {
                Array.from(element.children).forEach((button, index) => {
                    (button as MaterialElement).selected = options[index]?.value === current.current;
                });
            });
        };
        element.addEventListener("segmented-button-set-selection", select);
        return () => element.removeEventListener("segmented-button-set-selection", select);
    }, [onChange, options]);
    return <div className="alcomd-segmented-control">
        {createElement(materialElements.segmentedButtonSet, { ariaLabel: label, id, ref } as MaterialProps,
            options.map((option) => createElement(materialElements.segmentedButton, {
                key: option.value,
                disabled: option.disabled ?? false,
                label: option.label,
                noCheckmark: true,
                selected: option.value === value,
                title: option.title
            } as MaterialProps)))}
    </div>;
}

export function RadioGroup({ disabled, id, label, options, value, onChange }: {
    disabled?: boolean;
    id: string;
    label: string;
    options: readonly { value: string; label: string }[];
    value: string;
    onChange(value: string): void;
}) {
    return <fieldset className="alcomd-radio-group" disabled={disabled} id={id}>
        <legend>{label}</legend>
        <div className="alcomd-radio-options">
            {options.map((option) => <label className="material-toggle" key={option.value}>
                {createElement(materialElements.radio, {
                    ariaLabel: option.label,
                    checked: value === option.value,
                    disabled,
                    id: `${id}-${option.value}`,
                    name: id,
                    value: option.value,
                    onChange: (event: FormEvent<MaterialElement>) => {
                        if (!disabled && event.currentTarget.checked) onChange(option.value);
                    }
                } as MaterialProps)}
                <span>{option.label}</span>
            </label>)}
        </div>
    </fieldset>;
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

export function RefreshButton({ label, refreshing = false, refreshingLabel, disabled, title, type = "button", ...props }: Omit<IconButtonProps, "children" | "label"> & {
    label?: string;
    refreshing?: boolean;
    refreshingLabel?: string;
}) {
    const { t } = useTranslation();
    return <IconButton {...props} aria-busy={refreshing} disabled={disabled || refreshing} label={refreshing ? refreshingLabel ?? t("Refreshing…") : label ?? t("Refresh")} title={title ?? label ?? t("Refresh")} type={type}><Icon asset={refreshIcon} /></IconButton>;
}

export function NavigationList({ children, ...props }: HTMLAttributes<HTMLElement>) {
    return createElement(materialElements.list, props as MaterialProps, children);
}

export interface NavigationListItemProps extends Omit<HTMLAttributes<HTMLElement>, "onClick"> {
    disabled?: boolean;
    onClick?(): void;
    selected?: boolean;
}

export const NavigationListItem = forwardRef(function NavigationListItem(
    { "aria-label": ariaLabel, children, disabled = false, onClick, selected = false, ...props }: NavigationListItemProps,
    ref: Ref<HTMLElement>
) {
    return createElement(materialElements.listItem, {
        ...props,
        ariaLabel,
        ariaSelected: selected ? "true" : "false",
        disabled,
        onClick,
        ref,
        selected,
        type: "button"
    } as MaterialProps, children);
});

export function Menu({ anchorRef, children, className, onClose, open }: { anchorRef: { current: HTMLElement | null }; children: ReactNode; className?: string; onClose(): void; open: boolean }) {
    const quick = useSyncExternalStore(subscribeMotion, reducedMotion);
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
        // A previous closing animation can finish after a new show().
        const close = () => { if (!element.open) onClose(); };
        const closing = () => onClose();
        element.addEventListener("closing", closing);
        element.addEventListener("closed", close);
        return () => { element.removeEventListener("closing", closing); element.removeEventListener("closed", close); };
    }, [onClose]);
    return createElement(materialElements.menu, {
        anchorCorner: "end-end",
        "data-menu-kind": "actions",
        className: ["alcomd-action-menu", className].filter(Boolean).join(" "),
        menuCorner: "start-end",
        positioning: "popover",
        quick,
        ref
    } as MaterialProps, children);
}

export function MenuItem({ className, disabled, label, onClick, title }: { className?: string; disabled?: boolean; label: string; onClick?(): void; title?: string }) {
    const ref = useRef<MaterialElement>(null);
    const activating = useRef(false);
    const activate = (element: MaterialElement) => {
        if (disabled || onClick === undefined || activating.current) return;
        const menu = element.closest("md-menu") as MaterialElement | null;
        if (menu === null) {
            onClick();
            return;
        }
        if (!menu.open) return;
        activating.current = true;
        menu.addEventListener("closed", () => {
            activating.current = false;
            if (element.isConnected && !element.disabled) onClick();
        }, { once: true });
        menu.close?.();
    };
    useEffect(() => {
        const element = ref.current;
        if (element === null) return;
        const select = (event: Event) => {
            const detail = (event as CustomEvent<{ initiator?: EventTarget; reason?: { kind?: string; key?: string } }>).detail;
            if (detail?.initiator !== element) return;
            const reason = detail.reason;
            if (reason?.kind === "click-selection" || (reason?.kind === "keydown" && (reason.key === "Enter" || reason.key === "Space"))) activate(element);
        };
        // Material prevents the native Enter/Space click and publishes this event instead.
        element.addEventListener("close-menu", select);
        return () => element.removeEventListener("close-menu", select);
    }, [disabled, onClick]);
    return createElement(materialElements.menuItem, {
        "aria-disabled": disabled ? "true" : undefined,
        className,
        disabled,
        onClick: (event: ReactMouseEvent<MaterialElement>) => activate(event.currentTarget),
        ref,
        title,
        type: "button"
    } as MaterialProps, label);
}

// Material Web 2.5.0 menus only support menuitem/option/button/link, not a form
// of independently focusable filters. Use a non-modal top-layer container; all
// interactive children remain real Material controls.
export function FilterPopover({ anchorRef, children, onClose, open, label }: {
    anchorRef: { current: HTMLElement | null };
    children: ReactNode;
    onClose(): void;
    open: boolean;
    label: string;
}) {
    const ref = useRef<HTMLDivElement>(null);
    useEffect(() => {
        if (ref.current === null) return;
        const instance = createScrollArea(ref.current);
        return () => instance.destroy();
    }, []);
    useEffect(() => {
        const panel = ref.current;
        const anchor = anchorRef.current;
        if (panel === null || anchor === null) return;
        if (!open) { panel.hidePopover(); return; }
        panel.showPopover();
        const place = () => {
            const bounds = anchor.getBoundingClientRect();
            panel.style.left = `${Math.max(8, Math.min(bounds.right - panel.offsetWidth, window.innerWidth - panel.offsetWidth - 8))}px`;
            panel.style.top = `${Math.max(8, Math.min(bounds.bottom + 4, window.innerHeight - panel.offsetHeight - 8))}px`;
        };
        place();
        panel.focus();
        const observer = new ResizeObserver(place);
        observer.observe(panel);
        window.addEventListener("resize", place);
        return () => { observer.disconnect(); window.removeEventListener("resize", place); };
    }, [anchorRef, open]);
    return createPortal(<div
        aria-label={label}
        className="alcomd-filter-popover"
        onToggle={(event) => {
            if ((event.nativeEvent as ToggleEvent).newState === "closed") {
                onClose();
                if (document.activeElement === document.body || event.currentTarget.contains(document.activeElement)) anchorRef.current?.focus();
            }
        }}
        popover="auto"
        ref={ref}
        role="dialog"
        tabIndex={-1}
    >{children}</div>, document.body);
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
    onBlur?(): void;
    onFocus?(event: ReactFocusEvent<HTMLElement>): void;
    onKeyDown?(event: ReactKeyboardEvent<HTMLElement>): void;
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
    onBlur,
    onFocus,
    onKeyDown,
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
        onBlur: () => onBlur?.(),
        onFocus,
        onKeyDown,
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

/** Shows supplied paths readably while preserving untouched and manually entered values. */
export function PathField({ value, onBlur, onInput, ...props }: Omit<TextFieldProps, "value" | "type"> & { value: string }) {
    const [lastInput, setLastInput] = useState<string>();
    const visibleValue = value === lastInput ? value : displayPath(value);
    return <TextField {...props} value={visibleValue} onBlur={() => {
        setLastInput(undefined);
        onBlur?.();
    }} onInput={(next) => {
        if (next === visibleValue) return;
        setLastInput(next);
        onInput?.(next);
    }} />;
}

export function Slider({ id, label, min, max, value, onInput, disabled, labeled = false }: {
    id: string;
    label: string;
    min: number;
    max: number;
    value: number;
    onInput(value: number): void;
    disabled?: boolean;
    labeled?: boolean;
}) {
    return createElement(materialElements.slider, {
        id,
        ariaLabel: label,
        labeled,
        min,
        max,
        value,
        disabled,
        onInput: (event: FormEvent<MaterialElement>) => onInput(Number(event.currentTarget.value))
    } as MaterialProps);
}

// Material Web has no color picker. This approved adapter opens the browser's
// color dialog; pages use the shared control rather than native form inputs.
export function ColorPicker({ id, label, value, onInput, disabled }: {
    id: string;
    label: string;
    value: string;
    onInput(value: string): void;
    disabled?: boolean;
}) {
    return createElement("input", {
        type: "color",
        id,
        "aria-label": label,
        className: "alcomd-color-picker",
        value,
        disabled,
        onInput: (event: FormEvent<HTMLInputElement>) => onInput(event.currentTarget.value)
    });
}

export function SearchField({ className, label, ...props }: Omit<TextFieldProps, "type" | "variant" | "leadingIcon" | "aria-label">) {
    return <TextField {...props} aria-label={label} className={["alcomd-search-field", className].filter(Boolean).join(" ")} label="" type="search" variant="filled" leadingIcon={<Icon asset={searchIcon} slot="leading-icon" />} />;
}

export interface SelectOption {
    disabled?: boolean;
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
    onOpen?(): void;
    onClose?(): void;
    /** Review a choice after closing, while retaining the authoritative value. */
    onSelect?(value: string, origin: DialogOrigin): void;
    options: readonly SelectOption[];
    required?: boolean;
    supportingText?: string;
    value: string;
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
    onOpen,
    onClose,
    onSelect,
    options,
    required,
    supportingText,
    value
}: SelectProps) {
    const reduce = useSyncExternalStore(subscribeMotion, reducedMotion);
    const ref = useRef<MaterialElement>(null);
    const open = useRef(false);
    const pendingSelection = useRef<string | undefined>(undefined);
    const commitSelection = () => {
        const element = ref.current;
        const selected = pendingSelection.current;
        pendingSelection.current = undefined;
        if (element === null || selected === undefined) return;
        element.select?.(value);
        if (!disabled && options.some((option) => option.value === selected && !option.disabled)) onSelect?.(selected, ref);
    };
    useEffect(() => {
        const element = ref.current;
        if (element === null) return;
        const opening = (event: Event) => {
            if (event.target !== element) return;
            open.current = true;
            const bounds = element.getBoundingClientRect();
            const below = window.innerHeight - bounds.bottom - 8;
            // Reserve one standard 56px option and the menu's two 8px insets.
            const available = below >= 72 ? below : bounds.top - 8;
            element.style.setProperty("--alcomd-select-menu-max-height", `${Math.max(72, available)}px`);
            onOpen?.();
        };
        const closed = (event: Event) => {
            if (event.target !== element) return;
            open.current = false;
            onClose?.();
            commitSelection();
        };
        element.addEventListener("opening", opening);
        element.addEventListener("closed", closed);
        return () => { element.removeEventListener("opening", opening); element.removeEventListener("closed", closed); };
    });
    const selectedLabel = options.find((option) => option.value === value)?.label;
    useEffect(() => {
        const element = ref.current;
        if (element === null) return;
        let active = true;
        // Option text changes do not invalidate Material's selected-text cache.
        void Promise.resolve(element.updateComplete).then(() => {
            if (!active) return;
            // Material 2.5 exposes no text-layout token/part for this label.
            // Give its slotted text a shrinkable width so long values keep the arrow visible.
            const root = element.shadowRoot;
            if (root && !root.querySelector("style[data-alcomd-select-label]")) {
                const style = element.ownerDocument.createElement("style");
                style.dataset.alcomdSelectLabel = "";
                style.textContent = ":host(.alcomd-select--constrained) #label { width: 0; flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }";
                root.append(style);
            }
            element.select?.(value);
        });
        return () => { active = false; };
    }, [selectedLabel, value]);
    return createElement(
        materialElements.select.filled,
        {
            ariaDescribedBy,
            ariaInvalid,
            ariaLabel: ariaLabel ?? label,
            className: ["alcomd-select", className].filter(Boolean).join(" "),
            disabled,
            id,
            label: "",
            onChange: (event: FormEvent<MaterialElement>) => {
                const selected = event.currentTarget.value as string;
                if (disabled || !options.some((option) => option.value === selected && !option.disabled)) return;
                if (onSelect) {
                    pendingSelection.current = selected;
                    if (!open.current) commitSelection();
                } else onChange?.(selected);
            },
            quick: reduce,
            ref,
            required,
            supportingText,
            value
        } as MaterialProps,
        options.map((option) => createElement(materialElements.selectOption, {
            disabled: option.disabled,
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
    const ref = useRef<MaterialElement>(null);
    const observed = useRef(selected);
    const instance = useRef(0);
    const restoreFocus = useRef(false);
    const [, recordInput] = useState(0);
    // Material 2.5 binds a checked attribute, so changing its public selected
    // property cannot reset a user-dirtied native input. A controlled reset uses
    // a fresh real Material instance; ordinary user draft changes keep theirs.
    if (!disabled && observed.current !== selected) {
        observed.current = selected;
        instance.current += 1;
    }
    useEffect(() => {
        const element = ref.current;
        if (disabled || element === null || !restoreFocus.current) return;
        restoreFocus.current = false;
        let active = true;
        void Promise.resolve(element.updateComplete).then(() => {
            if (active && element.isConnected && !element.disabled
                && (document.activeElement === document.body || element.matches(":focus-within"))) element.focus();
        });
        return () => { active = false; };
    });
    return (
        <label className="material-toggle">
            {createElement(materialElements.switch, {
                ariaDescribedBy,
                ariaInvalid,
                ariaLabel: label,
                ariaReadOnly,
                disabled,
                key: instance.current,
                onChange: (event: FormEvent<MaterialElement>) => {
                    observed.current = Boolean(event.currentTarget.selected);
                    restoreFocus.current = event.currentTarget.matches(":focus-within");
                    recordInput((version) => version + 1);
                    onChange?.(observed.current);
                },
                ref,
                selected
            } as MaterialProps)}
            <span>{label}</span>
        </label>
    );
}

export function Checkbox({ checked, disabled, label, labelHidden = false, onChange }: { checked: boolean; disabled?: boolean; label: string; labelHidden?: boolean; onChange?(checked: boolean): void }) {
    const ref = useRef<MaterialElement>(null);
    // A Material checkbox changes its own property before dispatching change.
    // Reconcile even when the owner rejects the change (e.g. confirmation cancelled).
    useEffect(() => { if (ref.current !== null) ref.current.checked = checked; });
    return (
        <label className="material-toggle">
            {createElement(materialElements.checkbox, {
                ariaLabel: label,
                checked,
                disabled,
                ref,
                onChange: (event: FormEvent<MaterialElement>) => onChange?.(Boolean(event.currentTarget.checked))
            } as MaterialProps)}
            {labelHidden ? null : <span>{label}</span>}
        </label>
    );
}

export function Dialog({ actions, children, onClose, open, title, wide = false, dismissible = true, returnFocus, returnFocusReady = true }: { actions?: ReactNode; children: ReactNode; onClose(): void; open: boolean; title: string; wide?: boolean; dismissible?: boolean; returnFocus?: DialogOrigin; returnFocusReady?: boolean }) {
    const quick = useSyncExternalStore(subscribeMotion, reducedMotion);
    const ref = useRef<MaterialElement>(null);
    const contentRef = useRef<HTMLDivElement>(null);
    useLayoutEffect(() => {
        const element = ref.current;
        if (element === null) return;
        let release: (() => void) | undefined;
        const opened = () => {
            if (release === undefined && contentRef.current !== null) release = registerDialogSurface(contentRef.current);
        };
        const closed = () => { release?.(); release = undefined; };
        element.addEventListener("opened", opened);
        element.addEventListener("closed", closed);
        return () => { closed(); element.removeEventListener("opened", opened); element.removeEventListener("closed", closed); };
    }, []);
    const [restoreFocus, setRestoreFocus] = useState(false);
    useEffect(() => {
        const element = ref.current;
        if (element === null) return;
        const close = () => {
            if (returnFocus !== undefined) setRestoreFocus(true);
            onClose();
        };
        const cancel = (event: Event) => { if (!dismissible) event.preventDefault(); };
        element.addEventListener("closed", close);
        element.addEventListener("cancel", cancel);
        return () => { element.removeEventListener("closed", close); element.removeEventListener("cancel", cancel); };
    }, [onClose, dismissible, returnFocus]);
    useEffect(() => {
        if (open) {
            if (restoreFocus) setRestoreFocus(false);
            return;
        }
        if (!restoreFocus || !returnFocusReady) return;
        const target = returnFocus?.current as MaterialElement | null | undefined;
        if (target == null) {
            setRestoreFocus(false);
            return;
        }
        let active = true;
        // Owner state and Material's native button must both finish enabling first.
        void Promise.resolve(target.updateComplete).then(() => {
            if (!active) return;
            if (target.isConnected && !target.disabled) target.focus();
            setRestoreFocus(false);
        });
        return () => { active = false; };
    }, [open, restoreFocus, returnFocus, returnFocusReady]);
    return createPortal(
        createElement(
            materialElements.dialog,
            { ariaLabel: title, open, quick, ref, style: wide ? { maxWidth: "min(848px, calc(100vw - 48px))" } : undefined } as MaterialProps,
            createElement("div", { slot: "headline" }, title),
            createElement("div", { ref: contentRef, className: wide ? "material-dialog-content material-dialog-content--wide" : "material-dialog-content", slot: "content" }, <DialogOpenContext.Provider value={open}><LocalPageReadContext.Provider value={true}>{children}</LocalPageReadContext.Provider></DialogOpenContext.Provider>),
            actions === undefined ? null : createElement("div", { className: "material-dialog-actions", slot: "actions" }, actions)
        ),
        document.body
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

export function ButtonProgress({ slot }: { slot: "icon" }) {
    return <span aria-hidden="true" className="alcomd-button-progress" slot={slot}>
        {createElement(materialElements.circularProgress, { indeterminate: true })}
    </span>;
}
