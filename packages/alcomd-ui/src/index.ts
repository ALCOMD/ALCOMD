export const productFamily = "ALCOMD";
export const technicalName = "alcomd";

import "@material/web/button/filled-button.js";
import "@material/web/button/filled-tonal-button.js";
import "@material/web/button/text-button.js";
import "@material/web/checkbox/checkbox.js";
import "@material/web/dialog/dialog.js";
import "@material/web/iconbutton/icon-button.js";
import "@material/web/progress/linear-progress.js";
import "@material/web/select/outlined-select.js";
import "@material/web/select/select-option.js";
import "@material/web/switch/switch.js";
import "@material/web/textfield/outlined-text-field.js";

export const materialElements = {
    button: {
        filled: "md-filled-button",
        tonal: "md-filled-tonal-button",
        text: "md-text-button"
    },
    checkbox: "md-checkbox",
    dialog: "md-dialog",
    iconButton: "md-icon-button",
    progress: "md-linear-progress",
    select: "md-outlined-select",
    selectOption: "md-select-option",
    switch: "md-switch",
    textField: "md-outlined-text-field"
} as const;

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
