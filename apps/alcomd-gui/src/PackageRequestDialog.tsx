import { useState } from "react";
import type { PackageBulkIntent, PackageSourceSelector } from "./core-models";
import type { PackageActionSelection } from "./CoreActions";
import { Button, Dialog, DialogCloseButton, Select, TextField, type DialogOrigin } from "./Material";
import { useTranslation } from "./i18n";

type Change = PackageBulkIntent["kind"];
type Mode = "install" | "upgrade" | "bulk" | "reinstall" | "reinstall-all" | "resolve";
type Policy = "default" | "stable" | "prerelease";
interface Row {
    packageId: string;
    sourceOptions: Array<{ key: string; label: string; selector: PackageSourceSelector }>;
}

export function PackageRequestDialog({ rows, origin, onClose, onPlan }: {
    rows: Row[];
    origin: DialogOrigin;
    onClose(): void;
    onPlan(selection: Omit<PackageActionSelection, "key" | "origin">): void;
}) {
    const { t } = useTranslation();
    const [mode, setMode] = useState<Mode>("install");
    const [change, setChange] = useState<Change>("install");
    const [packageId, setPackageId] = useState("");
    const [version, setVersion] = useState("");
    const [policy, setPolicy] = useState<Policy>("default");
    const [sourceKey, setSourceKey] = useState("");
    const [queue, setQueue] = useState<PackageBulkIntent[]>([]);
    const id = packageId.trim();
    const row = rows.find((item) => item.packageId === id);
    const source = row?.sourceOptions.find((item) => item.key === sourceKey)?.selector;
    const choosingPackages = mode !== "resolve";
    const kind = mode === "bulk" ? change : mode === "reinstall-all" ? "reinstall" : mode;
    const versionAllowed = kind === "install" || kind === "upgrade";
    const many = mode === "bulk" || mode === "reinstall" || mode === "reinstall-all";
    const constraint = policy === "default" ? {} : { includePrerelease: policy === "prerelease" };
    const override = source === undefined ? {} : { source };

    function currentIntent(): PackageBulkIntent | undefined {
        if (!id || kind === "resolve" || (mode === "reinstall-all" && !source)) return;
        if (kind === "remove") return { kind, packageId: id };
        if (kind === "reinstall") return { kind, packageId: id, ...override };
        return { kind, packageId: id, ...constraint, ...override, ...(version.trim() ? { versionRange: version.trim() } : {}) };
    }
    const current = currentIntent();
    const repeated = queue.some((item) => item.packageId === id);
    const intents = current && !repeated ? [...queue, current] : queue;
    const applicable = mode === "resolve" || (!repeated && (mode === "reinstall-all" ? !id || !!current : intents.length > 0));

    function clearEntry() {
        setPackageId(""); setSourceKey(""); setVersion(""); setPolicy("default");
    }
    function prepare() {
        if (!applicable) return;
        if (mode === "resolve") onPlan({ action: mode, packageId: "", ...constraint });
        else if (mode === "bulk") onPlan({ action: "bulk-candidates", packageId: "", intents });
        else if (mode === "reinstall" || mode === "reinstall-all") {
            const sources = intents.flatMap((item) => item.kind === "reinstall" && item.source ? [{ packageId: item.packageId, source: item.source }] : []);
            onPlan({ action: mode, packageId: "", ...(mode === "reinstall" ? { packageIds: intents.map((item) => item.packageId) } : {}), ...(sources.length ? { reinstallSources: sources } : {}) });
        } else onPlan({ action: mode, packageId: id, ...(version.trim() ? { version: version.trim() } : {}), ...constraint, ...override });
    }
    const modes: Array<{ value: Mode; label: string }> = [
        { value: "install", label: t("Install") }, { value: "upgrade", label: t("Upgrade") },
        { value: "bulk", label: t("Mixed package changes") }, { value: "reinstall", label: t("Reinstall selected") },
        { value: "reinstall-all", label: t("Reinstall All Installed Packages…") }, { value: "resolve", label: t("Resolve Dependencies…") },
    ];
    const changes: Array<{ value: Change; label: string }> = [
        { value: "install", label: t("Install") }, { value: "upgrade", label: t("Upgrade") },
        { value: "remove", label: t("Remove") }, { value: "reinstall", label: t("Reinstall") }
    ];
    return <Dialog open title={t("Custom package plan")} onClose={onClose} returnFocus={origin} actions={<>
        <DialogCloseButton variant="text">{t("Cancel")}</DialogCloseButton>
        <Button type="button" disabled={!applicable} onClick={prepare}>{t("Review package plan")}</Button>
    </>}>
        <Select label={t("Plan kind")} value={mode} options={modes} onChange={(value) => {
            const selected = modes.find((item) => item.value === value);
            if (selected) { setMode(selected.value); setQueue([]); clearEntry(); }
        }} />
        {mode === "bulk" && <Select label={t("Package action")} value={change} options={changes} onChange={(value) => {
            const selected = changes.find((item) => item.value === value);
            if (selected) { setChange(selected.value); setVersion(""); setPolicy("default"); }
        }} />}
        {choosingPackages && <TextField label={t("Package ID")} value={packageId} onInput={(value) => { setPackageId(value); setSourceKey(""); }} />}
        {versionAllowed && <TextField label={t("Version range")} value={version} onInput={setVersion} />}
        {(versionAllowed || mode === "resolve") && <Select label={t("Prerelease policy")} value={policy} options={[
            { value: "default", label: t("Default") }, { value: "stable", label: t("Stable versions only") }, { value: "prerelease", label: t("Include prerelease versions") }
        ]} onChange={(value) => { if (value === "default" || value === "stable" || value === "prerelease") setPolicy(value); }} />}
        {choosingPackages && kind !== "remove" && row?.sourceOptions.length ? <Select label={t("Installation source")} value={sourceKey}
            options={[{ value: "", label: t("Default") }, ...row.sourceOptions.map((item) => ({ value: item.key, label: item.label }))]} onChange={setSourceKey} /> : null}
        {many && <Button variant="text" type="button" disabled={!current || repeated} onClick={() => { if (current && !repeated) { setQueue([...queue, current]); clearEntry(); } }}>{t("Add package")}</Button>}
        {queue.length > 0 && <ul>{queue.map((item) => <li key={item.packageId}>{changes.find((choice) => choice.value === item.kind)?.label} · {item.packageId}
            <Button type="button" variant="text" onClick={() => setQueue(queue.filter((other) => other !== item))}>{t("Remove")}</Button>
        </li>)}</ul>}
    </Dialog>;
}
