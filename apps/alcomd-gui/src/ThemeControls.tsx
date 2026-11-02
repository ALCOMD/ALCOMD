import { resolveThemeSourceColor, type ThemePreferences, type ThemeScheme } from "@alcomd/ui";
import { useEffect, useRef, useState } from "react";

import type { SettingsGetResult } from "./core-models";
import { useTranslation } from "./i18n";
import { Button, ColorPicker, Select, TextField } from "./Material";
import type { RpcError } from "@alcomd/sdk";
import { normalizeRequestError } from "./request-errors";
import { RequestReminder, useReminder } from "./Reminder";

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

export function ThemeControls({ value, disabled, onSave, onReload, onPreview, onSession, onDirtyChange }: ThemeControlsProps) {
    const { t } = useTranslation();
    const reminder = useReminder();
    const [draft, setDraft] = useState(value);
    const [hex, setHex] = useState<string>(resolveThemeSourceColor(value.sourceColor));
    const [saving, setSaving] = useState(false);
    const [error, setError] = useState<RpcError>();
    const desired = useRef(value);
    const saved = useRef(value);
    const working = useRef(false);
    const active = useRef(true);
    const callbacks = useRef({ onSave, onReload, onPreview, onDirtyChange });
    callbacks.current = { onSave, onReload, onPreview, onDirtyChange };
    const dirty = !sameTheme(draft, saved.current) || error !== undefined;

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
    useEffect(() => {
        if (working.current || sameTheme(saved.current, value)) return;
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
        callbacks.current.onPreview(merged);
    }, [value.sourceColor, value.mode, value.scheme]);

    const change = (next: ThemePreferences) => {
        if (!active.current || working.current) return;
        desired.current = next;
        setDraft(next);
        callbacks.current.onPreview(next);
    };

    const source = (input: string) => {
        const normalized = input.trim().toUpperCase();
        if (!/^#[0-9A-F]{6}$/.test(normalized)) return;
        const color = resolveThemeSourceColor(normalized);
        setHex(color);
        change({ ...desired.current, sourceColor: color });
    };

    const accept = (accepted: SettingsGetResult) => {
        const theme = themePreferences(accepted);
        saved.current = theme;
        if (!active.current) return;
        desired.current = theme;
        setDraft(theme);
        setHex(resolveThemeSourceColor(theme.sourceColor));
        setError(undefined);
        callbacks.current.onPreview(undefined);
        callbacks.current.onDirtyChange(false);
        reminder.success(t("Theme saved"));
    };

    const save = async () => {
        if (!active.current || working.current || !dirty) return;
        // Failures, including unknown commit outcomes, must be reconciled before another explicit save.
        if (error !== undefined) { await retry(); return; }
        working.current = true;
        setSaving(true);
        try {
            const accepted = await callbacks.current.onSave({ ...desired.current });
            accept(accepted);
        } catch (caught: unknown) {
            if (active.current) setError(normalizeRequestError(caught));
        } finally {
            working.current = false;
            if (active.current) setSaving(false);
        }
    };

    const retry = async () => {
        if (!active.current || working.current) return;
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
            callbacks.current.onPreview(merged);
            if (sameTheme(merged, fresh)) {
                accept(current);
            } else {
                // Retry is itself the explicit confirmation. It never writes unrelated fields.
                const accepted = await callbacks.current.onSave(merged);
                accept(accepted);
            }
        } catch (caught: unknown) {
            if (active.current) setError(normalizeRequestError(caught));
        } finally {
            working.current = false;
            if (active.current) setSaving(false);
        }
    };

    return <form aria-label={t("Theme")} aria-busy={saving} className="theme-editor" onSubmit={(event) => { event.preventDefault(); event.stopPropagation(); void save(); }}>
        <div className="settings-row">
            <label htmlFor="theme-mode">{t("Color mode")}</label>
            <Select disabled={disabled || saving} id="theme-mode" label={t("Color mode")} value={draft.mode}
                options={[{ label: t("Light"), value: "light" }, { label: t("System"), value: "system" }, { label: t("Dark"), value: "dark" }]}
                onChange={(mode) => change({ ...desired.current, mode: mode as ThemePreferences["mode"] })} />
        </div>
        <div className="settings-row">
            <label htmlFor="theme-source">{t("Source color")}</label>
            <div className="theme-source-row">
                <ColorPicker disabled={disabled || saving} id="theme-picker" label={t("Choose theme color")} value={resolveThemeSourceColor(draft.sourceColor)} onInput={source} />
                <TextField label="" aria-label={t("Source color")} disabled={disabled || saving} id="theme-source"
                    value={hex} onInput={(next) => { setHex(next); source(next); }} onBlur={() => setHex(resolveThemeSourceColor(desired.current.sourceColor))} />
            </div>
        </div>
        <div className="settings-row">
            <label htmlFor="theme-scheme">{t("Color scheme")}</label>
            <Select disabled={disabled || saving} id="theme-scheme" label={t("Color scheme")} value={draft.scheme}
                options={Object.entries(themeSchemeLabels).map(([key, label]) => ({ value: key, label: t(label) }))}
                onChange={(scheme) => change({ ...desired.current, scheme: scheme as ThemeScheme })} />
        </div>
        <div className="theme-feedback">
            {error === undefined ? null : <RequestReminder error={error} title={t(error.code === "revision_conflict" ? "Theme changed elsewhere."
                : error.code === "daemon_unavailable" || error.code === "internal_error" ? "Theme save result is unknown" : "Theme could not be saved.")}
                message={error.code === "daemon_unavailable" || error.code === "internal_error" ? "Retry checks the current settings before writing again." : undefined}>
                <Button disabled={disabled || saving} type="button" variant="tonal" onClick={() => void retry()}>{t("Retry theme save")}</Button>
            </RequestReminder>}
        </div>
        <div className="theme-editor-footer">
            <Button disabled={disabled || saving} type="button" variant="text" onClick={() => {
                const next: ThemePreferences = { mode: "system", sourceColor: null, scheme: "vibrant" };
                setHex(resolveThemeSourceColor(null));
                change(next);
            }}>{t("Restore defaults")}</Button>
            <div className="dialog-actions">
                <Button disabled={saving || !dirty} type="button" variant="text" onClick={() => {
                    desired.current = saved.current;
                    setDraft(saved.current);
                    setHex(resolveThemeSourceColor(saved.current.sourceColor));
                    setError(undefined);
                    callbacks.current.onPreview(undefined);
                    callbacks.current.onDirtyChange(false);
                }}>{t("Cancel")}</Button>
                <Button disabled={disabled || saving || !dirty} type="submit">{t("Save")}</Button>
            </div>
        </div>
    </form>;
}
