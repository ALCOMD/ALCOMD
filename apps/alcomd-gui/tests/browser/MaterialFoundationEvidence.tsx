import { useState } from "react";

import { Button, Checkbox, Dialog, IconButton, Progress, Select, Switch, TextField } from "../../src/Material";

export function MaterialFoundationEvidence() {
    const [dialogOpen, setDialogOpen] = useState(false);
    const [name, setName] = useState("");
    const [kind, setKind] = useState("avatar");
    const [enabled, setEnabled] = useState(true);
    const [confirmed, setConfirmed] = useState(false);
    return (
        <main className="material-evidence" aria-labelledby="material-evidence-title">
            <h1 id="material-evidence-title">Material foundation evidence</h1>
            <Button onClick={() => setDialogOpen(true)}>Open dialog</Button>
            <Button disabled variant="tonal">Disabled action</Button>
            <IconButton label="Refresh evidence">↻</IconButton>
            <TextField label="Project name" onInput={setName} supportingText="Host-owned test value" value={name} />
            <Select label="Project type" onChange={setKind} options={[
                { label: "Avatar", value: "avatar" },
                { label: "World", value: "world" }
            ]} value={kind} />
            <Switch label="Enable integration" onChange={setEnabled} selected={enabled} />
            <Checkbox checked={confirmed} label="I reviewed the plan" onChange={setConfirmed} />
            <Progress label="Material progress" value={0.62} />
            <Dialog onClose={() => setDialogOpen(false)} open={dialogOpen} title="Material dialog">
                <p>The dialog is hosted by the shared Material foundation.</p>
                <Button onClick={() => setDialogOpen(false)} variant="text">Close</Button>
            </Dialog>
        </main>
    );
}
