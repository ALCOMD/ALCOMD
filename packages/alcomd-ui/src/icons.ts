import arrowBackIconUrl from "@material-symbols/svg-400/rounded/arrow_back.svg?url";
import arrowDownwardIconUrl from "@material-symbols/svg-400/rounded/arrow_downward.svg?url";
import arrowUpwardIconUrl from "@material-symbols/svg-400/rounded/arrow_upward.svg?url";
import backupIconUrl from "@material-symbols/svg-400/rounded/backup.svg?url";
import deleteIconUrl from "@material-symbols/svg-400/rounded/delete.svg?url";
import downloadIconUrl from "@material-symbols/svg-400/rounded/download.svg?url";
import extensionIconUrl from "@material-symbols/svg-400/rounded/extension.svg?url";
import extensionSelectedIconUrl from "@material-symbols/svg-400/rounded/extension-fill.svg?url";
import historyIconUrl from "@material-symbols/svg-400/rounded/history.svg?url";
import infoIconUrl from "@material-symbols/svg-400/rounded/info.svg?url";
import infoSelectedIconUrl from "@material-symbols/svg-400/rounded/info-fill.svg?url";
import logIconUrl from "@material-symbols/svg-400/rounded/list.svg?url";
import logSelectedIconUrl from "@material-symbols/svg-400/rounded/list-fill.svg?url";
import packagesIconUrl from "@material-symbols/svg-400/rounded/package_2.svg?url";
import packagesSelectedIconUrl from "@material-symbols/svg-400/rounded/package_2-fill.svg?url";
import projectsIconUrl from "@material-symbols/svg-400/rounded/folder.svg?url";
import projectsSelectedIconUrl from "@material-symbols/svg-400/rounded/folder-fill.svg?url";
import playArrowIconUrl from "@material-symbols/svg-400/rounded/play_arrow.svg?url";
import refreshIconUrl from "@material-symbols/svg-400/rounded/refresh.svg?url";
import searchIconUrl from "@material-symbols/svg-400/rounded/search.svg?url";
import settingsIconUrl from "@material-symbols/svg-400/rounded/settings.svg?url";
import settingsSelectedIconUrl from "@material-symbols/svg-400/rounded/settings-fill.svg?url";
import syncIconUrl from "@material-symbols/svg-400/rounded/sync.svg?url";
import taskCenterIconUrl from "@material-symbols/svg-400/rounded/task_alt.svg?url";
import taskCenterSelectedIconUrl from "@material-symbols/svg-400/rounded/task_alt-fill.svg?url";
import upgradeIconUrl from "@material-symbols/svg-400/rounded/upgrade.svg?url";
import viewGridIconUrl from "@material-symbols/svg-400/rounded/grid_view.svg?url";
import viewListIconUrl from "@material-symbols/svg-400/rounded/view_list.svg?url";

export interface IconAsset {
    readonly filled: boolean;
    readonly url: string;
}

function defineIcon(url: string, filled = false): IconAsset {
    return Object.freeze({ filled, url });
}

export const arrowBackIcon = defineIcon(arrowBackIconUrl);
export const arrowDownwardIcon = defineIcon(arrowDownwardIconUrl);
export const arrowUpwardIcon = defineIcon(arrowUpwardIconUrl);
export const backupIcon = defineIcon(backupIconUrl);
export const deleteIcon = defineIcon(deleteIconUrl);
export const downloadIcon = defineIcon(downloadIconUrl);
export const extensionIcon = defineIcon(extensionIconUrl);
export const extensionSelectedIcon = defineIcon(extensionSelectedIconUrl, true);
export const historyIcon = defineIcon(historyIconUrl);
export const infoIcon = defineIcon(infoIconUrl);
export const infoSelectedIcon = defineIcon(infoSelectedIconUrl, true);
export const logIcon = defineIcon(logIconUrl);
export const logSelectedIcon = defineIcon(logSelectedIconUrl, true);
export const packagesIcon = defineIcon(packagesIconUrl);
export const packagesSelectedIcon = defineIcon(packagesSelectedIconUrl, true);
export const projectsIcon = defineIcon(projectsIconUrl);
export const projectsSelectedIcon = defineIcon(projectsSelectedIconUrl, true);
export const playArrowIcon = defineIcon(playArrowIconUrl);
export const refreshIcon = defineIcon(refreshIconUrl);
export const searchIcon = defineIcon(searchIconUrl);
export const settingsIcon = defineIcon(settingsIconUrl);
export const settingsSelectedIcon = defineIcon(settingsSelectedIconUrl, true);
export const syncIcon = defineIcon(syncIconUrl);
export const taskCenterIcon = defineIcon(taskCenterIconUrl);
export const taskCenterSelectedIcon = defineIcon(taskCenterSelectedIconUrl, true);
export const upgradeIcon = defineIcon(upgradeIconUrl);
export const viewGridIcon = defineIcon(viewGridIconUrl);
export const viewListIcon = defineIcon(viewListIconUrl);
