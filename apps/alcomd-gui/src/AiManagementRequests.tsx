import { useCallback, useEffect, useRef, useState } from "react";
import { Button, Dialog, DialogCloseButton } from "./Material";
import { useTranslation } from "./i18n";
import { normalizeRequestError, requestErrorMessage } from "./request-errors";
import type { GuiRpcClient } from "./rpc";

type Review = Awaited<ReturnType<GuiRpcClient["aiRequestGet"]>>;
type Choice = Parameters<GuiRpcClient["aiRequestDecide"]>[0];

export function AiManagementRequests({ client, revision, showSecret, changed }: {
    client: GuiRpcClient; revision: number; showSecret(value: string): void; changed(): void;
}) {
    const { t } = useTranslation();
    const [items, setItems] = useState<Review[]>([]);
    const [cursor, setCursor] = useState<string | null>(null);
    const [review, setReview] = useState<Review>();
    const [choice, setChoice] = useState<Choice>();
    const [error, setError] = useState<string>();
    const [busy, setBusy] = useState(false);
    const seq = useRef(0);
    const load = useCallback(async (after?: string) => {
        const n = ++seq.current;
        setError(undefined);
        try {
            const page = await client.aiRequestList(after ? { cursor: after } : {});
            if (seq.current !== n) return;
            setItems(old => after ? [...old, ...page.items] : page.items);
            setCursor(page.nextCursor);
        } catch (caught) { if (seq.current === n) setError(normalizeRequestError(caught).code); }
    }, [client]);
    useEffect(() => { void load(); return () => { ++seq.current; }; }, [load, revision]);
    async function open(item: Review) {
        setBusy(true);
        setError(undefined);
        try { setReview(await client.aiRequestGet({requestId:item.requestId})); setChoice(undefined); }
        catch (caught) { setError(normalizeRequestError(caught).code); }
        finally { setBusy(false); }
    }
    async function decide(approve: boolean) {
        if (!review || busy) return;
        const params = choice?.approve === approve ? choice : {
            requestId: review.requestId, expectedFingerprint: review.fingerprint, approve, idempotencyKey: crypto.randomUUID(),
        };
        setChoice(params);
        setBusy(true);
        setError(undefined);
        try {
            const result = await client.aiRequestDecide(params);
            setReview(undefined);
            const secret = result.credential ?? result.clientSecret;
            if (secret) showSecret(secret);
            changed();
        } catch (caught) { setError(normalizeRequestError(caught).code); }
        finally { setBusy(false); }
    }
    return <section><h3>{t("Client management requests")}</h3>
        <Button type="button" variant="text" disabled={busy} onClick={() => void load()}>{t("Refresh list")}</Button>
        {error && !review && <p role="alert">{t(requestErrorMessage(error))}</p>}
        {items.map(item => <div className="settings-row" key={item.requestId}>
            <div><strong>{t(actionTitle(item.action))}</strong><p>{item.requester} · {t(statusTitle(item.status))}</p></div>
            <Button type="button" variant="text" disabled={busy} onClick={() => void open(item)}>{t("Review request")}</Button>
        </div>)}
        {cursor && <Button type="button" disabled={busy} onClick={() => void load(cursor)}>{t("Load more")}</Button>}
        {review && <Dialog open dismissible={!busy} onClose={() => setReview(undefined)} title={t(actionTitle(review.action))}>
            <p>{t("Requested by")} <code>{review.requester}</code></p>
            <p>{t("Expires")} <time>{new Date(review.expiresAtMs).toLocaleString()}</time></p>
            <ActionDetails action={review.action} />
            {error && <p role="alert">{t(requestErrorMessage(error))}</p>}
            <div className="dialog-actions"><DialogCloseButton disabled={busy}>{t("Close")}</DialogCloseButton>
                {review.status === "pending" && <>
                    <Button type="button" variant="text" disabled={busy} onClick={() => void decide(false)}>{t("Deny request")}</Button>
                    <Button type="button" disabled={busy} onClick={() => void decide(true)}>{t("Approve request")}</Button>
                </>}
            </div>
        </Dialog>}
    </section>;
}

export function actionTitle(action: Review["action"]) {
    switch (action.kind) {
        case "createClient": return "Register client";
        case "issueCredential": return "Issue credential";
        case "revokeClient": return "Revoke client";
        case "setOAuthRegistration": return "Set OAuth registration";
        case "revokeOAuthRegistration": return "Revoke OAuth registration";
    }
}
function statusTitle(status: Review["status"]) {
    return { pending: "Pending", approved: "Approved", denied: "Denied", expired: "Expired" }[status];
}
export function ActionDetails({action}: {action: Review["action"]}) {
    const { t } = useTranslation();
    if (action.kind === "createClient") return <p>{action.displayName}</p>;
    return <>
        <p>{t("Client ID")} <code>{"clientId" in action ? action.clientId : action.oauthClientId}</code></p>
        <p>{t("Revision")} {action.expectedRevision}</p>
        {action.kind === "issueCredential" && <p>{t("Issuing a credential replaces this client’s current credential.")}</p>}
        {action.kind === "revokeClient" && <p>{t("Revoking this client permanently ends its access.")}</p>}
        {action.kind === "setOAuthRegistration" && <>
            <p>{t("Issuer")} <code>{action.issuer}</code></p><p>{t("Resource")} <code>{action.resource}</code></p>
            <p>{t("Redirect URIs")}</p><ul>{action.redirectUris.map(uri => <li key={uri}><code>{uri}</code></li>)}</ul>
            <p>{t("Confidential client")} · {t(action.confidential ? "Yes" : "No")}</p>
            <p>{t("Allow refresh tokens")} · {t(action.allowRefresh ? "Yes" : "No")}</p>
        </>}
    </>;
}
