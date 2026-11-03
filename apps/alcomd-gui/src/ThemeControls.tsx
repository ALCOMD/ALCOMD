import { resolveThemeSourceColor, type ThemePreferences, type ThemeScheme } from "@alcomd/ui";
import { useEffect, useState } from "react";
import type { SettingsGetResult } from "./core-models";
import { useTranslation } from "./i18n";
import { ColorPicker, Select, TextField } from "./Material";

export const themeSchemeLabels = {
    "tonal-spot": "Tonal spot", fidelity: "Fidelity", monochrome: "Monochrome", neutral: "Neutral",
    vibrant: "Vibrant", expressive: "Expressive", content: "Content", rainbow: "Rainbow", "fruit-salad": "Fruit salad"
} as const;

export function themePreferences(value: SettingsGetResult): ThemePreferences {
    const { sourceColor, mode, scheme } = value.settings.appearance;
    return { sourceColor, mode, scheme };
}

interface ThemeControlsProps {
    value: ThemePreferences;
    onChange(update: Partial<ThemePreferences>): void;
}

export function ThemeControls({ value, onChange }: ThemeControlsProps) {
    const { t } = useTranslation();
    const [hex, setHex] = useState<string>(resolveThemeSourceColor(value.sourceColor));
    useEffect(() => { setHex(resolveThemeSourceColor(value.sourceColor)); }, [value.sourceColor]);
    const source = (input: string) => {
        const normalized = input.trim().toUpperCase();
        if (!/^#[0-9A-F]{6}$/.test(normalized)) return;
        setHex(normalized);
        onChange({ sourceColor: normalized });
    };
    return <form aria-label={t("Theme")} className="theme-editor" onSubmit={(event) => { event.preventDefault(); event.stopPropagation(); }}>
        <div className="settings-row">
            <label htmlFor="theme-mode">{t("Color mode")}</label>
            <Select id="theme-mode" label={t("Color mode")} value={value.mode}
                options={[{ label: t("System"), value: "system" }, { label: t("Light"), value: "light" }, { label: t("Dark"), value: "dark" }]}
                onChange={(mode) => onChange({ mode: mode as ThemePreferences["mode"] })} />
        </div>
        <div className="settings-row">
            <label htmlFor="theme-source">{t("Source color")}</label>
            <div className="theme-source-row">
                <ColorPicker id="theme-picker" label={t("Choose theme color")} value={resolveThemeSourceColor(value.sourceColor)} onInput={source} />
                <TextField label="" aria-label={t("Source color")} id="theme-source"
                    value={hex} onInput={(next) => { setHex(next); source(next); }} onBlur={() => setHex(resolveThemeSourceColor(value.sourceColor))} />
            </div>
        </div>
        <div className="settings-row">
            <label htmlFor="theme-scheme">{t("Color scheme")}</label>
            <Select id="theme-scheme" label={t("Color scheme")} value={value.scheme}
                options={Object.entries(themeSchemeLabels).map(([key, label]) => ({ value: key, label: t(label) }))}
                onChange={(scheme) => onChange({ scheme: scheme as ThemeScheme })} />
        </div>
    </form>;
}
