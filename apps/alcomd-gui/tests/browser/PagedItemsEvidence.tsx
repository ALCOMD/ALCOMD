import { useState } from "react";
import { createRoot } from "react-dom/client";

import { Button } from "../../src/Material";
import { PagedItems, type ItemPage } from "../../src/PagedItems";
import "../../src/styles.css";

interface PendingPage {
    cursor: number;
    resolve(page: ItemPage<string, number>): void;
    reject(error: Error): void;
}

declare global {
    interface Window {
        pendingPages: PendingPage[];
    }
}

window.pendingPages = [];

function Evidence() {
    const [items, setItems] = useState(["First item"]);
    const [cursor, setCursor] = useState(1);
    const [mounted, setMounted] = useState(true);
    return (
        <main>
            <Button onClick={() => setItems(["Refreshed item"])}>Refresh first page</Button>
            <Button onClick={() => setCursor(10)}>Replace cursor</Button>
            <Button onClick={() => setMounted((value) => !value)}>Toggle list</Button>
            {mounted ? <PagedItems initialItems={items} initialCursor={cursor} loadMore={(next) => new Promise((resolve, reject) => {
                window.pendingPages.push({ cursor: next, resolve, reject });
            })}>{(loaded) => <ul aria-label="Loaded items">{loaded.map((item, index) => <li key={index}>{item}</li>)}</ul>}</PagedItems> : null}
        </main>
    );
}

createRoot(document.getElementById("root")!).render(<Evidence />);
