import { useEffect, useState } from "react";
import { applyAppearance, defaultAppearance } from "@alcomd/ui";
import type { RpcError } from "@alcomd/sdk";
import { Button, Dialog } from "../../src/Material";
import { LocaleProvider, type SupportedLocale } from "../../src/i18n";
import { GlobalReminder, Reminder, ReminderProvider, RequestReminder, useReminder } from "../../src/Reminder";

export function RequestReminderEvidence({ locale }: { locale: SupportedLocale }) {
    const [code, setCode] = useState("extension_publisher_confirmation_required");
    return <LocaleProvider locale={locale}><ReminderProvider visit={0}>
        <RequestReminder error={{ code, message: "https://private.invalid/?token=do-not-display", diagnosticId: "test-diagnostic" }}>
            <button type="button" onClick={() => setCode("extension_signature_invalid")}>Change error</button>
        </RequestReminder>
        <button type="button" onClick={() => setCode("unrecognized_future_error")}>Unknown error</button>
    </ReminderProvider></LocaleProvider>;
}

export function ReminderPresentationEvidence({ mode }: { mode: "light" | "dark" }) {
    const makeFailure = (): RpcError => ({ code: "permission_denied", message: "", diagnosticId: "00000000-0000-4000-8000-000000000001" });
    const [failure, setFailure] = useState(makeFailure);
    const [shellFailure] = useState(makeFailure);
    const [renders, setRenders] = useState(0);
    const [open, setOpen] = useState(false);
    useEffect(() => applyAppearance(document.documentElement, { ...defaultAppearance, mode }), [mode]);
    return <LocaleProvider locale="en-US"><ReminderProvider visit={0} feedback={<div className="settings-save-error"><RequestReminder error={shellFailure} title="Shell failure" /></div>}>
        <ReminderActionsEvidence failure={failure} />
        <Button onClick={() => setRenders((value) => value + 1)}>Render again</Button>
        <Button onClick={() => setOpen(true)}>Open modal</Button>
        <output aria-label="Failure state">{failure.code}</output><output aria-label="Render count">{renders}</output>
        {(["success", "info", "warning", "error"] as const).map((tone) => <GlobalReminder key={tone}><Reminder title="Notification" tone={tone}>Common notification</Reminder></GlobalReminder>)}
        <RequestReminder error={failure} title="Failed request" />
        <div className="inline-evidence"><Reminder tone="warning">Required review information</Reminder></div>
        <Dialog open={open} onClose={() => setOpen(false)} title="Notification evidence">
            <Button onClick={() => setFailure(makeFailure())}>Fail inside modal</Button>
            <Button onClick={() => setOpen(false)}>Close modal</Button>
        </Dialog>
    </ReminderProvider></LocaleProvider>;
}

function ReminderActionsEvidence({ failure }: { failure: RpcError }) {
    const reminder = useReminder();
    return <><Button onClick={() => reminder.success("Operation completed")}>Publish success</Button>
        <Button onClick={() => reminder.error(failure)}>Publish failure</Button></>;
}
