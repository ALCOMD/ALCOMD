import type { RpcError } from "@alcomd/sdk";
import { useEffect, useRef, useState } from "react";

import type { TemplateRecord, UnityInstallation } from "./core-models";
import { Button, Select, TextField } from "./Material";
import { RequestReminder } from "./Reminder";
import { useTranslation } from "./i18n";
import type { GuiRpcClient } from "./rpc";
import { normalizeRequestError } from "./request-errors";

export function useCreateUnityVersion(client: GuiRpcClient, template: TemplateRecord | undefined, open: boolean) {
    const [version, setVersion] = useState("");
    const [custom, setCustom] = useState(false);
    const [installations, setInstallations] = useState<UnityInstallation[]>([]);
    const [error, setError] = useState<RpcError>();
    const [retry, setRetry] = useState(0);
    const edited = useRef(false);
    useEffect(() => {
        edited.current = false;
        setVersion("");
        setCustom(false);
    }, [client, open, template?.templateId]);
    useEffect(() => {
        if (!open) return;
        let active = true;
        setInstallations([]);
        setError(undefined);
        void (async () => {
            const rows: UnityInstallation[] = [];
            const cursors = new Set<string>();
            let cursor: string | undefined;
            do {
                const page = await client.unityInstallationsList(cursor);
                if (!active) return;
                rows.push(...page.installations);
                cursor = page.nextCursor;
                if (cursor !== undefined && cursors.has(cursor)) throw new Error("Repeated installation cursor");
                if (cursor !== undefined) cursors.add(cursor);
            } while (cursor !== undefined);
            if (active) setInstallations(rows);
        })().catch((caught: unknown) => { if (active) setError(normalizeRequestError(caught)); });
        return () => { active = false; };
    }, [client, open, retry]);
    const compatible = installations.filter((installation) => {
        const match = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)[abfpx][1-9]\d*(?:c[1-9]\d*)?$/.exec(installation.unityVersion);
        return match !== null && template?.unity !== undefined
            && Number(match[1]) === template.unity.major && Number(match[2]) === template.unity.minor;
    });
    const compatibleVersion = compatible.length === 1 ? compatible[0]!.unityVersion : "";
    useEffect(() => {
        if (!edited.current) {
            setVersion(compatibleVersion);
            setCustom(compatible.length === 0);
        }
    }, [compatibleVersion, compatible.length, template?.templateId]);
    return {
        version,
        custom,
        chooseVersion: (value: string) => {
            edited.current = true;
            setCustom(value === "custom");
            setVersion(value === "custom" ? "" : value);
        },
        setVersion: (value: string) => { edited.current = true; setVersion(value); },
        versions: Array.from(new Set(compatible.map((installation) => installation.unityVersion))),
        error,
        retry: () => setRetry((current) => current + 1)
    };
}

export function CreateUnityVersionField({ disabled, selection }: { disabled: boolean; selection: ReturnType<typeof useCreateUnityVersion> }) {
    const { t } = useTranslation();
    return <>
        <Select disabled={disabled} label={t("Installed Unity version")} onChange={selection.chooseVersion}
            options={[{ label: t("Choose an installed version"), value: "" }, ...selection.versions.map((version) => ({ label: version, value: version })), { label: t("Custom"), value: "custom" }]}
            value={selection.custom ? "custom" : selection.version} />
        {selection.custom ? <TextField disabled={disabled} label={t("Unity version")} onInput={selection.setVersion} required value={selection.version} /> : null}
        {selection.error !== undefined ? <RequestReminder error={selection.error} title={t("Installed Unity versions could not be loaded.")}>
            <Button disabled={disabled} onClick={selection.retry} type="button" variant="text">{t("Retry")}</Button>
        </RequestReminder> : null}
    </>;
}
