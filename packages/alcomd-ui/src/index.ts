export const productFamily = "ALCOMD";
export const technicalName = "alcomd";

import "@material/web/button/filled-button.js";
import "@material/web/button/filled-tonal-button.js";
import "@material/web/button/outlined-button.js";
import "@material/web/button/text-button.js";
import "@material/web/checkbox/checkbox.js";
import "@material/web/dialog/dialog.js";
import "@material/web/focus/md-focus-ring.js";
import "@material/web/iconbutton/icon-button.js";
import "@material/web/list/list-item.js";
import "@material/web/list/list.js";
import "@material/web/menu/menu-item.js";
import "@material/web/menu/menu.js";
import "@material/web/progress/linear-progress.js";
import "@material/web/ripple/ripple.js";
import "@material/web/select/outlined-select.js";
import "@material/web/select/filled-select.js";
import "@material/web/select/select-option.js";
import "@material/web/switch/switch.js";
import "@material/web/textfield/outlined-text-field.js";
import "@material/web/textfield/filled-text-field.js";

export const materialElements = {
    button: {
        filled: "md-filled-button",
        tonal: "md-filled-tonal-button",
        outlined: "md-outlined-button",
        text: "md-text-button"
    },
    checkbox: "md-checkbox",
    dialog: "md-dialog",
    focusRing: "md-focus-ring",
    iconButton: "md-icon-button",
    list: "md-list",
    listItem: "md-list-item",
    menu: "md-menu",
    menuItem: "md-menu-item",
    progress: "md-linear-progress",
    ripple: "md-ripple",
    select: {
        filled: "md-filled-select",
        outlined: "md-outlined-select"
    },
    selectOption: "md-select-option",
    switch: "md-switch",
    textField: {
        filled: "md-filled-text-field",
        outlined: "md-outlined-text-field"
    }
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
