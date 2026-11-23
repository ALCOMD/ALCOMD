import { useState } from "react";

import { useActionScope } from "./CoreActions";
import { FilePicker } from "./FilePicker";
import { Button, Checkbox, Dialog, DialogCloseButton, type DialogOrigin } from "./Material";
import type { ProjectSnapshot } from "./core-models";
import { displayPath } from "./display-path";
import { useTranslation } from "./i18n";
import { normalizeRequestError, requestErrorMessage } from "./request-errors";
import type { GuiRpcClient } from "./rpc";

export function ProjectInspectionDialog({ client, onClose, origin }: {
    client: GuiRpcClient;
    onClose(): void;
    origin: DialogOrigin;
}) {
    const { t } = useTranslation();
    const current = useActionScope(client);
    const [path, setPath] = useState("");
    const [parents, setParents] = useState(false);
    const [choosing, setChoosing] = useState(false);
    const [busy, setBusy] = useState(false);
    const [project, setProject] = useState<ProjectSnapshot>();
    const [error, setError] = useState<string>();

    async function inspect() {
        if (busy || choosing || !path.trim()) return;
        setBusy(true);
        setError(undefined);
        setProject(undefined);
        try {
            const result = await client.projectsInspect(path, parents ? "search-parents" : "exact-root");
            if (current()) setProject(result.project);
        } catch (cause) {
            if (current()) setError(requestErrorMessage(normalizeRequestError(cause).code));
        } finally {
            if (current()) setBusy(false);
        }
    }

    return <Dialog open onClose={onClose} returnFocus={origin} title={t("Inspect project")} actions={<>
        <DialogCloseButton variant="text">{t("Close")}</DialogCloseButton>
        <Button disabled={busy || choosing || !path.trim()} onClick={() => void inspect()} type="button">{t("Inspect project")}</Button>
    </>}>
        <FilePicker client={client} kind="directory" label={t("Project location")} value={path} disabled={busy}
            choosePath={() => client.selectDirectory(path || undefined)} onChoosingChange={setChoosing}
            onPathChange={(value) => { setPath(value); setError(undefined); setProject(undefined); }} />
        <Checkbox checked={parents} disabled={busy} label={t("Search parent folders")}
            onChange={(value) => { setParents(value); setError(undefined); setProject(undefined); }} />
        {error && <p role="alert">{t(error)}</p>}
        {project && <div role="status">
            <p title={displayPath(project.rootPath)}>{displayPath(project.rootPath)}</p>
            <p>{t("Unity version")} · {project.unityVersion}</p>
            <details><summary>{t("Details")}</summary><pre>{JSON.stringify(project, null, 4)}</pre></details>
        </div>}
    </Dialog>;
}
