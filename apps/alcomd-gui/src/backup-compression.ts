import type { BackupCompressionMode } from "./core-models";
import type { Translate } from "./i18n-types";

const levels: ReadonlyArray<{ value: BackupCompressionMode; message: string }> = [
    { value: "store", message: "0 - Store only" },
    { value: "fast", message: "1 - Fastest compression" },
    { value: "level3", message: "3 - Fast compression" },
    { value: "level5", message: "5 - Normal compression" },
    { value: "level7", message: "7 - Maximum compression" },
    { value: "maximum", message: "9 - Ultra compression" }
];

export function backupCompressionOptions(t: Translate) {
    return levels.map(({ value, message }) => ({ value, label: t(message) }));
}

export function backupCompressionLabel(t: Translate, value: string): string {
    const level = levels.find((level) => level.value === value);
    return level === undefined ? value : t(level.message);
}
