import { Component, useEffect, type ErrorInfo, type ReactNode } from "react";
import type { GuiRpcClient } from "./rpc";
import { Reminder } from "./Reminder";
import { Button } from "./Material";
import { useTranslation } from "./i18n";

const reported = new WeakSet<Error>();
function truncateUtf8(value: string, maxBytes: number): string {
    const encoded = new TextEncoder().encode(value);
    let end = Math.min(encoded.byteLength, maxBytes);
    // Back up to the start of a scalar when the byte boundary falls inside it.
    while (end < encoded.byteLength && (encoded[end]! & 0xc0) === 0x80) end--;
    return new TextDecoder("utf-8", { ignoreBOM: true }).decode(encoded.subarray(0, end));
}
function report(client: GuiRpcClient, error: unknown, componentStack?: string) {
    // Typed Core failures already have a Core responsibility boundary.
    if (!(error instanceof Error) || "code" in error || reported.has(error)) return;
    reported.add(error);
    // A later execution may throw the same object again. Only deduplicate this propagation turn.
    setTimeout(() => reported.delete(error), 0);
    void client.reportFrontendError({ name: truncateUtf8(error.name, 128), message: truncateUtf8(error.message, 4096),
        stack: truncateUtf8([error.stack, componentStack].filter(Boolean).join("\n"), 16384),
        location: truncateUtf8(window.location.pathname, 2048) }).catch(() => {});
}
export function FrontendErrorReporter({ client }: { client: GuiRpcClient }) {
    useEffect(() => {
        const error = (event: ErrorEvent) => report(client, event.error);
        const rejection = (event: PromiseRejectionEvent) => report(client, event.reason);
        window.addEventListener("error", error); window.addEventListener("unhandledrejection", rejection);
        return () => { window.removeEventListener("error", error); window.removeEventListener("unhandledrejection", rejection); };
    }, [client]);
    return null;
}
export class FrontendErrorBoundary extends Component<{ client: GuiRpcClient; children: ReactNode }, { failed: boolean }> {
    override state = { failed: false };
    static getDerivedStateFromError() { return { failed: true }; }
    override componentDidCatch(error: Error, info: ErrorInfo) { report(this.props.client, error, info.componentStack ?? undefined); }
    override render() { return this.state.failed ? <FrontendFailure retry={() => this.setState({ failed: false })} /> : this.props.children; }
}

function FrontendFailure({ retry }: { retry(): void }) { const { t } = useTranslation(); return <Reminder tone="error" message="This view could not be displayed. Open another page or try again."><Button type="button" onClick={retry}>{t("Retry")}</Button></Reminder>; }
