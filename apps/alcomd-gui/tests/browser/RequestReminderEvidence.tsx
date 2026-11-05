import { useEffect, useState } from "react";
import { applyAppearance, defaultAppearance } from "@alcomd/ui";
import { Button, Dialog } from "../../src/Material";
import { LocaleProvider, type SupportedLocale } from "../../src/i18n";
import { Reminder, RequestReminder } from "../../src/Reminder";
export function RequestReminderEvidence({ locale }: { locale: SupportedLocale }) {
    const [activityUnavailable, setActivityUnavailable] = useState(false);
    const [code, setCode] = useState("extension_publisher_confirmation_required");
    return <LocaleProvider locale={locale}>
        <RequestReminder error={{ code, message: "https://private.invalid/?token=do-not-display", diagnosticId: "test-diagnostic", data: activityUnavailable ? { activityRecordingUnavailable: true } : undefined }}>
            <Button onClick={() => setCode("extension_signature_invalid")}>Change error</Button>
        </RequestReminder>
        <Button onClick={() => setCode("unrecognized_future_error")}>Unknown error</Button>
        <Button onClick={() => setActivityUnavailable(true)}>Activity recording unavailable</Button>
    </LocaleProvider>;
}
export function ReminderPresentationEvidence({ mode }: { mode: "light" | "dark" }) {
    const [open, setOpen] = useState(false);
    const [failed, setFailed] = useState(true);
    const [visible, setVisible] = useState(true);
    const [modalFailed, setModalFailed] = useState(false);
    const [renders, setRenders] = useState(0);
    useEffect(() => applyAppearance(document.documentElement, { ...defaultAppearance, mode }), [mode]);
    return <LocaleProvider locale="en-US"><main>
        {visible ? <RequestReminder error={{ code: "permission_denied", message: "", diagnosticId: "00000000-0000-4000-8000-000000000001" }} title="Failed request" onDismiss={() => setVisible(false)} /> : null}
        <Reminder tone="warning">Required review information</Reminder>
        <Button onClick={() => setRenders((v) => v + 1)}>Render again</Button>
        <Button onClick={() => setOpen(true)}>Open modal</Button>
        <Button onClick={() => { setFailed(true); setVisible(true); }}>Retry failure</Button>
        <output aria-label="Failure state">{String(failed)}</output><output aria-label="Render count">{renders}</output>
        <Dialog open={open} onClose={() => setOpen(false)} title="Contextual feedback evidence">
            <Button onClick={() => setModalFailed(true)}>Fail inside modal</Button>
            {modalFailed ? <RequestReminder error={{ code: "revision_conflict", message: "" }} title="Modal failure" /> : null}
            <Button onClick={() => setOpen(false)}>Close modal</Button>
        </Dialog>
    </main></LocaleProvider>;
}
