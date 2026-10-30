export const productFamily = "ALCOMD";
export const technicalName = "alcomd";

import "@material/web/button/filled-button.js";
import "@material/web/button/filled-tonal-button.js";
import "@material/web/button/outlined-button.js";
import "@material/web/button/text-button.js";
import "@material/web/checkbox/checkbox.js";
import "@material/web/dialog/dialog.js";
import "@material/web/focus/md-focus-ring.js";
import "@material/web/icon/icon.js";
import "@material/web/iconbutton/icon-button.js";
import "@material/web/list/list-item.js";
import "@material/web/list/list.js";
import "@material/web/labs/segmentedbutton/outlined-segmented-button.js";
import "@material/web/labs/segmentedbuttonset/outlined-segmented-button-set.js";
import "@material/web/menu/menu-item.js";
import "@material/web/menu/menu.js";
import "@material/web/progress/linear-progress.js";
import "@material/web/ripple/ripple.js";
import "@material/web/radio/radio.js";
import "@material/web/select/outlined-select.js";
import "@material/web/select/filled-select.js";
import "@material/web/select/select-option.js";
import "@material/web/slider/slider.js";
import "@material/web/switch/switch.js";
import "@material/web/textfield/outlined-text-field.js";
import "@material/web/textfield/filled-text-field.js";
import { installMaterialScrollbars } from "./scrollbars";
import { installMaterialMotion } from "./motion";
export { createScrollArea } from "./scrollbars";
export { applyMotion, reducedMotion, subscribeMotion, type MotionPreference } from "./motion";

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
    icon: "md-icon",
    iconButton: "md-icon-button",
    list: "md-list",
    listItem: "md-list-item",
    menu: "md-menu",
    menuItem: "md-menu-item",
    progress: "md-linear-progress",
    ripple: "md-ripple",
    radio: "md-radio",
    select: {
        filled: "md-filled-select",
        outlined: "md-outlined-select"
    },
    selectOption: "md-select-option",
    segmentedButton: "md-outlined-segmented-button",
    segmentedButtonSet: "md-outlined-segmented-button-set",
    slider: "md-slider",
    switch: "md-switch",
    textField: {
        filled: "md-filled-text-field",
        outlined: "md-outlined-text-field"
    }
} as const;

installMaterialMotion([...new Set([
    ...Object.values(materialElements).flatMap((value) => typeof value === "string" ? [value] : Object.values(value))
])]);

installMaterialScrollbars([
    { name: materialElements.dialog, viewport: ".scroller" },
    { name: materialElements.menu, viewport: ".items" }
]);

export const spacing = {
    compact: "8px",
    standard: "16px",
    spacious: "24px"
} as const;

export {
    applyAppearance,
    defaultAppearance,
    defaultThemeSourceColor,
    resolveThemeSourceColor,
    themeHct,
    sourceColorFromHct,
    type AppearanceMode,
    type AppearanceSettings,
    type InterfaceDensity,
    type SourceColor,
    type ThemePreferences,
    type ThemeScheme
} from "./theme";
