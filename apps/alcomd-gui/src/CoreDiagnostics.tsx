import { useRef, useState } from "react";
import type { CoreEvent } from "@alcomd/sdk";
import { Button, Dialog, DialogCloseButton } from "./Material";
import { useActionScope } from "./CoreActions";
import { capabilities, useCapability } from "./capabilities";
import { useGuiWrite } from "./gui-write-attempt";
import { useTranslation } from "./i18n";
import { normalizeRequestError, requestErrorMessage } from "./request-errors";
import type { GuiRpcClient } from "./rpc";

const PAGE_SIZE = 50;

export function CoreDiagnostics({ client, navigate }: { client: GuiRpcClient; navigate(path: string): void }) {
    const { t } = useTranslation();
    const check = useCapability(capabilities.stateCheck);
    const replay = useCapability(capabilities.eventsReplay);
    const write = useGuiWrite(client);
    const current = useActionScope(client);
    const [open, setOpen] = useState(false);
    const [busy, setBusy] = useState(false);
    const [rows, setRows] = useState<CoreEvent[]>([]);
    const [end, setEnd] = useState(false);
    const [error, setError] = useState<string>();
    const pending = useRef(false);
    const cursor = useRef(0);

    async function perform(action: () => Promise<void>) {
        if (pending.current) return;
        pending.current = true;
        setBusy(true);
        setError(undefined);
        try {
            await action();
        } catch (caught) {
            if (current()) setError(normalizeRequestError(caught).code);
        } finally {
            pending.current = false;
            if (current()) setBusy(false);
        }
    }
    async function readEvents(first: boolean) {
        if (first) {
            setRows([]);
            setOpen(true);
            setEnd(false);
            cursor.current = 0;
        }
        const result = await client.eventsList(cursor.current, PAGE_SIZE);
        if (!current()) return;
        setRows(previous => first ? result.events : [...previous, ...result.events]);
        cursor.current = result.nextSequence;
        setEnd(result.events.length < PAGE_SIZE);
    }
    const feedback = error && <p role="alert">{t(requestErrorMessage(error))}</p>;
    return <>
        {check && <Button type="button" variant="text" disabled={busy} onClick={() => void perform(async () => {
            const operation = await write("stateCheck");
            if (current()) navigate(`/operations/${operation.operationId}`);
        })}>{t("Check Core state")}</Button>}
        {replay && <Button type="button" variant="text" disabled={busy} onClick={() => void perform(() => readEvents(true))}>{t("Core events")}</Button>}
        {!open && feedback}
        {open && <Dialog open title={t("Core events")} dismissible={!busy} onClose={() => setOpen(false)}
            actions={<DialogCloseButton disabled={busy}>{t("Close")}</DialogCloseButton>}>
            <ol>{rows.map(row => <li key={row.sequence}><strong>{row.kind}</strong> · <code>{row.aggregateId}</code> · {row.sequence}</li>)}</ol>
            {feedback}
            {end ? <p>{t("No further events")}</p> : <Button type="button" disabled={busy} onClick={() => void perform(() => readEvents(false))}>{t("Load more")}</Button>}
        </Dialog>}
    </>;
}
