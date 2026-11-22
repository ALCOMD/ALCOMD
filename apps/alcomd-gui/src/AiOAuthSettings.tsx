import { useCallback, useEffect, useRef, useState } from "react";
import { Button, Checkbox, Dialog, DialogCloseButton, TextField } from "./Material";
import { useTranslation } from "./i18n";
import { normalizeRequestError, requestErrorMessage } from "./request-errors";
import type { GuiRpcClient } from "./rpc";
import { ActionDetails, actionTitle } from "./AiManagementRequests";

type Registration = Awaited<ReturnType<GuiRpcClient["aiOAuthList"]>>["items"][number];
type SetParams = Parameters<GuiRpcClient["aiOAuthSet"]>[0];
type Action = Extract<Parameters<GuiRpcClient["aiRequestCreate"]>[0]["action"], { kind: "setOAuthRegistration" | "revokeOAuthRegistration" }>;
type Vote = Awaited<ReturnType<GuiRpcClient["aiOAuthRequestGet"]>>;

export function AiOAuthSettings({ client, revision, showSecret, changed }: {
    client: GuiRpcClient; revision: number; showSecret(value: string): void; changed(): void;
}) {
    const { t } = useTranslation();
    const [items, setItems] = useState<Registration[]>([]);
    const [cursor, setCursor] = useState<string | null>();
    const [form, setForm] = useState<SetParams>();
    const [review, setReview] = useState<{ action: Action; key: string }>();
    const [requestId, setRequestId] = useState("");
    const [vote, setVote] = useState<Vote>();
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState<string>();
    const pendingVote = useRef<Parameters<GuiRpcClient["aiOAuthRequestDecide"]>[0] | undefined>(undefined);
    const seq = useRef(0);
    const load = useCallback(async (after?: string) => {
        const n = ++seq.current;
        try {
            const page = await client.aiOAuthList(after ? {cursor:after} : {});
            if (n !== seq.current) return;
            setItems(old => after ? [...old,...page.items] : page.items);
            setCursor(page.nextCursor);
        } catch (caught) { if (n === seq.current) setError(normalizeRequestError(caught).code); }
    }, [client]);
    useEffect(() => { void load(); return () => { ++seq.current; }; }, [load,revision]);
    async function perform(work: () => Promise<void>) {
        if (busy) return;
        setError(undefined);
        setBusy(true);
        try { await work(); }
        catch (caught) { setError(normalizeRequestError(caught).code); }
        finally { setBusy(false); }
    }
    function edit(item?: Registration) {
        setError(undefined);
        setForm({clientId:item?.clientId ?? "", expectedRevision:item?.revision ?? 0,
            issuer:item?.issuer ?? "", resource:item?.resource ?? "", redirectUris:item?.redirectUris ?? [""],
            confidential:item?.confidential ?? false, allowRefresh:item?.allowRefresh ?? false, idempotencyKey:crypto.randomUUID()});
    }
    async function submit(asRequest: boolean) {
        if (!review) return;
        const { action, key: idempotencyKey } = review;
        if (asRequest) await client.aiRequestCreate({action,idempotencyKey});
        else if (action.kind === "setOAuthRegistration") {
            const {kind: _, ...params} = action;
            const result = await client.aiOAuthSet({...params,idempotencyKey});
            if (result.clientSecret) showSecret(result.clientSecret);
        } else await client.aiOAuthRevoke({oauthClientId:action.oauthClientId,expectedRevision:action.expectedRevision,idempotencyKey});
        setReview(undefined);
        changed();
    }
    async function decide(approve: boolean) {
        if (!vote) return;
        const saved = pendingVote.current;
        const params = saved && saved.approve === approve ? saved : {
            requestId:vote.requestId,expectedFingerprint:vote.approvalFingerprint,approve,idempotencyKey:crypto.randomUUID(),
        };
        pendingVote.current = params;
        await client.aiOAuthRequestDecide(params);
        setVote(undefined);
    }
    const feedback = error && <p role="alert">{t(requestErrorMessage(error))}</p>;
    return <section><h3>{t("OAuth registrations")}</h3>
        <Button type="button" onClick={() => edit()}>{t("Set OAuth registration")}</Button>
        <Button type="button" variant="text" disabled={busy} onClick={() => void perform(() => load())}>{t("Refresh list")}</Button>
        {!review && !vote && !form && feedback}
        {items.map(item => <div className="settings-row" key={item.oauthClientId}><div><strong>{item.issuer}</strong>
            <p><code>{item.clientId}</code></p></div>
            {item.revokedAtMs == null ? <div className="dialog-actions">
                <Button type="button" variant="text" onClick={() => edit(item)}>{t("Edit")}</Button>
                <Button type="button" variant="text" onClick={() => { setError(undefined); setReview({
                    action:{kind:"revokeOAuthRegistration",oauthClientId:item.oauthClientId,expectedRevision:item.revision},key:crypto.randomUUID(),
                }); }}>{t("Revoke OAuth registration")}</Button>
            </div> : <span>{t("Revoked")}</span>}
        </div>)}
        {cursor && <Button type="button" disabled={busy} onClick={() => void perform(() => load(cursor))}>{t("Load more")}</Button>}
        <h3>{t("OAuth authorization requests")}</h3>
        <TextField label={t("Request ID")} value={requestId} onInput={setRequestId} />
        <Button type="button" disabled={busy || !requestId.trim()} onClick={() => void perform(async () => {
            setVote(await client.aiOAuthRequestGet({requestId:requestId.trim()})); pendingVote.current = undefined;
        })}>{t("Review request")}</Button>
        {form && <Dialog open title={t("Set OAuth registration")} onClose={() => setForm(undefined)}>
            <TextField label={t("Client ID")} value={form.clientId} readOnly={form.expectedRevision > 0} onInput={clientId => setForm({...form,clientId})} />
            <TextField label={t("Issuer")} value={form.issuer} onInput={issuer => setForm({...form,issuer,resource:issuer+"/mcp"})} />
            <TextField label={t("Resource")} readOnly value={form.resource} />
            <TextField label={t("Redirect URIs")} rows={3} value={form.redirectUris.join("\n")} onInput={value => setForm({...form,redirectUris:value.split("\n")})} />
            <Checkbox label={t("Confidential client")} checked={form.confidential} onChange={confidential => setForm({...form,confidential})} />
            <Checkbox label={t("Allow refresh tokens")} checked={form.allowRefresh} onChange={allowRefresh => setForm({...form,allowRefresh})} />
            <div className="dialog-actions"><DialogCloseButton>{t("Cancel")}</DialogCloseButton>
                <Button type="button" disabled={!form.clientId || !form.issuer || !form.redirectUris.some(uri => uri.trim())} onClick={() => {
                    const {idempotencyKey:key,...params} = form;
                    setReview({action:{kind:"setOAuthRegistration",...params,redirectUris:params.redirectUris.map(uri => uri.trim()).filter(Boolean)},key});
                    setForm(undefined);
                }}>{t("Review")}</Button></div>
        </Dialog>}
        {review && <Dialog open title={t(actionTitle(review.action))} dismissible={!busy} onClose={() => setReview(undefined)}>
            <ActionDetails action={review.action} />{feedback}
            <div className="dialog-actions"><DialogCloseButton disabled={busy}>{t("Cancel")}</DialogCloseButton>
                <Button type="button" variant="text" disabled={busy} onClick={() => void perform(() => submit(true))}>{t("Submit for approval")}</Button>
                <Button type="button" disabled={busy} onClick={() => void perform(() => submit(false))}>{t("Confirm")}</Button></div>
        </Dialog>}
        {vote && <Dialog open title={t("Review request")} dismissible={!busy} onClose={() => setVote(undefined)}>
            <h3>{vote.displayName}</h3><p><code>{vote.clientId}</code></p>
            <p>{t("Issuer")} <code>{vote.issuer}</code></p><p>{t("Resource")} <code>{vote.resource}</code></p>
            <p>{t("Redirect URIs")} <code>{vote.redirectUri}</code></p>
            <p>{t("OAuth scope")} <code>{vote.scopes.join(" ")}</code></p>
            <p>{t("Expires")} {new Date(vote.expiresAtMs).toLocaleString()}</p>{feedback}
            <div className="dialog-actions"><DialogCloseButton disabled={busy}>{t("Close")}</DialogCloseButton>
                {vote.status === "pending" && <>
                    <Button type="button" variant="text" disabled={busy} onClick={() => void perform(() => decide(false))}>{t("Deny request")}</Button>
                    <Button type="button" disabled={busy} onClick={() => void perform(() => decide(true))}>{t("Approve request")}</Button>
                </>}</div>
        </Dialog>}
    </section>;
}
