import { useCallback, useState } from "react";
import { createRoot } from "react-dom/client";

import { Button, TextField } from "../../src/Material";
import { UtilityWorkspace } from "../../src/UtilityWorkspace";
import "../../src/styles.css";

declare global {
    interface Window {
        workspaceReads: { route: string; resolve(value: string): void; reject(error: unknown): void }[];
    }
}

window.workspaceReads = [];

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

createRoot(document.getElementById("root")!).render(<Evidence />);
