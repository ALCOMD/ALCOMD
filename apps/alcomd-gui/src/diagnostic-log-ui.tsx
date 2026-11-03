import { createContext, useContext, useEffect, useMemo, useRef, useSyncExternalStore, type ReactNode } from "react";
import type { DiagnosticLogItem } from "@alcomd/sdk";
import { DiagnosticDetails, GlobalReminder, Reminder } from "./Reminder";
import { DiagnosticLogFeed, diagnosticLogKey, diagnosticLogMessage, type DiagnosticLogSnapshot, type DiagnosticLogSource } from "./diagnostic-log";
import { useTranslation } from "./i18n";

const DiagnosticLogContext = createContext<DiagnosticLogFeed | undefined>(undefined);
const noChanges = () => () => {};
const empty: DiagnosticLogSnapshot = { items: [], notifications: [], gap: false };
const emptySnapshot = (): DiagnosticLogSnapshot => empty;

export function DiagnosticLogProvider({ children, client, enabled }: { children: ReactNode; client: DiagnosticLogSource; enabled: boolean }) {
    const feed = useMemo(() => new DiagnosticLogFeed(client), [client]);
    const stopping = useRef<Promise<void>>(Promise.resolve());
    useEffect(() => {
        if (!enabled) return;
        let active = true;
        const starting = stopping.current.then(() => { if (active) feed.start(); });
        return () => {
            active = false;
            const stopped = feed.stop();
            stopping.current = Promise.all([starting, stopped]).then(() => {});
        };
    }, [enabled, feed]);
    return <DiagnosticLogContext.Provider value={feed}>{children}<DiagnosticLogNotifications feed={feed} /></DiagnosticLogContext.Provider>;
}

export function useDiagnosticLog() {
    const feed = useContext(DiagnosticLogContext);
    return { snapshot: useSyncExternalStore(feed?.subscribe ?? noChanges, feed?.getSnapshot ?? emptySnapshot), retry: feed?.retryNow };
}

function DiagnosticLogNotifications({ feed }: { feed: DiagnosticLogFeed }) {
    const snapshot = useSyncExternalStore(feed.subscribe, feed.getSnapshot);
    return <>{snapshot.notifications.map((item) => <GlobalReminder key={diagnosticLogKey(item)}>
        <DiagnosticLogNotification item={item} dismiss={() => feed.dismiss(diagnosticLogKey(item))} />
    </GlobalReminder>)}</>;
}

function DiagnosticLogNotification({ dismiss, item }: { dismiss(): void; item: DiagnosticLogItem }) {
    const { t } = useTranslation();
    return <Reminder details={<DiagnosticDetails code={item.code} diagnosticId={item.diagnosticId} />} onDismiss={dismiss}
        title={t(item.level === "warning" ? "Background warning" : "Background error")} tone={item.level === "warning" ? "warning" : "error"}>
        <p>{t(diagnosticLogMessage(item.code))}</p>
    </Reminder>;
}
