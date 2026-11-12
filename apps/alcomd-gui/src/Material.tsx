import { createScrollArea, materialElements, reducedMotion, subscribeMotion } from "@alcomd/ui";
import { refreshIcon, resolveIconGeometry, searchIcon, type IconAsset, type IconSize } from "@alcomd/ui/icons";
import { displayPath } from "./display-path";
import {
    Children,
    createContext,
    createElement,
    forwardRef,
    isValidElement,
    useCallback,
    useContext,
    useEffect,
    useEffectEvent,
    useId,
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
import { usePreloadIntent, useRoutePreload } from "./Preload";
import { registerDialogSurface } from "./dialog-surface";

type MaterialElement = HTMLElement & {
    anchorElement?: HTMLElement | null;
    checked?: boolean;
    close?(): void;
    disabled?: boolean;
    displayText?: string;
    indeterminate?: boolean;
    open?: boolean;
    selected?: boolean;
    select?(value: string): void;
    show?(): void;
    updateComplete?: Promise<boolean>;
    value?: number | string;
};

type MaterialSelectElement = MaterialElement & {
    addController(controller: { hostUpdated(): void }): void;
    removeController(controller: { hostUpdated(): void }): void;
    showPicker(): void;
};

type MaterialProps = HTMLAttributes<HTMLElement> & Record<string, unknown>;
export const DialogOpenContext = createContext<boolean | undefined>(undefined);
const DialogCloseContext = createContext<{
    requestClose(afterClose?: () => void): void;
} | undefined>(undefined);

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

export function SegmentedButtons({ id, label, options, value, onChange, preload = false }: {
    id?: string;
    label: string;
    options: readonly { value: string; label: string; disabled?: boolean; title?: string }[];
    value: string;
    onChange(value: string): void;
    preload?: boolean;
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
            options.map((option) => <IntentSegment key={option.value} option={option} selected={option.value === value} preload={preload} />))}
    </div>;
}

function IntentSegment({ option, selected, preload }: { option: { value: string; label: string; disabled?: boolean; title?: string }; selected: boolean; preload: boolean }) {
    const { pin: _pin, prepare: _prepare, ...intent } = useRoutePreload(option.value, preload && !selected && !option.disabled);
    return createElement(materialElements.segmentedButton, { ...intent, disabled: option.disabled ?? false, label: option.label, noCheckmark: true, selected, title: option.title } as MaterialProps);
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

const ACTION_FEEDBACK_MIN_MS = 300;

type FeedbackClick = (event: ReactMouseEvent<HTMLButtonElement>) => void | Promise<unknown>;

export function useActionFeedback(loading: boolean) {
    const [active, setActive] = useState(false);
    const pending = useRef(false);
    const mounted = useRef(true);
    const timer = useRef<ReturnType<typeof setTimeout>>(undefined);
    useEffect(() => {
        mounted.current = true;
        return () => {
            mounted.current = false;
            if (timer.current !== undefined) clearTimeout(timer.current);
        };
    }, []);
    const run = (action: () => void | Promise<unknown>) => {
        if (loading || pending.current) return;
        pending.current = true;
        setActive(true);
        const started = performance.now();
        const finish = () => {
            if (!mounted.current) return;
            timer.current = setTimeout(() => {
                pending.current = false;
                setActive(false);
            }, Math.max(0, ACTION_FEEDBACK_MIN_MS - (performance.now() - started)));
        };
        try {
            return Promise.resolve(action()).finally(finish);
        } catch (error) {
            finish();
            throw error;
        }
    };
    return { busy: loading || active, run };
}

/** Await an explicit action while keeping fast completion perceptible and the label width fixed. */
export function LoadingButton({ children, className, disabled, loading = false, onClick, softDisabled, ...props }: Omit<ButtonProps, "onClick"> & {
    loading?: boolean;
    onClick?: FeedbackClick;
}) {
    const feedback = useActionFeedback(loading);
    return <Button {...props} className={[className, feedback.busy && "alcomd-loading-button--busy"].filter(Boolean).join(" ")} aria-busy={feedback.busy ? "true" : "false"} disabled={disabled || feedback.busy} softDisabled={softDisabled} onClick={(event) => {
        if (!disabled && !softDisabled && !feedback.busy && onClick !== undefined) return feedback.run(() => onClick(event));
    }}>
        <span className="alcomd-loading-button-content">
            <span style={{ opacity: feedback.busy ? 0 : 1 }}>{children}</span>
            {feedback.busy ? <ButtonProgress /> : null}
        </span>
    </Button>;
}

export function RefreshButton({ className, label, refreshing = false, refreshingLabel, disabled, title, type = "button", onClick, ...props }: Omit<IconButtonProps, "children" | "label" | "onClick"> & {
    label?: string;
    refreshing?: boolean;
    refreshingLabel?: string;
    onClick?: FeedbackClick;
}) {
    const { t } = useTranslation();
    const feedback = useActionFeedback(refreshing);
    return <IconButton {...props} className={[className, feedback.busy && "alcomd-refresh-button--busy"].filter(Boolean).join(" ")} aria-busy={feedback.busy ? "true" : "false"} disabled={disabled || feedback.busy} label={feedback.busy ? refreshingLabel ?? t("Refreshing…") : label ?? t("Refresh")} title={title ?? label ?? t("Refresh")} type={type} onClick={(event) => {
        if (!disabled && !feedback.busy && onClick !== undefined) return feedback.run(() => onClick(event));
    }}>{feedback.busy ? <ButtonProgress /> : <Icon asset={refreshIcon} />}</IconButton>;
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
    kind?: "heading" | "separator" | "status";
    title?: string;
    disabled?: boolean;
    /** Public Material option text used by the trigger, independently of its menu label. */
    displayText?: string;
    label: string;
    value: string;
}

export interface SelectProps {
    "aria-describedby"?: string;
    "aria-invalid"?: boolean;
    "aria-label"?: string;
    className?: string;
    disabled?: boolean;
    displayText?: string;
    /** Prepare async options on intent; opening waits at most 100ms. */
    prepareOpen?(intent?: "prefetch" | "open"): Promise<void>;
    preloadIdentity?: string;
    discardPrepared?(): void;
    preparedWeight?(): number;
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
    displayText,
    prepareOpen,
    preloadIdentity,
    discardPrepared,
    preparedWeight,
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
    const ref = useRef<MaterialSelectElement>(null);
    const open = useRef(false);
    const preloadId = useId();
    const preload = usePreloadIntent({ key: `select:${preloadId}:${preloadIdentity ?? ""}`, enabled: Boolean(prepareOpen) && !disabled,
        load: () => prepareOpen?.("prefetch") ?? Promise.resolve(), discard: () => discardPrepared?.(), weight: preparedWeight });
    const pendingOpen = useRef<object | undefined>(undefined);
    const replayingOpen = useRef(false);
    const cancelOpen = () => { pendingOpen.current = undefined; preload.pin(false); onClose?.(); };
    useEffect(() => () => { pendingOpen.current = undefined; }, []);
    useEffect(() => { if (disabled) pendingOpen.current = undefined; }, [disabled]);
    const requestOpen = async (activate: () => void) => {
        if (!prepareOpen || disabled || pendingOpen.current) return;
        const request = {};
        pendingOpen.current = request;
        preload.pin(true);
        let timer: number | undefined;
        try {
            await Promise.race([
                preload.prepare().then(() => {
                    if (pendingOpen.current === request) return prepareOpen("open");
                }).catch(() => undefined),
                new Promise<void>((resolve) => { timer = window.setTimeout(resolve, 100); })
            ]);
            await ref.current?.updateComplete;
            if (pendingOpen.current !== request || !ref.current?.isConnected || ref.current.disabled) return;
            pendingOpen.current = undefined;
            replayingOpen.current = true;
            try { activate(); } finally { replayingOpen.current = false; }
        } finally { window.clearTimeout(timer); }
    };
    const pendingSelection = useRef<string | undefined>(undefined);
    const commitSelection = () => {
        const element = ref.current;
        const selected = pendingSelection.current;
        pendingSelection.current = undefined;
        if (element === null || selected === undefined) return;
        element.select?.(value);
        if (displayText !== undefined) element.displayText = displayText;
        if (!disabled && options.some((option) => !option.kind && option.value === selected && !option.disabled)) onSelect?.(selected, ref);
    };
    useEffect(() => {
        const element = ref.current;
        if (element === null) return;
        const opening = (event: Event) => {
            if (event.target !== element) return;
            open.current = true;
            preload.pin(true);
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
            preload.pin(false);
            onClose?.();
            commitSelection();
        };
        element.addEventListener("opening", opening);
        element.addEventListener("closed", closed);
        return () => { element.removeEventListener("opening", opening); element.removeEventListener("closed", closed); };
    });
    const selectedOption = options.find((option) => option.value === value);
    const selectedLabel = displayText ?? selectedOption?.displayText ?? selectedOption?.label;
    useLayoutEffect(() => {
        const element = ref.current;
        if (element === null || displayText === undefined) return;
        // Async option lists can be empty even though the caller already knows
        // the current value. Keep that explicit text controlled across Material's
        // initialization and option updates, before the browser paints.
        const synchronize = () => {
            if (element.displayText !== displayText) element.displayText = displayText;
        };
        const controller = { hostUpdated: synchronize };
        element.addController(controller);
        synchronize();
        return () => element.removeController(controller);
    }, [displayText]);
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
            if (displayText !== undefined) element.displayText = displayText;
        });
        return () => { active = false; };
    }, [selectedLabel, value, displayText]);
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
            onPointerEnter: preload.onPointerEnter,
            onPointerDown: preload.onPointerDown,
            onPointerCancel: preload.onPointerCancel,
            onPointerLeave: () => { preload.onPointerLeave(); if (prepareOpen && !open.current && !pendingOpen.current) onClose?.(); },
            onFocusCapture: (event: ReactFocusEvent<MaterialElement>) => {
                if (prepareOpen && !disabled && event.target === event.currentTarget) preload.onFocusCapture();
            },
            onBlurCapture: preload.onBlurCapture,
            onBlur: (event: ReactFocusEvent<MaterialElement>) => {
                if (pendingOpen.current && !event.currentTarget.contains(event.relatedTarget as Node | null)) cancelOpen();
            },
            onClickCapture: (event: ReactMouseEvent<MaterialElement>) => {
                if (!prepareOpen || disabled || open.current || replayingOpen.current) return;
                event.preventDefault(); event.stopPropagation();
                void requestOpen(() => ref.current?.showPicker());
            },
            onChange: (event: FormEvent<MaterialElement>) => {
                const selected = event.currentTarget.value as string;
                if (disabled || !options.some((option) => !option.kind && option.value === selected && !option.disabled)) return;
                if (onSelect) {
                    pendingSelection.current = selected;
                    if (!open.current) commitSelection();
                } else onChange?.(selected);
            },
            onKeyDownCapture: (event: ReactKeyboardEvent<MaterialElement>) => {
                if (event.key === "Escape" && pendingOpen.current) {
                    cancelOpen(); event.preventDefault(); event.stopPropagation(); return;
                }
                if (prepareOpen && !disabled && !open.current && !replayingOpen.current && ["Enter", "Space", "ArrowDown", "ArrowUp", "Home", "End"].includes(event.code)) {
                    const target = event.nativeEvent.composedPath()[0];
                    const key = { key: event.key, code: event.code, bubbles: true, composed: true };
                    event.preventDefault(); event.stopPropagation();
                    void requestOpen(() => { if (target instanceof HTMLElement && target.isConnected) target.dispatchEvent(new KeyboardEvent("keydown", key)); });
                    return;
                }
                if (event.key !== "Escape" || !open.current) return;
                if (event.nativeEvent.composedPath().some((node) => node instanceof HTMLElement && node.matches(materialElements.selectOption))) return;
                // Async options may arrive after opening, while focus remains
                // in the field. Use Material's normal focusout closing path.
                const element = event.currentTarget;
                pendingSelection.current = undefined;
                element.addEventListener("closed", () => element.focus(), { once: true });
                element.blur();
                event.preventDefault();
                event.stopPropagation();
            },
            quick: reduce,
            ref,
            required,
            supportingText,
            value
        } as MaterialProps,
        options.map((option) => option.kind ? createElement("div", {
            className: `alcomd-select-${option.kind}`,
            key: option.value,
            role: option.kind === "status" ? "status" : option.kind === "heading" ? "presentation" : "separator",
            "aria-live": option.kind === "status" ? "polite" : undefined
        }, option.kind === "separator" ? null : option.label) : createElement(materialElements.selectOption, {
            disabled: option.disabled,
            title: option.title,
            ...(option.displayText === undefined ? {} : { displayText: option.displayText }),
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

export function Checkbox({ checked, disabled, label, labelHidden = false, onChange }: { checked: boolean; disabled?: boolean; label: string; labelHidden?: boolean; onChange?(checked: boolean, shiftKey: boolean): void }) {
    const ref = useRef<MaterialElement>(null);
    const shift = useRef(false);
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
                onClickCapture: (event: ReactMouseEvent<MaterialElement>) => { shift.current = event.shiftKey; },
                onKeyDownCapture: (event: ReactKeyboardEvent<MaterialElement>) => { shift.current = event.shiftKey; },
                onChange: (event: FormEvent<MaterialElement>) => onChange?.(Boolean(event.currentTarget.checked), shift.current)
            } as MaterialProps)}
            {labelHidden ? null : <span>{label}</span>}
        </label>
    );
}

export function DialogCloseButton({ afterClose, disabled, ...props }: Omit<ButtonProps, "onClick" | "type"> & { afterClose?: () => void }) {
    const dialog = useContext(DialogCloseContext);
    if (dialog === undefined) throw new Error("Missing dialog close context");
    return <Button {...props} disabled={disabled} type="button" onClick={() => dialog.requestClose(afterClose)} />;
}

interface DialogView {
    actions: ReactNode;
    headlineActions: ReactNode;
    children: ReactNode;
    title: string;
    wide: boolean;
}

function observeDialogSize(dialog: HTMLDialogElement, content: HTMLElement) {
    const measure = () => ({ width: dialog.offsetWidth, height: dialog.offsetHeight });
    let size = measure();
    let animation: Animation | undefined;
    let stopped = false;
    const resize = () => {
        // Animated dimensions also trigger the observer. Measure the new layout after they settle.
        if (stopped || !dialog.open || animation !== undefined) return;
        const next = measure();
        if (next.width === size.width && next.height === size.height) return;
        const before = size;
        size = next;
        if (reducedMotion()) return;
        // On a wide-to-standard transition, the standard cap would otherwise clamp the first frame.
        const keyframe = ({ width, height }: typeof size) => ({
            width: `${width}px`, maxWidth: `min(${width}px, calc(100vw - 48px))`, height: `${height}px`
        });
        animation = dialog.animate([keyframe(before), keyframe(next)], { duration: 180, easing: "cubic-bezier(0.2, 0, 0, 1)" });
        animation.onfinish = () => { animation = undefined; resize(); };
    };
    const observer = new ResizeObserver(resize);
    observer.observe(dialog, { box: "border-box" });
    observer.observe(content, { box: "border-box" });
    const stopMotionListener = subscribeMotion(() => {
        if (!reducedMotion()) return;
        animation?.cancel();
        animation = undefined;
        size = measure();
    });
    const stop = () => {
        stopped = true;
        observer.disconnect();
        stopMotionListener();
    };
    return {
        startClosing() {
            stop();
            // Keep the in-flight size steady for Material's closing animation.
            animation?.pause();
        },
        dispose() {
            stop();
            animation?.cancel();
        }
    };
}

export function Dialog({ actions, headlineActions, children, onClose, open, title, wide = false, dismissible = true, returnFocus, returnFocusReady = true }: { actions?: ReactNode; headlineActions?: ReactNode; children: ReactNode; onClose(): void; open: boolean; title: string; wide?: boolean; dismissible?: boolean; returnFocus?: DialogOrigin; returnFocusReady?: boolean }) {
    const quick = useSyncExternalStore(subscribeMotion, reducedMotion);
    const ref = useRef<MaterialElement>(null);
    const contentRef = useRef<HTMLDivElement>(null);
    const lastOpenView = useRef<DialogView>(undefined);
    const closeRequest = useRef<{ afterClose?: () => void }>(undefined);
    const closing = useRef(false);
    const [closingView, setClosingView] = useState<DialogView>();
    const [restoreFocus, setRestoreFocus] = useState(false);
    const view = { actions, headlineActions, children, title, wide };
    // The owner can clear its form in the same render that requests a close.
    const visible = closingView ?? (open ? view : lastOpenView.current ?? view);
    useLayoutEffect(() => {
        if (open && !closing.current) lastOpenView.current = view;
    });
    const requestClose = useCallback((afterClose?: () => void) => {
        const element = ref.current;
        if (!element?.open || closing.current || closeRequest.current !== undefined) return;
        closeRequest.current = { afterClose };
        element.close?.();
    }, []);
    const finishClose = useEffectEvent(() => {
        const complete = closeRequest.current?.afterClose ?? onClose;
        closeRequest.current = undefined;
        closing.current = false;
        lastOpenView.current = undefined;
        setClosingView(undefined);
        if (returnFocus !== undefined) setRestoreFocus(true);
        complete();
    });
    const cancelClose = useEffectEvent((event: Event) => {
        if (!dismissible) event.preventDefault();
    });
    useLayoutEffect(() => {
        const element = ref.current;
        if (element === null) return;
        let release: (() => void) | undefined;
        let resize: ReturnType<typeof observeDialogSize> | undefined;
        let active = true;
        const observe = () => {
            const native = element.shadowRoot?.querySelector("dialog");
            const content = contentRef.current;
            if (active && native?.open && content !== null && !closing.current && resize === undefined) {
                resize = observeDialogSize(native, content);
            }
        };
        const opening = (event: Event) => {
            if (event.target !== element) return;
            // Material opens the native dialog immediately after dispatching this event.
            queueMicrotask(observe);
        };
        const opened = () => {
            if (closing.current) return;
            if (release === undefined && contentRef.current !== null) release = registerDialogSurface(contentRef.current);
        };
        const cleanup = () => {
            release?.();
            release = undefined;
            resize?.dispose();
            resize = undefined;
        };
        const startClose = (event: Event) => {
            if (event.target !== element) return;
            if (closing.current) { event.preventDefault(); return; }
            closing.current = true;
            resize?.startClosing();
            setClosingView(lastOpenView.current);
        };
        const closed = (event: Event) => {
            if (event.target !== element) return;
            cleanup();
            finishClose();
        };
        const cancel = (event: Event) => {
            if (event.target === element) cancelClose(event);
        };
        element.addEventListener("open", opening);
        element.addEventListener("opened", opened);
        element.addEventListener("close", startClose);
        element.addEventListener("closed", closed);
        element.addEventListener("cancel", cancel);
        // React can reattach effects while the native dialog is already open.
        observe();
        return () => {
            active = false;
            cleanup();
            element.removeEventListener("open", opening);
            element.removeEventListener("opened", opened);
            element.removeEventListener("close", startClose);
            element.removeEventListener("closed", closed);
            element.removeEventListener("cancel", cancel);
        };
    }, []);
    useLayoutEffect(() => {
        const element = ref.current;
        if (element === null) return;
        if (open) element.open = true;
        else requestClose();
    }, [open, requestClose]);
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
        <DialogCloseContext.Provider value={{ requestClose }}>{createElement(
            materialElements.dialog,
            { ariaLabel: visible.title, quick, ref, style: visible.wide ? { maxWidth: "min(848px, calc(100vw - 48px))" } : undefined } as MaterialProps,
            createElement("div", { slot: "headline", className: visible.headlineActions === undefined ? undefined : "material-dialog-headline",
                role: visible.headlineActions === undefined ? undefined : "group", "aria-label": visible.headlineActions === undefined ? undefined : visible.title },
                visible.headlineActions === undefined ? visible.title : <><span className="material-dialog-headline-title">{visible.title}</span><div className="material-dialog-headline-actions">{visible.headlineActions}</div></>),
            createElement("div", { ref: contentRef, className: visible.wide ? "material-dialog-content material-dialog-content--wide" : "material-dialog-content", slot: "content" }, <DialogOpenContext.Provider value={open}><LocalPageReadContext.Provider value={true}>{visible.children}</LocalPageReadContext.Provider></DialogOpenContext.Provider>),
            visible.actions === undefined ? null : createElement("div", { className: "material-dialog-actions", slot: "actions" }, visible.actions)
        )}</DialogCloseContext.Provider>,
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

export function ButtonProgress({ slot }: { slot?: "icon" }) {
    return <span aria-hidden="true" className="alcomd-button-progress" slot={slot}>
        {createElement(materialElements.circularProgress, { indeterminate: true })}
    </span>;
}
