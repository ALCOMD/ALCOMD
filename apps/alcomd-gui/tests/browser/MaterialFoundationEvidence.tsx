import { useState } from "react";
import { projectsIcon } from "@alcomd/ui/icons";

import { DataTableHeader, MaterialDataTable } from "../../src/DataTable";
import { Button, Checkbox, Dialog, Icon, IconButton, Progress, SegmentedButtons, Select, Switch, TextField } from "../../src/Material";

export function MaterialFoundationEvidence() {
    const [dialogOpen, setDialogOpen] = useState(false);
    const [name, setName] = useState("");
    const [kind, setKind] = useState("avatar");
    const [enabled, setEnabled] = useState(true);
    const [confirmed, setConfirmed] = useState(false);
    const [longLabel, setLongLabel] = useState(false);
    const [segment, setSegment] = useState("first");
    const [selections, setSelections] = useState(0);
    return (
        <main className="material-evidence" aria-labelledby="material-evidence-title">
            <h1 id="material-evidence-title">Material foundation evidence</h1>
            <SegmentedButtons label="Selection evidence" value={segment} onChange={(next) => { setSelections((count) => count + 1); if (next !== "blocked") setSegment(next); }} options={[
                { value: "first", label: "First section" },
                { value: "disabled", label: "Unavailable section", disabled: true },
                { value: "last", label: "Last section" },
                { value: "blocked", label: "Guarded section" }
            ]} />
            <output data-testid="selection-count">{selections}</output>
            <section aria-label="Button width evidence" style={{ display: "flex", gap: 8 }}>
                {(["filled", "tonal", "outlined", "text"] as const).map((variant) => <Button key={variant} data-testid={"width-" + variant} variant={variant}>Repositories</Button>)}
            </section>
            <section aria-label="Icon button width evidence" style={{ display: "flex", gap: 8 }}>
                {(["filled", "tonal", "outlined", "text"] as const).map((variant) => <Button key={variant} data-testid={"icon-width-" + variant} variant={variant}><Icon asset={projectsIcon} slot="icon" />Repositories</Button>)}
            </section>
            <section style={{ display: "flex", gap: 8 }}>
                <Button data-testid="changing-label" onClick={() => setLongLabel(!longLabel)} widthLabels={["Refresh", "Refreshing…"]}><Icon asset={projectsIcon} slot="icon" />{longLabel ? "Refreshing…" : "Refresh"}</Button>
                <Button data-testid="stable-neighbor">Neighbor</Button>
            </section>
            <Button onClick={() => setDialogOpen(true)}>Open dialog</Button>
            <Button disabled variant="tonal">Disabled action</Button>
            <IconButton label="Project evidence"><Icon asset={projectsIcon} size={24} /></IconButton>
            <TextField label="Project name" onInput={setName} supportingText="Host-owned test value" value={name} />
            <Select label="Project type" onChange={setKind} options={[
                { label: "Avatar", value: "avatar" },
                { label: "World", value: "world" }
            ]} value={kind} />
            <Switch label="Enable integration" onChange={setEnabled} selected={enabled} />
            <Checkbox checked={confirmed} label="I reviewed the plan" onChange={setConfirmed} />
            <Progress label="Material progress" value={0.62} />
            <MaterialDataTable label="Material table foundation" minWidth={240}>
                <thead><tr><DataTableHeader>Project</DataTableHeader><DataTableHeader>State</DataTableHeader></tr></thead>
                <tbody><tr><td>Foundation fixture</td><td>Ready</td></tr></tbody>
            </MaterialDataTable>
            <div style={{ whiteSpace: "nowrap" }}>
                <Dialog onClose={() => setDialogOpen(false)} open={dialogOpen} title="Material dialog">
                    <p>The dialog is hosted by the shared Material foundation.</p>
                    <p data-testid="dialog-width-evidence">Shared dialog content must wrap within the Material surface instead of being measured wider and clipped by the internal container.</p>
                    <Button onClick={() => setDialogOpen(false)} variant="text">Close</Button>
                </Dialog>
            </div>
        </main>
    );
}
