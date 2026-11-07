import { requestErrorMessage } from "./request-errors";
import type { RpcError } from "@alcomd/sdk";
import { useEffect, useRef, useState } from "react";

import type { UnityVersionOptions } from "./core-models";
import { Button, Select, TextField } from "./Material";
import { useTranslation } from "./i18n";
import type { GuiRpcClient } from "./rpc";
import { normalizeRequestError } from "./request-errors";

export function useCreateUnityVersion(client: GuiRpcClient, fixedVersion: string | undefined, open: boolean) {
    const [version, setVersion] = useState("");
    const [custom, setCustom] = useState(false);
    const [options, setOptions] = useState<UnityVersionOptions>();
    const [error, setError] = useState<RpcError>();
    const [retry, setRetry] = useState(0);
    const edited = useRef(false);
    useEffect(() => {
        edited.current = false;
        setVersion("");
        setCustom(false);
    }, [client, open, fixedVersion]);
    useEffect(() => {
        if (!open || fixedVersion !== undefined) return;
        let active = true;
        setOptions(undefined);
        setError(undefined);
        void (async () => {
            const { versionOptions } = await client.unityInstallationsList(undefined, true);
            if (versionOptions === undefined) throw new Error("Missing Unity version options");
            if (active) setOptions(versionOptions);
        })().catch((caught: unknown) => { if (active) setError(normalizeRequestError(caught)); });
        return () => { active = false; };
    }, [client, fixedVersion, open, retry]);
    const suggested = options?.defaultVersion ?? "";
    useEffect(() => {
        if (!edited.current) setVersion(suggested);
    }, [client, fixedVersion, open, suggested]);
    return {
        version: fixedVersion ?? version,
        fixedVersion,
        custom,
        chooseVersion: (value: string) => {
            edited.current = true;
            setCustom(value === "custom");
            setVersion(value === "custom" ? "" : value);
        },
        setVersion: (value: string) => { edited.current = true; setVersion(value); },
        versions: options?.versions ?? [],
        error: fixedVersion === undefined ? error : undefined,
        retry: () => setRetry((current) => current + 1)
    };
}

export function CreateUnityVersionField({ disabled, selection }: { disabled: boolean; selection: ReturnType<typeof useCreateUnityVersion> }) {
    const { t } = useTranslation();
    if (selection.fixedVersion !== undefined) {
        return <TextField label={t("Unity version")} readOnly value={selection.fixedVersion} />;
    }
    return <>
        <div className="project-create-unity-version">
            <span>{t("Unity version:")}</span>
            <Select disabled={disabled} label={t("Unity version")} onChange={selection.chooseVersion}
                options={[...selection.versions.map((version) => ({ label: version, value: version })), { label: t("Custom"), value: "custom" }]}
                value={selection.custom ? "custom" : selection.version} />
        </div>
        {selection.custom ? <TextField disabled={disabled} label={t("Unity version")} onInput={selection.setVersion} required value={selection.version} /> : null}
        {selection.error !== undefined ? <div role="alert"><strong>{t("Installed Unity versions could not be loaded.")}</strong><p>{t(requestErrorMessage((selection.error).code))}</p><code>{(selection.error).code}</code>{selection.error.diagnosticId === undefined ? null : <p><code>{selection.error.diagnosticId}</code></p>}
            <Button disabled={disabled} onClick={selection.retry} type="button" variant="text">{t("Retry")}</Button>
        </div> : null}
    </>;
}
