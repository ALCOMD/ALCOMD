import { useMemo, useState } from "react";
import { FilePicker } from "../../src/FilePicker";
import { Button } from "../../src/Material";
import type { FileDrag } from "../../src/rpc";

export function FilePickerEvidence() {
    const [path, setPath] = useState("C:/fixture/typed.txt");
    const [disabled, setDisabled] = useState(false);
    const [visible, setVisible] = useState(true);
    const [choosing, setChoosing] = useState(false);
    const [requests, setRequests] = useState(0);
    const client = useMemo(() => ({
        async subscribeFileDrag(receive: (event: FileDrag) => void) {
            const listener = (event: Event) => receive((event as CustomEvent<FileDrag>).detail);
            window.addEventListener("fixture-file-drag", listener);
            return () => window.removeEventListener("fixture-file-drag", listener);
        }
    }), []);
    const choosePath = () => {
        setRequests((value) => value + 1);
        return new Promise<string | undefined>((resolve, reject) => {
            window.addEventListener("fixture-selection-result", (event) => {
                const result = (event as CustomEvent<{ path?: string; error?: boolean }>).detail;
                if (result.error) reject(new Error("Fixture selection failure"));
                else resolve(result.path);
            }, { once: true });
        });
    };
    return <main className="material-evidence">
        <h1>File picker evidence</h1>
        <Button onClick={() => setDisabled((value) => !value)}>Toggle disabled</Button>
        <Button onClick={() => setVisible((value) => !value)}>Toggle mounted</Button>
        <output data-testid="picker-requests">{requests}</output>
        <output data-testid="picker-choosing">{String(choosing)}</output>
        <output data-testid="picker-path">{path}</output>
        {visible ? <FilePicker client={client} kind="file" label="Evidence path" disabled={disabled} value={path}
            choosePath={choosePath} onPathChange={setPath} onChoosingChange={setChoosing} /> : null}
    </main>;
}
