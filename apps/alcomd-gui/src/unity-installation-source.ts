export function unityInstallationSourceLabel(source: string): string {
    if (source === "manual") return "Manually added";
    if (source === "hub_config" || source === "unity_cli_hint" || source === "unity_hub") return "Unity installation list";
    return "Detected by the system";
}
