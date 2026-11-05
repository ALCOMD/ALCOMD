import { useEffect, useMemo, useRef, useSyncExternalStore } from "react";
import type { DiagnosticLogQuery } from "@alcomd/sdk";
import type { GuiRpcClient } from "./rpc";
import { DiagnosticLogFeed } from "./diagnostic-log";

export function useTechnicalLogs(client: GuiRpcClient, query: DiagnosticLogQuery) {
    const queryKey = JSON.stringify(query);
    const feeds = useMemo(() => ({
        core: new DiagnosticLogFeed({ read: (params) => client.diagnosticLogRead(params), history: (params) => client.diagnosticLogHistory(params), cancel: () => client.diagnosticLogCancel() }, JSON.parse(queryKey) as DiagnosticLogQuery),
        gui: new DiagnosticLogFeed({ read: (params) => client.localLogRead(params), history: (params) => client.localLogHistory(params), cancel: () => client.localLogCancel() }, JSON.parse(queryKey) as DiagnosticLogQuery)
    }), [client, queryKey]);
    const stopping = useRef<Promise<unknown>>(Promise.resolve());
    useEffect(() => {
        let active = true;
        const started = stopping.current.then(() => { if (active) { feeds.core.start(); feeds.gui.start(); } });
        return () => {
            active = false;
            stopping.current = Promise.all([started, feeds.core.stop(), feeds.gui.stop()]);
        };
    }, [feeds]);
    const core = useSyncExternalStore(feeds.core.subscribe, feeds.core.getSnapshot);
    const gui = useSyncExternalStore(feeds.gui.subscribe, feeds.gui.getSnapshot);
    return { core, gui, feeds };
}
