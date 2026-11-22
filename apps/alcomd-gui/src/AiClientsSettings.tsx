import { useCallback, useEffect, useRef, useState } from "react";
import { Button, Dialog, DialogCloseButton, TextField } from "./Material";
import { useTranslation } from "./i18n";
import { normalizeRequestError, requestErrorMessage } from "./request-errors";
import type { GuiRpcClient } from "./rpc";
import { AiManagementRequests } from "./AiManagementRequests";
import { AiOAuthSettings } from "./AiOAuthSettings";
import { AiConnectionActivity } from "./AiConnectionActivity";

type AiClient = Awaited<ReturnType<GuiRpcClient["aiClientsList"]>>["items"][number];
type Change =
    | { kind: "create"; displayName: string; key: string }
    | { kind: "issue" | "revoke"; target: AiClient; key: string };

export function AiClientsSettings({ client }: { client: GuiRpcClient }) {
    const { t } = useTranslation();
    const [items, setItems] = useState<AiClient[]>([]);
    const [cursor, setCursor] = useState<string | null>(null);
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string>();
    const [name, setName] = useState("");
    const [change, setChange] = useState<Change>();
    const [writing, setWriting] = useState(false);
    const [writeError, setWriteError] = useState<string>();
    const [secret, setSecret] = useState<string>();
    const [revision, setRevision] = useState(0);
    const sequence = useRef(0);
    const submitting = useRef(false);

    const load = useCallback(async (after?: string) => {
        const request = ++sequence.current;
        setLoading(true);
        setError(undefined);
        try {
            const result = await client.aiClientsList(after ? { cursor: after } : {});
            if (request !== sequence.current) return;
            setItems(previous => after ? [...previous, ...result.items] : result.items);
            setCursor(result.nextCursor);
        } catch (caught) {
            if (request === sequence.current) setError(normalizeRequestError(caught).code);
        } finally {
            if (request === sequence.current) setLoading(false);
        }
    }, [client]);
    useEffect(() => {
        void load();
        return () => { ++sequence.current; };
    }, [load]);

    function review(next: Change) {
        setWriteError(undefined);
        setChange(next);
    }
    async function apply(asRequest = false) {
        if (!change || submitting.current) return;
        submitting.current = true;
        setWriting(true);
        setWriteError(undefined);
        try {
            if (asRequest) {
                const action = change.kind === "create" ? {kind: "createClient" as const, displayName: change.displayName}
                    : {kind: change.kind === "issue" ? "issueCredential" as const : "revokeClient" as const,
                        clientId: change.target.clientId, expectedRevision: change.target.revision};
                await client.aiRequestCreate({action, idempotencyKey: change.key});
                setRevision(value => value + 1);
            } else if (change.kind === "create") {
                await client.aiClientCreate({ displayName: change.displayName, idempotencyKey: change.key });
                setName("");
            } else {
                const params = { clientId: change.target.clientId, expectedRevision: change.target.revision, idempotencyKey: change.key };
                if (change.kind === "issue") setSecret((await client.aiClientIssueCredential(params)).credential);
                else await client.aiClientRevoke(params);
            }
            setChange(undefined);
            await load();
        } catch (caught) {
            setWriteError(normalizeRequestError(caught).code);
        } finally {
            submitting.current = false;
            setWriting(false);
        }
    }
    return <section className="settings-section" aria-label={t("AI clients")}>
        <h2>{t("AI clients")}</h2>
        <form onSubmit={event => { event.preventDefault(); if (name.trim()) review({ kind: "create", displayName: name, key: crypto.randomUUID() }); }}>
            <TextField label={t("Client name")} value={name} maxLength={128} required onInput={setName} />
            <Button type="submit" disabled={!name.trim()}>{t("Register client")}</Button>
        </form>
        <Button type="button" variant="text" disabled={loading} onClick={() => void load()}>{t("Refresh list")}</Button>
        {error && <p role="alert">{t(requestErrorMessage(error))}</p>}
        {!loading && !error && !items.length && <p>{t("No registered AI clients.")}</p>}
        {items.map(item => <div className="settings-row" key={item.clientId}>
            <div><strong>{item.displayName}</strong><p><code>{item.clientId}</code></p></div>
            {item.revokedAtMs !== null ? <span>{t("Revoked")}</span> : <div className="dialog-actions">
                <Button type="button" variant="text" onClick={() => review({ kind: "issue", target: item, key: crypto.randomUUID() })}>{t("Issue credential")}</Button>
                <Button type="button" variant="text" onClick={() => review({ kind: "revoke", target: item, key: crypto.randomUUID() })}>{t("Revoke client")}</Button>
            </div>}
        </div>)}
        {cursor && <Button type="button" disabled={loading} onClick={() => void load(cursor)}>{t("Load more")}</Button>}
        {change && <Dialog open dismissible={!writing} title={change.kind === "create" ? t("Register client") : change.kind === "issue" ? t("Issue credential") : t("Revoke client")}
            onClose={() => setChange(undefined)}>
            <strong>{change.kind === "create" ? change.displayName : change.target.displayName}</strong>
            {change.kind !== "create" && <p><code>{change.target.clientId}</code></p>}
            {change.kind === "issue" && <p>{t("Issuing a credential replaces this client’s current credential.")}</p>}
            {change.kind === "revoke" && <p>{t("Revoking this client permanently ends its access.")}</p>}
            {writeError && <p role="alert">{t(requestErrorMessage(writeError))}</p>}
            <div className="dialog-actions"><DialogCloseButton disabled={writing}>{t("Cancel")}</DialogCloseButton>
                <Button type="button" variant="text" disabled={writing} onClick={() => void apply(true)}>{t("Submit for approval")}</Button>
                <Button disabled={writing} onClick={() => void apply()}>{t("Confirm")}</Button></div>
        </Dialog>}
        {secret && <Dialog open title={t("One-time credential")} onClose={() => setSecret(undefined)}>
            <p>{t("Copy this credential to the AI client before closing.")}</p><code>{secret}</code>
            <DialogCloseButton>{t("Close")}</DialogCloseButton>
        </Dialog>}
        <AiManagementRequests client={client} revision={revision} showSecret={setSecret}
            changed={() => { setRevision(value => value + 1); void load(); }} />
        <AiOAuthSettings client={client} revision={revision} showSecret={setSecret}
            changed={() => setRevision(value => value + 1)} />
        <AiConnectionActivity client={client} />
    </section>;
}
