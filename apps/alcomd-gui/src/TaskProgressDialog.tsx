import { useEffect, useState } from "react";
import type { CoreEvents, AcceptedGuiTask } from "./core-events";
import type { Operation } from "./core-models";
import { OperationActions, OperationFollow, OperationProgress } from "./CoreActions";
import { DialogCloseButton, Dialog } from "./Material";
import { useTranslation } from "./i18n";

/** Presentation only: Core retains task state and history after this window is minimized. */
export function TaskProgressDialog({ events, navigate }: { events: CoreEvents; navigate(path: string): void }) {
    const [accepted, setAccepted] = useState<{ events: CoreEvents; task: AcceptedGuiTask }>();
    useEffect(() => {
        return events.subscribeAcceptedTasks((task) => setAccepted({ events, task }));
    }, [events]);
    if (accepted?.events !== events) return null;
    return <AcceptedTaskDialog events={events} task={accepted.task} key={accepted.task.operationId} navigate={navigate} dismiss={() => setAccepted(undefined)} />;
}

function AcceptedTaskDialog({ events, task, navigate, dismiss }: { events: CoreEvents; task: AcceptedGuiTask; navigate(path: string): void; dismiss(): void }) {
    const { t } = useTranslation();
    const [snapshot, setSnapshot] = useState<Operation>();
    const terminal = snapshot !== undefined && ["succeeded", "failed", "cancelled"].includes(snapshot.state);
    return <Dialog open onClose={dismiss} title={t(task.title)} actions={<>
        <DialogCloseButton afterClose={() => { dismiss(); navigate(`/operations/${task.operationId}`); }} variant="text">{t("View operation")}</DialogCloseButton>
        {snapshot?.state !== "succeeded" || task.resultPath === undefined ? null : <DialogCloseButton afterClose={() => { dismiss(); navigate(task.resultPath!); }} variant="tonal">{t(task.resultLabel!)}</DialogCloseButton>}
        <DialogCloseButton>{t(terminal ? "Close" : "Minimize")}</DialogCloseButton>
    </>}><OperationFollow client={events.client} operationId={task.operationId} onSnapshot={setSnapshot}>
        {(operation, error) => {
            return <>
                <OperationProgress error={error} operation={operation} title={task.title} />
                {operation?.state === "succeeded" ? <p>{t("Task completed.")}</p> : terminal ? null : <p>{t("You can minimize this window and follow progress in Tasks.")}</p>}
                {operation === undefined || terminal ? null : <OperationActions client={events.client} operation={operation} onChanged={() => events.invalidate(`operation:${task.operationId}`, true)} />}
            </>;
        }}
    </OperationFollow></Dialog>;
}
