import type { LaunchOptions } from "@playwright/test";
import playwrightPackage from "@playwright/test/package.json" with { type: "json" };

// Playwright 1.62.1's complete disabled-feature list. Chromium keeps only the last
// --disable-features argument, so adding one feature must retain every default.
// https://github.com/microsoft/playwright/issues/43557
const disabledFeatures = [
    "AvoidUnnecessaryBeforeUnloadCheckSync",
    "BoundaryEventDispatchTracksNodeRemoval",
    "DestroyProfileOnBrowserClose",
    "DialMediaRouteProvider",
    "GlobalMediaControls",
    "HttpsUpgrades",
    "LensOverlay",
    "MediaRouter",
    "PaintHolding",
    "ThirdPartyStoragePartitioning",
    "BlockOriginHeaderModificationOnRedirect",
    "Translate",
    "AutoDeElevate",
    "OptimizationHints",
    "msForceBrowserSignIn",
    "msEdgeUpdateLaunchServicesPreferredVersion",
    // Matches the upstream fix for Windows loopback connection allocation errors.
    // https://github.com/microsoft/playwright/pull/43605
    "TcpPortRandomizationWin"
];

if (playwrightPackage.version !== "1.62.1") {
    throw new Error("Review the Chromium disabled-feature defaults when updating the pinned Playwright version.");
}

// The added feature exists only on Windows, as in Playwright's upstream list.
export const chromiumLaunchOptions: LaunchOptions = {
    args: [`--disable-features=${disabledFeatures.join(",")}`]
};
