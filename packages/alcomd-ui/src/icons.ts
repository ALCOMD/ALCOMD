import extensionIconUrl from "@material-symbols/svg-400/rounded/extension.svg?url";
import extensionSelectedIconUrl from "@material-symbols/svg-400/rounded/extension-fill.svg?url";
import logIconUrl from "@material-symbols/svg-400/rounded/list.svg?url";
import logSelectedIconUrl from "@material-symbols/svg-400/rounded/list-fill.svg?url";
import packagesIconUrl from "@material-symbols/svg-400/rounded/package_2.svg?url";
import packagesSelectedIconUrl from "@material-symbols/svg-400/rounded/package_2-fill.svg?url";
import projectsIconUrl from "@material-symbols/svg-400/rounded/folder.svg?url";
import projectsSelectedIconUrl from "@material-symbols/svg-400/rounded/folder-fill.svg?url";
import settingsIconUrl from "@material-symbols/svg-400/rounded/settings.svg?url";
import settingsSelectedIconUrl from "@material-symbols/svg-400/rounded/settings-fill.svg?url";

export interface IconAsset {
    readonly filled: boolean;
    readonly url: string;
}

function defineIcon(url: string, filled = false): IconAsset {
    return Object.freeze({ filled, url });
}

export const extensionIcon = defineIcon(extensionIconUrl);
export const extensionSelectedIcon = defineIcon(extensionSelectedIconUrl, true);
export const logIcon = defineIcon(logIconUrl);
export const logSelectedIcon = defineIcon(logSelectedIconUrl, true);
export const packagesIcon = defineIcon(packagesIconUrl);
export const packagesSelectedIcon = defineIcon(packagesSelectedIconUrl, true);
export const projectsIcon = defineIcon(projectsIconUrl);
export const projectsSelectedIcon = defineIcon(projectsSelectedIconUrl, true);
export const settingsIcon = defineIcon(settingsIconUrl);
export const settingsSelectedIcon = defineIcon(settingsSelectedIconUrl, true);
