import type { ClientInstanceSummary, ClientProgramSummary, NativeRpcMethods } from "@alcomd/sdk";
import { useCallback, useEffect, useRef, useState } from "react";
import { Button, Dialog, DialogCloseButton } from "./Material";
import { capabilities, useCapability } from "./capabilities";
import { useTranslation } from "./i18n";
import { normalizeRequestError, requestErrorMessage } from "./request-errors";
import type { GuiRpcClient } from "./rpc";

type Disconnect = NativeRpcMethods["clients.disconnect"]["params"];
type RegistryPage<Row> = { coreInstanceId: string; items: Row[]; nextCursor?: string; coverageIncomplete?: boolean };

function useRegistryPage<Row>(read: (cursor?: string) => Promise<RegistryPage<Row>>) {
    const [page, setPage] = useState<RegistryPage<Row>>();
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState<string>();
    const sequence = useRef(0);
    const load = useCallback(async (cursor?: string) => {
        const request = ++sequence.current;
        setBusy(true);
        setError(undefined);
        try {
            const next = await read(cursor);
            if (request === sequence.current) setPage(previous => ({
                ...next, items: cursor && previous && previous.coreInstanceId === next.coreInstanceId ? [...previous.items, ...next.items] : next.items,
            }));
        } catch (caught) {
            if (request === sequence.current) setError(normalizeRequestError(caught).code);
        } finally {
            if (request === sequence.current) setBusy(false);
        }
    }, [read]);
    useEffect(() => {
        void load();
        return () => { ++sequence.current; };
    }, [load]);
    return { page, load, busy, error };
}

export function NativeClientsSettings({ client }: { client: GuiRpcClient }) {
    const { t } = useTranslation();
    const available = useCapability(capabilities.clientRegistry);
    return <section className="settings-section" aria-label={t("Client programs")}><h2>{t("Client programs")}</h2>
        {available ? <Programs client={client} /> : <p>{t("The Core does not provide client discovery.")}</p>}
    </section>;
}

function Programs({ client }: { client: GuiRpcClient }) {
    const { t } = useTranslation();
    const read = useCallback((cursor?: string) => client.clientsList({cursor}), [client]);
    const { page, load, error, busy } = useRegistryPage<ClientProgramSummary>(read);
    const [selected, setSelected] = useState<ClientProgramSummary>();
    return <>
        <Button type="button" variant="text" disabled={busy} onClick={() => void load()}>{t("Refresh list")}</Button>
        {error && <p role="alert">{t(requestErrorMessage(error))}</p>}
        {page?.coverageIncomplete && <p role="status">{t("Some installation records could not be read.")}</p>}
        {page?.items.length === 0 && <p>{t("No client programs found.")}</p>}
        {page?.items.map(program => <div className="settings-row" key={program.programKey}>
            <div><strong>{program.displayName}</strong><p>{program.installations.map(i => i.productVersion).join(", ") || t("Not installed")} · {t("{count} active instances", { count: program.activeInstanceCount })}</p></div>
            <Button type="button" variant="text" onClick={() => setSelected(program)}>{t("View connections")}</Button>
        </div>)}
        {page?.nextCursor && <Button type="button" disabled={busy} onClick={() => void load(page.nextCursor)}>{t("Load more")}</Button>}
        {selected && <Instances key={selected.programKey} client={client} program={selected} onClose={() => { setSelected(undefined); void load(); }} />}
    </>;
}

function Instances({ client, program, onClose }: { client: GuiRpcClient; program: ClientProgramSummary; onClose(): void }) {
    const { t } = useTranslation();
    const read = useCallback((cursor?: string) => client.clientInstancesList({programKey: program.programKey, cursor}), [client, program.programKey]);
    const { page, load, busy, error } = useRegistryPage<ClientInstanceSummary>(read);
    const [request, setRequest] = useState<Disconnect>();
    const [disconnecting, setDisconnecting] = useState(false);
    const [failure, setFailure] = useState<string>();
    const select = (target: Disconnect["target"]) => {
        if (!page) return;
        setFailure(undefined);
        setRequest({ expectedCoreInstanceId: page.coreInstanceId, target, idempotencyKey: crypto.randomUUID() });
    };
    const disconnect = async () => {
        if (!request || disconnecting) return;
        setDisconnecting(true);
        setFailure(undefined);
        try {
            await client.clientsDisconnect(request);
            setRequest(undefined);
            await load();
        } catch (caught) { setFailure(normalizeRequestError(caught).code); }
        finally { setDisconnecting(false); }
    };
    return <Dialog open dismissible={!disconnecting} title={program.displayName} onClose={onClose}>
        {request ? <>
            <p>{t("Disconnect this client target? It can reconnect.")}</p>
            <code>{request.target.id}</code>
            {failure && <p role="alert">{t(requestErrorMessage(failure))}</p>}
            <div className="dialog-actions">
                <Button type="button" variant="text" disabled={disconnecting} onClick={() => setRequest(undefined)}>{t("Back")}</Button>
                <Button type="button" disabled={disconnecting} onClick={() => void disconnect()}>{t("Disconnect")}</Button>
            </div>
        </> : <>
            {error && <p role="alert">{t(requestErrorMessage(error))}</p>}
            {page?.items.length === 0 && <p>{t("No active connections.")}</p>}
            {page?.items.map(instance => <section key={instance.instanceId}>
                <h3>{t("Client instance")}</h3><code>{instance.instanceId}</code>
                <Button type="button" variant="text" onClick={() => select({kind:"instance", id:instance.instanceId})}>{t("Disconnect instance")}</Button>
                {instance.connections.map(connection => <div className="settings-row" key={connection.connectionId}>
                    <div><code>{connection.connectionId}</code><p>{t("Reported by client: {name} {version}", {name:connection.reported.name, version:connection.reported.version})}</p></div>
                    <Button type="button" variant="text" onClick={() => select({kind:"connection",id:connection.connectionId})}>{t("Disconnect connection")}</Button>
                </div>)}
            </section>)}
            {page?.nextCursor && <Button type="button" disabled={busy} onClick={() => void load(page.nextCursor)}>{t("Load more")}</Button>}
            <div className="dialog-actions">
                <Button type="button" variant="text" disabled={busy} onClick={() => void load()}>{t("Refresh list")}</Button>
                <DialogCloseButton>{t("Close")}</DialogCloseButton>
            </div>
        </>}
    </Dialog>;
}
