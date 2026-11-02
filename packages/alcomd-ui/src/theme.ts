import {
    argbFromHex,
    Hct,
    hexFromArgb,
    MaterialDynamicColors,
    SchemeVibrant,
    SchemeTonalSpot,
    SchemeFidelity,
    SchemeMonochrome,
    SchemeNeutral,
    SchemeExpressive,
    SchemeContent,
    SchemeRainbow,
    SchemeFruitSalad
} from "@material/material-color-utilities";

export type AppearanceMode = "system" | "light" | "dark";
export type SourceColor = `#${string}`;
export type ThemeScheme = "tonal-spot" | "fidelity" | "monochrome" | "neutral" | "vibrant" | "expressive" | "content" | "rainbow" | "fruit-salad";
export interface ThemePreferences {
    mode: AppearanceMode;
    sourceColor: string | null;
    scheme: ThemeScheme;
}

export interface AppearanceSettings {
    mode: AppearanceMode;
    sourceColor: SourceColor;
    scheme: ThemeScheme;
}

export const defaultThemeSourceColor: SourceColor = "#6CB6FF";

export const defaultAppearance: AppearanceSettings = {
    mode: "system",
    sourceColor: defaultThemeSourceColor,
    scheme: "vibrant"
};

export function resolveThemeSourceColor(value: string | null): SourceColor {
    return value !== null && /^#[0-9a-f]{6}$/i.test(value)
        ? value.toUpperCase() as SourceColor
        : defaultThemeSourceColor;
}

const materialColors = new MaterialDynamicColors();
const colorRoles = [
    ...materialColors.allColors,
    materialColors.surfaceVariant(),
    materialColors.shadow(),
    materialColors.scrim(),
    materialColors.surfaceTint()
];

const schemeConstructors = {
    "tonal-spot": SchemeTonalSpot,
    fidelity: SchemeFidelity,
    monochrome: SchemeMonochrome,
    neutral: SchemeNeutral,
    vibrant: SchemeVibrant,
    expressive: SchemeExpressive,
    content: SchemeContent,
    rainbow: SchemeRainbow,
    "fruit-salad": SchemeFruitSalad
} as const;

export function themeColorRoles(sourceColor: SourceColor, dark: boolean, name: ThemeScheme = "vibrant"): Record<string, string> {
    const scheme = new schemeConstructors[name](Hct.fromInt(argbFromHex(sourceColor)), dark, 0);
    return Object.fromEntries(colorRoles.map((role) => [
        role.name.replaceAll("_", "-"),
        hexFromArgb(role.getArgb(scheme))
    ]));
}

export function applyAppearance(root: HTMLElement, settings: AppearanceSettings): void {
    const sourceColor = resolveThemeSourceColor(settings.sourceColor);
    for (const dark of [false, true]) {
        const mode = dark ? "dark" : "light";
        for (const [role, color] of Object.entries(themeColorRoles(sourceColor, dark, settings.scheme))) {
            root.style.setProperty(`--alcomd-color-${mode}-${role}`, color);
        }
    }
    root.dataset.appearance = settings.mode;
    root.dataset.sourceColor = sourceColor;
    root.dataset.themeScheme = settings.scheme;
}
