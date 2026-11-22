import { useState } from "react";
import type { PlansGetParams, PlansGetResult } from "@alcomd/sdk";
import { useActionScope } from "./CoreActions";
import { Button, Dialog, DialogCloseButton, Select, TextField, type DialogOrigin } from "./Material";
import { capabilities, useCapability } from "./capabilities";
import { useTranslation } from "./i18n";
import { normalizeRequestError, requestErrorMessage } from "./request-errors";
import type { GuiRpcClient } from "./rpc";

export function FrozenPlanDialog({ client, origin, onClose }: { client: GuiRpcClient; origin: DialogOrigin; onClose(): void }) {
    const { t } = useTranslation();
    const current = useActionScope(client);
    const extensionLifecycle = useCapability(capabilities.extensionsLifecycle);
    const options = [
        { value: "package", label: t("Package"), disabled: !useCapability(capabilities.packagesPlan) },
        { value: "project_copy", label: t("Copy project"), disabled: !useCapability(capabilities.projectsCopy) },
        { value: "project_delete_directory", label: t("Project directory deletion"), disabled: !useCapability(capabilities.projectsDelete) },
        { value: "project_unity_migration", label: t("Unity migration"), disabled: !useCapability(capabilities.projectsUnityMigration) },
        { value: "extension_install", label: t("Install extension"), disabled: !extensionLifecycle },
        { value: "extension_uninstall", label: t("Uninstall extension"), disabled: !extensionLifecycle },
        { value: "repositoryRegistration", label: t("Add repository"), disabled: !useCapability(capabilities.repositoriesRead) },
    ] as const;
    const [kind, setKind] = useState<PlansGetParams["kind"]>(options.find((option) => !option.disabled)?.value ?? "package");
    const [id, setId] = useState("");
    const [busy, setBusy] = useState(false);
    const [plan, setPlan] = useState<PlansGetResult>();
    const [error, setError] = useState<string>();

    function clearResult() {
        setPlan(undefined);
        setError(undefined);
    }

    async function find() {
        if (busy || !id.trim()) return;
        setBusy(true);
        clearResult();
        try {
            const result = await client.getPlan({ kind, planId: id.trim() });
            if (current()) setPlan(result);
        } catch (cause) {
            if (current()) setError(requestErrorMessage(normalizeRequestError(cause).code));
        } finally {
            if (current()) setBusy(false);
        }
    }

    return <Dialog open title={t("Look up plan")} onClose={onClose} returnFocus={origin} actions={<>
        <DialogCloseButton variant="text">{t("Close")}</DialogCloseButton>
        <Button type="button" disabled={busy || !id.trim() || options.some((option) => option.value === kind && option.disabled)} onClick={() => void find()}>{t("Look up plan")}</Button>
    </>}>
        <Select label={t("Plan kind")} options={options} value={kind} disabled={busy} onChange={(value) => {
            const selected = options.find((option) => option.value === value && !option.disabled);
            if (selected) { setKind(selected.value); clearResult(); }
        }} />
        <TextField label={t("Plan ID")} value={id} disabled={busy} onInput={(value) => { setId(value); clearResult(); }} />
        {error && <p role="alert">{t(error)}</p>}
        {plan && <pre role="status">{JSON.stringify(plan.plan, null, 4)}</pre>}
    </Dialog>;
}
