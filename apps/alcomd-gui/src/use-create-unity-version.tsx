import { requestErrorMessage } from "./request-errors";
import type { ProjectTemplateDeclaration, RpcError } from "@alcomd/sdk";
import { useEffect, useRef, useState } from "react";

import type { UnityInstallation } from "./core-models";
import { Button, Select, TextField } from "./Material";
import { useTranslation } from "./i18n";
import type { GuiRpcClient } from "./rpc";
import { normalizeRequestError } from "./request-errors";

export function useCreateUnityVersion(client: GuiRpcClient, template: ProjectTemplateDeclaration | undefined, open: boolean) {
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
    }, [client, open, template?.id]);
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
    const presets = template?.unity.presets ?? [];
    const series = template?.unity.series ?? [];
    const unrestricted = series.length === 0 && presets.length === 0;
    const allowCustom = unrestricted || series.length > 0;
    const acceptsVersion = (candidate: string) => {
        const match = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)[abfpx][1-9]\d*(?:c[1-9]\d*)?$/.exec(candidate);
        return match !== null && (unrestricted || presets.includes(candidate) || series.includes(match[1] + "." + match[2]));
    };
    const versions = [...new Set([...presets, ...installations.map((installation) => installation.unityVersion).filter(acceptsVersion)])];
    const suggested = versions.length === 1 ? versions[0]! : "";
    useEffect(() => {
        if (!edited.current) setVersion(suggested);
    }, [suggested, template?.id]);
    return {
        version,
        custom,
        chooseVersion: (value: string) => {
            edited.current = true;
            setCustom(value === "custom");
            setVersion(value === "custom" ? "" : value);
        },
        setVersion: (value: string) => { edited.current = true; setVersion(value); },
        versions,
        allowCustom,
        valid: acceptsVersion(version.trim()),
        error,
        retry: () => setRetry((current) => current + 1)
    };
}

export function CreateUnityVersionField({ disabled, selection }: { disabled: boolean; selection: ReturnType<typeof useCreateUnityVersion> }) {
    const { t } = useTranslation();
    return <>
        <div className="project-create-unity-version">
            <span>{t("Unity version:")}</span>
            <Select disabled={disabled} label={t("Unity version")} onChange={selection.chooseVersion}
                options={[...selection.versions.map((version) => ({ label: version, value: version })), ...(selection.allowCustom ? [{ label: t("Custom"), value: "custom" }] : [])]}
                value={selection.custom ? "custom" : selection.version} />
        </div>
        {selection.custom ? <TextField disabled={disabled} label={t("Unity version")} onInput={selection.setVersion} required value={selection.version} /> : null}
        {selection.error !== undefined ? <div role="alert"><strong>{t("Installed Unity versions could not be loaded.")}</strong><p>{t(requestErrorMessage((selection.error).code))}</p><code>{(selection.error).code}</code>{selection.error.diagnosticId === undefined ? null : <p><code>{selection.error.diagnosticId}</code></p>}
            <Button disabled={disabled} onClick={selection.retry} type="button" variant="text">{t("Retry")}</Button>
        </div> : null}
    </>;
}
