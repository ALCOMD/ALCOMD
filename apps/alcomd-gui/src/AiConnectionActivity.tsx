import { useCallback, useEffect, useRef, useState } from "react";
import { Button, TextField } from "./Material";
import { useTranslation } from "./i18n";
import { normalizeRequestError, requestErrorMessage } from "./request-errors";
import type { GuiRpcClient } from "./rpc";

type Page = {items: object[]; totalTracked: number; truncated: boolean};
function ActivityRows({title,page}: {title:string;page?:Page}) {
    const {t} = useTranslation();
    if (!page) return null;
    return <details><summary>{title} ({page.totalTracked})</summary>
        {page.truncated && <p>{t("Showing the first {count} items.", {count:page.items.length})}</p>}
        {page.items.map((row,i) => <dl key={i}>{Object.entries(row).map(([label, value]) =>
            <div key={label}><dt>{label}</dt><dd>{value === null ? "—" : String(value)}</dd></div>)}</dl>)}
    </details>;
}

export function AiConnectionActivity({client}: {client:GuiRpcClient}) {
    const {t} = useTranslation();
    const [filter,setFilter] = useState("");
    const [limit,setLimit] = useState("");
    const [runtime,setRuntime] = useState<Awaited<ReturnType<GuiRpcClient["aiRuntimeSnapshot"]>>>();
    const [transport,setTransport] = useState<Awaited<ReturnType<GuiRpcClient["aiTransportSnapshot"]>>>();
    const [problem,setProblem] = useState<string>();
    const [busy,setBusy] = useState(false);
    const sequence = useRef(0);
    const load = useCallback(async (id: string, size: string) => {
        const n = ++sequence.current;
        const params = {...(id.trim() ? {clientId:id.trim()} : {}), ...(size ? {limit:Number(size)} : {})};
        setBusy(true);
        const [core,adapter] = await Promise.allSettled([client.aiRuntimeSnapshot(params),client.aiTransportSnapshot(params)]);
        if (sequence.current !== n) return;
        setRuntime(core.status === "fulfilled" ? core.value : undefined);
        setTransport(adapter.status === "fulfilled" ? adapter.value : undefined);
        const failed = [core,adapter].find(item => item.status === "rejected");
        setProblem(failed?.status === "rejected" ? normalizeRequestError(failed.reason).code : undefined);
        setBusy(false);
    },[client]);
    useEffect(() => { void load("",""); return () => { ++sequence.current; }; },[load]);
    return <section><h3>{t("AI connection activity")}</h3>
        <TextField label={t("Client ID")} value={filter} onInput={setFilter} />
        <TextField label={t("Maximum items")} type="number" min={1} max={100} value={limit} onInput={setLimit} />
        <Button type="button" variant="text" disabled={busy} onClick={() => void load(filter,limit)}>{t("Refresh list")}</Button>
        {problem && <p role="alert">{t(requestErrorMessage(problem))}</p>}
        {(runtime?.coverageIncomplete || transport?.coverageIncomplete) && <p>{t("Activity coverage is incomplete.")}</p>}
        <ActivityRows title={t("Core connections")} page={runtime?.connections} />
        <ActivityRows title={t("Core requests")} page={runtime?.requests} />
        {transport && <p>{t("Transport status")} · {transport.status}</p>}
        <ActivityRows title={t("Transport observers")} page={transport && {items:transport.observers,totalTracked:transport.observers.length,truncated:false}} />
        <ActivityRows title={t("Transport connections")} page={transport?.connections} />
        <ActivityRows title={t("Transport requests")} page={transport?.requests} />
        <ActivityRows title={t("Subscriptions")} page={transport?.subscriptionStreams} />
    </section>;
}
