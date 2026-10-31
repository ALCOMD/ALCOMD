import { useCallback, useMemo, useState } from "react";
import { createRoot } from "react-dom/client";

import { Button, TextField } from "../../src/Material";
import { UtilityWorkspace } from "../../src/UtilityWorkspace";
import { OperationFollow } from "../../src/CoreActions";
import type { Operation } from "../../src/core-models";
import "../../src/styles.css";

declare global {
    interface Window {
        workspaceReads: { route: string; resolve(value: string): void; reject(error: unknown): void }[];
        followReads: { client: number; operationId: string; resolve(value: Operation): void; reject(error: unknown): void }[];
        followTerminals: Operation[];
    }
}

window.workspaceReads = [];
window.followReads = [];
window.followTerminals = [];

function Draft() {
    const [value, setValue] = useState("");
    return <TextField label="Draft" value={value} onInput={setValue} />;
}

function Evidence() {
    const [route, setRoute] = useState("first");
    const load = useCallback(() => new Promise<string>((resolve, reject) => {
        window.workspaceReads.push({ route, resolve, reject });
    }), [route]);
    return <main>
        <Button onClick={() => setRoute("second")}>Change route</Button>
        <UtilityWorkspace title="Evidence" load={load} action={{ label: "Edit", render: () => <Draft /> }}>
            {(value, refresh) => <><p aria-label="Snapshot">{value}</p><Button onClick={() => { refresh(); refresh(); }}>Mutation completed</Button></>}
        </UtilityWorkspace>
    </main>;
}

function FollowEvidence() {
    const [clientIndex, setClientIndex] = useState(0);
    const [operationId, setOperationId] = useState("00000000-0000-4000-8000-000000000501");
    const [visible, setVisible] = useState(true);
    const client = useMemo(() => ({ operationGet: (id: string) => new Promise<Operation>((resolve, reject) => {
        window.followReads.push({ client: clientIndex, operationId: id, resolve, reject });
    }) }), [clientIndex]);
    return <main>
        <Button onClick={() => setClientIndex((current) => current + 1)}>Switch client</Button>
        <Button onClick={() => setOperationId("00000000-0000-4000-8000-000000000502")}>Switch task</Button>
        <Button onClick={() => setVisible(false)}>Leave page</Button>
        {visible ? <OperationFollow client={client} onTerminal={(operation) => window.followTerminals.push(operation)} operationId={operationId} /> : null}
    </main>;
}

createRoot(document.getElementById("root")!).render(new URLSearchParams(window.location.search).has("operationFollow") ? <FollowEvidence /> : <Evidence />);
