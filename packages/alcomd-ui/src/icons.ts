import keyboardArrowDown20Url from "../assets/material-symbols/20/keyboard_arrow_down.svg?url";
import keyboardArrowUp20Url from "../assets/material-symbols/20/keyboard_arrow_up.svg?url";
import accountCircle24Url from "../assets/material-symbols/24/account_circle.svg?url";
import arrowBack24Url from "../assets/material-symbols/24/arrow_back.svg?url";
import arrowDownward24Url from "../assets/material-symbols/24/arrow_downward.svg?url";
import arrowUpward24Url from "../assets/material-symbols/24/arrow_upward.svg?url";
import backup24Url from "../assets/material-symbols/24/backup.svg?url";
import close24Url from "../assets/material-symbols/24/close.svg?url";
import delete24Url from "../assets/material-symbols/24/delete.svg?url";
import download24Url from "../assets/material-symbols/24/download.svg?url";
import extension24Url from "../assets/material-symbols/24/extension.svg?url";
import folder24Url from "../assets/material-symbols/24/folder.svg?url";
import gridView24Url from "../assets/material-symbols/24/grid_view.svg?url";
import help24Url from "../assets/material-symbols/24/help.svg?url";
import history24Url from "../assets/material-symbols/24/history.svg?url";
import info24Url from "../assets/material-symbols/24/info.svg?url";
import list24Url from "../assets/material-symbols/24/list.svg?url";
import menu24Url from "../assets/material-symbols/24/menu.svg?url";
import moreVert24Url from "../assets/material-symbols/24/more_vert.svg?url";
import package24Url from "../assets/material-symbols/24/package_2.svg?url";
import playArrow24Url from "../assets/material-symbols/24/play_arrow.svg?url";
import public24Url from "../assets/material-symbols/24/public.svg?url";
import refresh24Url from "../assets/material-symbols/24/refresh.svg?url";
import search24Url from "../assets/material-symbols/24/search.svg?url";
import settings24Url from "../assets/material-symbols/24/settings.svg?url";
import star24Url from "../assets/material-symbols/24/star.svg?url";
import sync24Url from "../assets/material-symbols/24/sync.svg?url";
import taskAlt24Url from "../assets/material-symbols/24/task_alt.svg?url";
import upgrade24Url from "../assets/material-symbols/24/upgrade.svg?url";
import viewList24Url from "../assets/material-symbols/24/view_list.svg?url";

export type IconSize = 20 | 24;

interface IconSources {
    readonly 20?: string;
    readonly 24?: string;
}

export interface IconAsset {
    readonly filled: boolean;
    readonly name: string;
    readonly sources: IconSources;
}

function defineIcon(name: string, sources: IconSources, filled = false): IconAsset {
    return { filled, name, sources };
}

export function resolveIconUrl(asset: IconAsset, size: IconSize): string {
    const url = asset.sources[size];
    if (url === undefined) {
        throw new Error(`Material Symbol ${asset.name} has no opsz=${size} asset`);
    }
    return url;
}

export const arrowBackIcon = defineIcon("arrow_back", { 24: arrowBack24Url });
export const arrowDownwardIcon = defineIcon("arrow_downward", { 24: arrowDownward24Url });
export const arrowUpwardIcon = defineIcon("arrow_upward", { 24: arrowUpward24Url });
export const accountCircleIcon = defineIcon("account_circle", { 24: accountCircle24Url });
export const backupIcon = defineIcon("backup", { 24: backup24Url });
export const deleteIcon = defineIcon("delete", { 24: delete24Url });
export const downloadIcon = defineIcon("download", { 24: download24Url });
export const extensionIcon = defineIcon("extension", { 24: extension24Url });
export const historyIcon = defineIcon("history", { 24: history24Url });
export const helpIcon = defineIcon("help", { 24: help24Url });
export const infoIcon = defineIcon("info", { 24: info24Url });
export const keyboardArrowDownIcon = defineIcon("keyboard_arrow_down", { 20: keyboardArrowDown20Url });
export const keyboardArrowUpIcon = defineIcon("keyboard_arrow_up", { 20: keyboardArrowUp20Url });
export const logIcon = defineIcon("list", { 24: list24Url });
export const menuIcon = defineIcon("menu", { 24: menu24Url });
export const moreVertIcon = defineIcon("more_vert", { 24: moreVert24Url });
export const closeIcon = defineIcon("close", { 24: close24Url });
export const packagesIcon = defineIcon("package_2", { 24: package24Url });
export const projectsIcon = defineIcon("folder", { 24: folder24Url });
export const publicIcon = defineIcon("public", { 24: public24Url });
export const playArrowIcon = defineIcon("play_arrow", { 24: playArrow24Url });
export const refreshIcon = defineIcon("refresh", { 24: refresh24Url });
export const searchIcon = defineIcon("search", { 24: search24Url });
export const settingsIcon = defineIcon("settings", { 24: settings24Url });
export const starIcon = defineIcon("star", { 24: star24Url });
export const syncIcon = defineIcon("sync", { 24: sync24Url });
export const taskCenterIcon = defineIcon("task_alt", { 24: taskAlt24Url });
export const upgradeIcon = defineIcon("upgrade", { 24: upgrade24Url });
export const viewGridIcon = defineIcon("grid_view", { 24: gridView24Url });
export const viewListIcon = defineIcon("view_list", { 24: viewList24Url });
