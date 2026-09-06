import keyboardArrowDown20Svg from "../assets/material-symbols/20/keyboard_arrow_down.svg?raw";
import keyboardArrowUp20Svg from "../assets/material-symbols/20/keyboard_arrow_up.svg?raw";
import accountCircle24Svg from "../assets/material-symbols/24/account_circle.svg?raw";
import arrowBack24Svg from "../assets/material-symbols/24/arrow_back.svg?raw";
import arrowDownward24Svg from "../assets/material-symbols/24/arrow_downward.svg?raw";
import arrowUpward24Svg from "../assets/material-symbols/24/arrow_upward.svg?raw";
import backup24Svg from "../assets/material-symbols/24/backup.svg?raw";
import close24Svg from "../assets/material-symbols/24/close.svg?raw";
import delete24Svg from "../assets/material-symbols/24/delete.svg?raw";
import download24Svg from "../assets/material-symbols/24/download.svg?raw";
import extension24Svg from "../assets/material-symbols/24/extension.svg?raw";
import folder24Svg from "../assets/material-symbols/24/folder.svg?raw";
import gridView24Svg from "../assets/material-symbols/24/grid_view.svg?raw";
import help24Svg from "../assets/material-symbols/24/help.svg?raw";
import history24Svg from "../assets/material-symbols/24/history.svg?raw";
import info24Svg from "../assets/material-symbols/24/info.svg?raw";
import list24Svg from "../assets/material-symbols/24/list.svg?raw";
import menu24Svg from "../assets/material-symbols/24/menu.svg?raw";
import moreVert24Svg from "../assets/material-symbols/24/more_vert.svg?raw";
import package24Svg from "../assets/material-symbols/24/package_2.svg?raw";
import playArrow24Svg from "../assets/material-symbols/24/play_arrow.svg?raw";
import public24Svg from "../assets/material-symbols/24/public.svg?raw";
import refresh24Svg from "../assets/material-symbols/24/refresh.svg?raw";
import search24Svg from "../assets/material-symbols/24/search.svg?raw";
import settings24Svg from "../assets/material-symbols/24/settings.svg?raw";
import star24Svg from "../assets/material-symbols/24/star.svg?raw";
import sync24Svg from "../assets/material-symbols/24/sync.svg?raw";
import taskAlt24Svg from "../assets/material-symbols/24/task_alt.svg?raw";
import upgrade24Svg from "../assets/material-symbols/24/upgrade.svg?raw";
import viewList24Svg from "../assets/material-symbols/24/view_list.svg?raw";

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

export function resolveIconGeometry(asset: IconAsset, size: IconSize): { viewBox: string; path: string } {
    const svg = asset.sources[size];
    if (svg === undefined) {
        throw new Error(`Material Symbol ${asset.name} has no opsz=${size} asset`);
    }
    // Only the pinned, single-path upstream assets are accepted, never arbitrary SVG markup.
    const match = svg.trim().match(/^<svg xmlns="http:\/\/www\.w3\.org\/2000\/svg" height="(?:20|24)" viewBox="(0 -960 960 960)" width="(?:20|24)"><path d="([MmLlHhVvCcSsQqTtAaZz0-9., +\\-]+)"\/><\/svg>$/);
    if (match === null || match[1] === undefined || match[2] === undefined) {
        throw new Error(`Unsupported pinned Material Symbol geometry: ${asset.name}`);
    }
    return { viewBox: match[1], path: match[2] };
}

export const arrowBackIcon = defineIcon("arrow_back", { 24: arrowBack24Svg });
export const arrowDownwardIcon = defineIcon("arrow_downward", { 24: arrowDownward24Svg });
export const arrowUpwardIcon = defineIcon("arrow_upward", { 24: arrowUpward24Svg });
export const accountCircleIcon = defineIcon("account_circle", { 24: accountCircle24Svg });
export const backupIcon = defineIcon("backup", { 24: backup24Svg });
export const deleteIcon = defineIcon("delete", { 24: delete24Svg });
export const downloadIcon = defineIcon("download", { 24: download24Svg });
export const extensionIcon = defineIcon("extension", { 24: extension24Svg });
export const historyIcon = defineIcon("history", { 24: history24Svg });
export const helpIcon = defineIcon("help", { 24: help24Svg });
export const infoIcon = defineIcon("info", { 24: info24Svg });
export const keyboardArrowDownIcon = defineIcon("keyboard_arrow_down", { 20: keyboardArrowDown20Svg });
export const keyboardArrowUpIcon = defineIcon("keyboard_arrow_up", { 20: keyboardArrowUp20Svg });
export const logIcon = defineIcon("list", { 24: list24Svg });
export const menuIcon = defineIcon("menu", { 24: menu24Svg });
export const moreVertIcon = defineIcon("more_vert", { 24: moreVert24Svg });
export const closeIcon = defineIcon("close", { 24: close24Svg });
export const packagesIcon = defineIcon("package_2", { 24: package24Svg });
export const projectsIcon = defineIcon("folder", { 24: folder24Svg });
export const publicIcon = defineIcon("public", { 24: public24Svg });
export const playArrowIcon = defineIcon("play_arrow", { 24: playArrow24Svg });
export const refreshIcon = defineIcon("refresh", { 24: refresh24Svg });
export const searchIcon = defineIcon("search", { 24: search24Svg });
export const settingsIcon = defineIcon("settings", { 24: settings24Svg });
export const starIcon = defineIcon("star", { 24: star24Svg });
export const syncIcon = defineIcon("sync", { 24: sync24Svg });
export const taskCenterIcon = defineIcon("task_alt", { 24: taskAlt24Svg });
export const upgradeIcon = defineIcon("upgrade", { 24: upgrade24Svg });
export const viewGridIcon = defineIcon("grid_view", { 24: gridView24Svg });
export const viewListIcon = defineIcon("view_list", { 24: viewList24Svg });
