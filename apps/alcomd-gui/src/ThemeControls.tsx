import { resolveThemeSourceColor, sourceColorFromHct, themeHct, type ThemePreferences, type ThemeScheme } from "@alcomd/ui";
import { paletteIcon } from "@alcomd/ui/icons";
import { useEffect, useRef, useState } from "react";

import type { SettingsGetResult } from "./core-models";
import { useTranslation } from "./i18n";
import { Button, ColorPicker, Icon, RadioGroup, SegmentedButtons, Slider, TextField } from "./Material";
import { UtilityActionDialog, UtilityDialogCancel, useUtilityDialogBusy, useUtilityDialogClose } from "./UtilityActionDialog";

export const themeSchemeLabels = {
    "tonal-spot": "Tonal spot", fidelity: "Fidelity", monochrome: "Monochrome", neutral: "Neutral",
    vibrant: "Vibrant", expressive: "Expressive", content: "Content", rainbow: "Rainbow", "fruit-salad": "Fruit salad"
} as const;

export function themePreferences(value: SettingsGetResult): ThemePreferences {
    const { sourceColor, mode, scheme } = value.settings.appearance;
    return { sourceColor, mode, scheme };
}

function sameTheme(first: ThemePreferences, second: ThemePreferences): boolean {
    return first.sourceColor === second.sourceColor && first.mode === second.mode && first.scheme === second.scheme;
}

interface ThemeControlsProps {
    value: ThemePreferences;
    disabled: boolean;
    onSave(theme: ThemePreferences): Promise<SettingsGetResult>;
    onReload(): Promise<SettingsGetResult>;
    onPreview(theme: ThemePreferences | undefined): void;
    onSession(cancel: (() => void) | undefined): void;
    onDirtyChange(dirty: boolean): void;
}

export function ThemeControls(props: ThemeControlsProps) {
    const { t } = useTranslation();
    const [open, setOpen] = useState(false);
    const [confirmed, setConfirmed] = useState(false);
    const opener = useRef<HTMLElement>(null);
    return <div className="action-row">
        {confirmed ? <span role="status">{t("Theme saved")}</span> : null}
        <Button id="settings-theme-button" ref={opener} disabled={props.disabled} type="button" variant="filled"
            onClick={() => { setConfirmed(false); setOpen(true); }}><Icon asset={paletteIcon} slot="icon" />{t("Theme")}</Button>
        {open ? <UtilityActionDialog title={t("Theme")} wide onClose={() => {
            setOpen(false);
            window.requestAnimationFrame(() => opener.current?.focus());
        }}>
            <ThemeEditor {...props} onSaved={() => setConfirmed(true)} />
        </UtilityActionDialog> : null}
    </div>;
}

function ThemeEditor({ value, disabled, onSave, onReload, onPreview, onSession, onDirtyChange, onSaved }: ThemeControlsProps & { onSaved(): void }) {
    const { t } = useTranslation();
    const close = useUtilityDialogClose();
    const [draft, setDraft] = useState(value);
    const [hex, setHex] = useState<string>(resolveThemeSourceColor(value.sourceColor));
    const [hct, setHct] = useState(() => themeHct(resolveThemeSourceColor(value.sourceColor)));
    const [saving, setSaving] = useState(false);
    const [finished, setFinished] = useState(false);
    const [error, setError] = useState<"save" | "conflict">();
    const desired = useRef(value);
    const saved = useRef(value);
    const working = useRef(false);
    const active = useRef(true);
    const callbacks = useRef({ onSave, onReload, onPreview, onDirtyChange, onSaved });
    callbacks.current = { onSave, onReload, onPreview, onDirtyChange, onSaved };
    const dirty = !finished && (!sameTheme(draft, saved.current) || error !== undefined);
    useUtilityDialogBusy(saving);

    useEffect(() => {
        active.current = true;
        onSession(() => { active.current = false; callbacks.current.onPreview(undefined); callbacks.current.onDirtyChange(false); });
        return () => {
            active.current = false;
            onSession(undefined);
            callbacks.current.onPreview(undefined);
            callbacks.current.onDirtyChange(false);
        };
    }, [onSession]);

    useEffect(() => { onDirtyChange(dirty); }, [dirty, onDirtyChange]);
    useEffect(() => { if (finished && !saving) close(); }, [finished, saving, close]);
    useEffect(() => {
        if (working.current || finished || sameTheme(saved.current, value)) return;
        const previous = saved.current;
        const local = desired.current;
        const merged: ThemePreferences = {
            sourceColor: local.sourceColor === previous.sourceColor ? value.sourceColor : local.sourceColor,
            mode: local.mode === previous.mode ? value.mode : local.mode,
            scheme: local.scheme === previous.scheme ? value.scheme : local.scheme
        };
        saved.current = value;
        desired.current = merged;
        setDraft(merged);
        setHex(resolveThemeSourceColor(merged.sourceColor));
        setHct(themeHct(resolveThemeSourceColor(merged.sourceColor)));
        callbacks.current.onPreview(merged);
    }, [value.sourceColor, value.mode, value.scheme, finished]);

    const change = (next: ThemePreferences) => {
        if (!active.current || working.current) return;
        desired.current = next;
        setDraft(next);
        callbacks.current.onPreview(next);
    };

    const source = (input: string, keepHct = false) => {
        const normalized = input.trim().toUpperCase();
        if (!/^#[0-9A-F]{6}$/.test(normalized)) return;
        const color = resolveThemeSourceColor(normalized);
        setHex(color);
        if (!keepHct) setHct(themeHct(color));
        change({ ...desired.current, sourceColor: color });
    };

    const save = async () => {
        if (!active.current || working.current || finished || !dirty) return;
        // Failures, including unknown commit outcomes, must be reconciled before another explicit save.
        if (error !== undefined) { await retry(); return; }
        working.current = true;
        setSaving(true);
        try {
            const accepted = await callbacks.current.onSave({ ...desired.current });
            saved.current = themePreferences(accepted);
            if (!active.current) return;
            callbacks.current.onPreview(undefined);
            callbacks.current.onDirtyChange(false);
            callbacks.current.onSaved();
            setFinished(true);
        } catch (caught: unknown) {
            if (active.current) setError(typeof caught === "object" && caught !== null && "code" in caught && caught.code === "revision_conflict" ? "conflict" : "save");
        } finally {
            working.current = false;
            if (active.current) setSaving(false);
        }
    };

    const retry = async () => {
        if (!active.current || working.current || finished) return;
        working.current = true;
        setSaving(true);
        const previous = saved.current;
        try {
            const current = await callbacks.current.onReload();
            if (!active.current) return;
            const fresh = themePreferences(current);
            const local = desired.current;
            const merged: ThemePreferences = {
                sourceColor: local.sourceColor === previous.sourceColor ? fresh.sourceColor : local.sourceColor,
                mode: local.mode === previous.mode ? fresh.mode : local.mode,
                scheme: local.scheme === previous.scheme ? fresh.scheme : local.scheme
            };
            saved.current = fresh;
            desired.current = merged;
            setDraft(merged);
            setHex(resolveThemeSourceColor(merged.sourceColor));
            setHct(themeHct(resolveThemeSourceColor(merged.sourceColor)));
            callbacks.current.onPreview(merged);
            if (sameTheme(merged, fresh)) {
                callbacks.current.onPreview(undefined);
                callbacks.current.onDirtyChange(false);
                callbacks.current.onSaved();
                setFinished(true);
            } else {
                // Retry is itself the explicit confirmation. It never writes unrelated fields.
                const accepted = await callbacks.current.onSave(merged);
                saved.current = themePreferences(accepted);
                if (!active.current) return;
                callbacks.current.onPreview(undefined);
                callbacks.current.onDirtyChange(false);
                callbacks.current.onSaved();
                setFinished(true);
            }
        } catch (caught: unknown) {
            if (active.current) setError(typeof caught === "object" && caught !== null && "code" in caught && caught.code === "revision_conflict" ? "conflict" : "save");
        } finally {
            working.current = false;
            if (active.current) setSaving(false);
        }
    };

    return <form aria-busy={saving} className="theme-editor" onSubmit={(event) => { event.preventDefault(); event.stopPropagation(); void save(); }}>
        <div className="theme-editor-columns">
            <div className="theme-color-column">
                <div className="theme-panel">
                    <div className="theme-source-row">
                        <ColorPicker disabled={disabled || saving} id="theme-picker" label={t("Choose theme color")} value={resolveThemeSourceColor(draft.sourceColor)} onInput={source} />
                        <TextField label={t("Source color")} disabled={disabled || saving} id="theme-source"
                            value={hex} onInput={(next) => { setHex(next); source(next); }} onBlur={() => setHex(resolveThemeSourceColor(desired.current.sourceColor))} />
                    </div>
                </div>
                <div className="theme-panel theme-hct-controls">
                    {(["hue", "chroma", "tone"] as const).map((key) => <div key={key}>
                        <label htmlFor={`theme-${key}`}>{t({ hue: "Hue", chroma: "Chroma", tone: "Tone" }[key])}</label>
                        <Slider disabled={disabled || saving} id={`theme-${key}`} labeled label={t({ hue: "Hue", chroma: "Chroma", tone: "Tone" }[key])}
                            min={0} max={{ hue: 360, chroma: 150, tone: 100 }[key]} value={hct[key]} onInput={(next) => {
                                if (!Number.isFinite(next)) return;
                                const updated = { ...hct, [key]: Math.min({ hue: 360, chroma: 150, tone: 100 }[key], Math.max(0, next)) };
                                setHct(updated);
                                source(sourceColorFromHct(updated), true);
                            }} />
                    </div>)}
                </div>
            </div>
            <div className="theme-options-column">
                <SegmentedButtons id="theme-mode" label={t("Theme")} showCheckmark value={draft.mode}
                    options={[{ label: t("Light"), value: "light" }, { label: t("System"), value: "system" }, { label: t("Dark"), value: "dark" }].map((option) => ({ ...option, disabled: disabled || saving }))}
                    onChange={(mode) => change({ ...desired.current, mode: mode as ThemePreferences["mode"] })} />
                <div className="theme-panel">
                    <RadioGroup disabled={disabled || saving} id="theme-scheme" label={t("Color scheme")} value={draft.scheme}
                    options={Object.entries(themeSchemeLabels).map(([key, label]) => ({ value: key, label: t(label) }))}
                    onChange={(scheme) => change({ ...desired.current, scheme: scheme as ThemeScheme })} />
                </div>
            </div>
        </div>
        <div className="theme-feedback">
            {error === undefined ? null : <div role="alert">
                <p>{t(error === "conflict" ? "Theme changed elsewhere." : "Theme could not be saved.")}</p>
                <Button disabled={disabled || saving} type="button" variant="tonal" onClick={() => void retry()}>{t("Retry theme save")}</Button>
            </div>}
        </div>
        <div className="theme-editor-footer">
            <Button disabled={disabled || saving} type="button" variant="text" onClick={() => {
                const next: ThemePreferences = { mode: "system", sourceColor: null, scheme: "vibrant" };
                setHex(resolveThemeSourceColor(null));
                setHct(themeHct(resolveThemeSourceColor(null)));
                change(next);
            }}>{t("Restore defaults")}</Button>
            <div className="dialog-actions">
                <UtilityDialogCancel disabled={saving} />
                <Button disabled={disabled || saving || !dirty} type="submit">{t("Save")}</Button>
            </div>
        </div>
    </form>;
}
