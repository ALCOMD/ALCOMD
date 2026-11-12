import { useRef, useState } from "react";
import { Button, LoadingButton, RefreshButton } from "../../src/Material";

export function RefreshFeedbackEvidence() {
    const [calls, setCalls] = useState(0);
    const [completed, setCompleted] = useState(0);
    const [failed, setFailed] = useState(false);
    const [visible, setVisible] = useState(true);
    const [controlled, setControlled] = useState(false);
    const release = useRef<(() => void) | undefined>(undefined);
    const immediate = () => {
        setCalls((value) => value + 1);
        setCompleted((value) => value + 1);
        return Promise.resolve();
    };
    const held = () => {
        setCalls((value) => value + 1);
        return new Promise<void>((resolve) => { release.current = resolve; })
            .then(() => setCompleted((value) => value + 1));
    };
    const failure = async () => {
        setCalls((value) => value + 1);
        try { await Promise.reject(new Error("fixture refresh failure")); }
        catch { setFailed(true); }
    };
    return <main className="material-evidence">
        <h1>Refresh feedback</h1>
        <div className="action-row">
            <RefreshButton data-testid="immediate-refresh" label="Immediate refresh" onClick={immediate} />
            <RefreshButton data-testid="disabled-refresh" label="Disabled refresh" disabled onClick={immediate} />
            <RefreshButton data-testid="controlled-refresh" label="Controlled refresh" refreshing={controlled} onClick={() => {
                setCalls((value) => value + 1);
                setControlled(true);
                setTimeout(() => { setControlled(false); setCompleted((value) => value + 1); }, 900);
            }} />
            {visible ? <RefreshButton data-testid="held-refresh" label="Held refresh" onClick={held} /> : null}
        </div>
        <div className="action-row">
            <LoadingButton data-testid="text-refresh" onClick={immediate}>Discover and refresh</LoadingButton>
            <LoadingButton data-testid="held-text-refresh" onClick={held}>Refresh repositories</LoadingButton>
            <LoadingButton data-testid="failed-refresh" variant="text" onClick={failure}>Refresh permissions</LoadingButton>
            <Button onClick={() => release.current?.()}>Complete refresh</Button>
            <Button onClick={() => setVisible((value) => !value)}>Toggle refresh control</Button>
        </div>
        <output data-testid="refresh-calls">{calls}</output>
        <output data-testid="refresh-completed">{completed}</output>
        {failed ? <p role="alert">Refresh failed</p> : null}
    </main>;
}
