import { useTranslation } from "./i18n";
import { createContext, useCallback, useContext, useEffect, useLayoutEffect, useMemo, useState, type ReactNode } from "react";
import { Dialog, DialogCloseButton } from "./Material";

const BusyContext = createContext<(busy: boolean) => void>(() => {});
const CloseContext = createContext<{ busy: boolean; close(): void; complete(): void; registerCancel(): () => void } | undefined>(undefined);

export function useUtilityDialogComplete() {
    return useContext(CloseContext)?.complete;
}

export function useUtilityDialogBusy(busy: boolean) {
    const setBusy = useContext(BusyContext);
    useEffect(() => { setBusy(busy); return () => setBusy(false); }, [busy, setBusy]);
}

export function useUtilityDialogClose() {
    const context = useContext(CloseContext);
    if (context === undefined) throw new Error("Missing utility dialog context");
    return context.close;
}

export function UtilityDialogCancel({ disabled = false, label }: { disabled?: boolean; label?: string }) {
    const { t } = useTranslation();
    const context = useContext(CloseContext);
    const registerCancel = context?.registerCancel;
    useLayoutEffect(() => registerCancel?.(), [registerCancel]);
    if (context === undefined) return null;
    return <DialogCloseButton disabled={disabled || context.busy} variant="text">{label ?? t("Cancel")}</DialogCloseButton>;
}

export function UtilityActionDialog({ children, onClose, title, wide = false }: { children: ReactNode; onClose(): void; title: string; wide?: boolean }) {
    const { t } = useTranslation();
    const [open, setOpen] = useState(true);
    const [busy, setBusy] = useState(false);
    const [cancelCount, setCancelCount] = useState(0);
    const close = useCallback(() => { if (!busy) setOpen(false); }, [busy]);
    const complete = useCallback(() => setOpen(false), []);
    const registerCancel = useCallback(() => {
        setCancelCount((count) => count + 1);
        return () => setCancelCount((count) => count - 1);
    }, []);
    const context = useMemo(() => ({ busy, close, complete, registerCancel }), [busy, close, complete, registerCancel]);
    return <BusyContext.Provider value={setBusy}><CloseContext.Provider value={context}>
        <Dialog open={open} title={title} wide={wide} dismissible={!busy} onClose={onClose}>
            <div className="utility-action-dialog">{children}</div>
            {cancelCount === 0 ? <div className="dialog-actions"><DialogCloseButton disabled={busy} variant="text">{t("Cancel")}</DialogCloseButton></div> : null}
        </Dialog>
    </CloseContext.Provider></BusyContext.Provider>;
}
