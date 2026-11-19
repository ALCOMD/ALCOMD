import type { ProjectInventoryResult, RpcError } from "@alcomd/sdk";
import { useEffect, useState } from "react";

import { DesktopList } from "./DesktopList";
import { Button, Dialog, DialogCloseButton, type DialogOrigin } from "./Material";
import { PagedItems } from "./PagedItems";
import { useTranslation } from "./i18n";
import { normalizeRequestError, requestErrorMessage } from "./request-errors";
import type { GuiRpcClient } from "./rpc";

const evidenceLabels = {
    matched: "Matches locked version",
    missing: "Package files missing",
    version_mismatch: "Files differ from locked version",
    unknown: "Could not verify package files"
} as const;

const resolutionLabels = {
    needed: "Package resolution is needed",
    no_issue_observed: "No package issues found",
    unknown: "Package resolution status is unknown"
} as const;

export function ProjectInventoryDialog({ client, projectId, revision, open, onClose, origin }: {
    client: GuiRpcClient; projectId: string; revision: number; open: boolean; onClose(): void; origin: DialogOrigin;
}) {
    const { t } = useTranslation();
    const [page, setPage] = useState<ProjectInventoryResult>();
    const [error, setError] = useState<RpcError>();
    const [retry, setRetry] = useState(0);
    useEffect(() => {
        if (!open) return;
        let active = true;
        setPage(undefined);
        setError(undefined);
        void client.projectInventory({ projectId, expectedRevision: revision }).then((result) => {
            if (active) setPage(result);
        }).catch((caught: unknown) => { if (active) setError(normalizeRequestError(caught)); });
        return () => { active = false; };
    }, [client, open, projectId, revision, retry]);

    return <Dialog open={open} onClose={onClose} returnFocus={origin} title={t("Installed packages")} wide
            actions={<DialogCloseButton variant="text">{t("Close")}</DialogCloseButton>}>
            {!open ? null : error !== undefined ? <>
                <p role="alert">{t(requestErrorMessage(error.code))}</p>
                <Button onClick={() => setRetry((current) => current + 1)} type="button" variant="text">{t("Retry")}</Button>
            </> : page === undefined ? <p role="status">{t("Loading…")}</p> : <>
                <p>{t(resolutionLabels[page.resolution])}</p>
                <PagedItems initialItems={page.entries} initialCursor={page.nextCursor ?? undefined} loadMore={async (cursor) => {
                    const next = await client.projectInventory({ projectId, expectedRevision: revision, cursor });
                    return { items: next.entries, nextCursor: next.nextCursor ?? undefined };
                }}>
                    {(items, pagination) => <>
                        {items.length === 0 ? <p>{t("No packages in the project manifest")}</p> : <DesktopList minWidth={570}
                            label={t("Installed packages")}
                            columns={[
                                { key: "package", header: t("Package ID") },
                                { key: "requested", header: t("Requested version") },
                                { key: "locked", header: t("Locked version") },
                                { key: "found", header: t("Detected version") },
                                { key: "state", header: t("Status") }
                            ]}
                            rows={items.map((item) => ({ key: item.packageId, cells: [
                                item.packageId, item.declaredRange ?? "—", item.lockedVersion ?? "—", item.observedVersion ?? "—",
                                t(evidenceLabels[item.state])
                            ] }))} />}
                        {pagination.error === undefined ? null : <>
                            <p role="alert">{t(requestErrorMessage(pagination.error.code))}</p>
                            <Button onClick={() => setRetry((current) => current + 1)} type="button" variant="text">{t("Refresh")}</Button>
                        </>}
                    </>}
                </PagedItems>
            </>}
    </Dialog>;
}
