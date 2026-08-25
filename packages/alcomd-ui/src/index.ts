export const productFamily = "ALCOMD";
export const technicalName = "alcomd";

export const spacing = {
    compact: "8px",
    standard: "16px",
    spacious: "24px"
} as const;

export type AppearanceMode = "system" | "light" | "dark";
export type InterfaceDensity = "comfortable" | "compact";
export type SourceColor = "violet" | "blue" | "teal";

export interface AppearanceSettings {
    mode: AppearanceMode;
    density: InterfaceDensity;
    sourceColor: SourceColor;
}

export const defaultAppearance: AppearanceSettings = {
    mode: "system",
    density: "comfortable",
    sourceColor: "violet"
};

export function applyAppearance(root: HTMLElement, settings: AppearanceSettings): void {
    root.dataset.appearance = settings.mode;
    root.dataset.density = settings.density;
    root.dataset.sourceColor = settings.sourceColor;
}
