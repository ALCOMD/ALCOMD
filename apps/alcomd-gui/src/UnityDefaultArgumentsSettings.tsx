import { useRef, useState, useSyncExternalStore } from "react";
import type { RpcError } from "@alcomd/sdk";
import type { SettingsGetResult } from "./core-models";
import { Button, Dialog } from "./Material";
import { capabilities, useCapability } from "./capabilities";
import { useTranslation } from "./i18n";
import { Reminder, RequestReminder } from "./Reminder";
import { normalizeRequestError } from "./request-errors";
import { SettingsAutosave, UnityArgumentsConflictError } from "./settings-autosave";
import { UnityArgumentsEditor } from "./UnityArgumentsEditor";
import { SettingsResetButton } from "./SettingsResetButton";

export function UnityDefaultArgumentsSettings({ settingsWriter, snapshot }: { settingsWriter: SettingsAutosave; snapshot: SettingsGetResult }) {
    const { t } = useTranslation();
    const saving = useSyncExternalStore(settingsWriter.subscribe, settingsWriter.getSnapshot);
    const accepted = saving.accepted ?? snapshot;
    const canManage = useCapability(capabilities.unityManage);
    const [editing, setEditing] = useState(false);
    const arguments_ = accepted.settings.unity.defaultLaunchArguments;
    return <section className="settings-section" aria-label={t("Default Unity launch arguments")}>
        <header className="settings-section-heading"><h2 tabIndex={-1}>{t("Default Unity launch arguments")}</h2>
            <div className="action-row">
                <Button disabled={!canManage || saving.saving} onClick={() => setEditing(true)} type="button" variant="text">{t("Edit")}</Button>
                <SettingsResetButton title={t("Default Unity launch arguments")} disabled={!canManage || saving.saving || saving.failed || editing || arguments_ === null}
                    onRestore={() => settingsWriter.restoreDefaults({ unity: { defaultLaunchArguments: null } }, { unity: { defaultLaunchArguments: accepted.settings.unity.defaultLaunchArguments } })} />
            </div>
        </header>
        <div className="settings-section-content">
            <p className="field-hint">{t("Used by projects without custom launch arguments.")}</p>
            <p>{arguments_ === null || arguments_.length === 0 ? t("No additional arguments.") : t("{count} arguments", { count: arguments_.length })}</p>
        </div>
        {editing ? <DefaultArgumentsDialog settingsWriter={settingsWriter} initial={arguments_} onClose={() => setEditing(false)} /> : null}
    </section>;
}

function DefaultArgumentsDialog({ settingsWriter, initial, onClose }: { settingsWriter: SettingsAutosave; initial: string[] | null; onClose(): void }) {
    const { t } = useTranslation();

    const canManage = useCapability(capabilities.unityManage);
    const [open, setOpen] = useState(true);
    const baseline = useRef(structuredClone(initial)).current;
    const [draft, setDraft] = useState<string[] | null>(() => structuredClone(initial));
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState<RpcError>();
    const [unknown, setUnknown] = useState(false);
    const [conflict, setConflict] = useState(false);
    const pending = useRef(false);
    const close = () => { if (!pending.current) setOpen(false); };
    const finish = () => {
        setOpen(false);

    };
    const save = async (desired: string[] | null) => {
        if (!canManage || pending.current || conflict) return;
        pending.current = true; setBusy(true); setError(undefined); setUnknown(false);
        try {
            await settingsWriter.saveUnityArguments(desired, baseline);
            finish();
        } catch (caught: unknown) {
            if (caught instanceof UnityArgumentsConflictError) { setConflict(true); return; }
            // A lost response can follow a successful save. Check before offering another write.
            try {
                const current = await settingsWriter.reload();
                if (JSON.stringify(current.settings.unity.defaultLaunchArguments) === JSON.stringify(desired)) { finish(); return; }
                if (JSON.stringify(current.settings.unity.defaultLaunchArguments) !== JSON.stringify(baseline)) { setConflict(true); return; }
            } catch { setUnknown(true); }
            setError(normalizeRequestError(caught));
        } finally { pending.current = false; setBusy(false); }
    };
    return <Dialog open={open} onClose={onClose} dismissible={!busy} title={t("Default Unity launch arguments")}>
        <form onSubmit={(event) => { event.preventDefault(); void save(draft); }}>
            <UnityArgumentsEditor id="default-unity-argument" value={draft ?? []} onChange={(next) => { setDraft(next); setError(undefined); }} disabled={busy || !canManage || conflict} />
            {conflict ? <Reminder tone="error">{t("These arguments changed elsewhere. Close and reopen this editor to load the current values.")}</Reminder> : error === undefined ? null
                : <RequestReminder error={error} title={unknown ? t("Launch argument save result is unknown") : t("Launch arguments could not be saved.")}
                    message={unknown ? "Retry checks the current settings before writing again." : undefined} />}
            <div className="dialog-actions">
                <Button disabled={busy} onClick={close} type="button" variant="text">{t("Cancel")}</Button>
                <Button disabled={busy || !canManage || conflict || JSON.stringify(draft) === JSON.stringify(baseline)} type="submit">{t(error === undefined ? "Save launch arguments" : "Retry")}</Button>
            </div>
        </form>
    </Dialog>;
}
