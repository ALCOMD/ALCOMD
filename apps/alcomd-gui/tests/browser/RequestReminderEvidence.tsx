import { useState } from "react";
import { LocaleProvider, type SupportedLocale } from "../../src/i18n";
import { ReminderProvider, RequestReminder } from "../../src/Reminder";

export function RequestReminderEvidence({ locale }: { locale: SupportedLocale }) {
    const [code, setCode] = useState("extension_publisher_confirmation_required");
    return <LocaleProvider locale={locale}><ReminderProvider visit={0}>
        <RequestReminder error={{ code, message: "https://private.invalid/?token=do-not-display", diagnosticId: "test-diagnostic" }}>
            <button type="button" onClick={() => setCode("extension_signature_invalid")}>Change error</button>
        </RequestReminder>
        <button type="button" onClick={() => setCode("unrecognized_future_error")}>Unknown error</button>
    </ReminderProvider></LocaleProvider>;
}
