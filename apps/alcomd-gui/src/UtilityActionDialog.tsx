import { createContext, useCallback, useContext, useEffect, useLayoutEffect, useMemo, useState, type ReactNode } from "react";
import { Button, Dialog } from "./Material";

const BusyContext = createContext<(busy: boolean) => void>(() => {});
const CloseContext = createContext<{ busy: boolean; close(): void; registerCancel(): () => void } | undefined>(undefined);

export function useUtilityDialogBusy(busy: boolean) {
    const setBusy = useContext(BusyContext);
    useEffect(() => { setBusy(busy); return () => setBusy(false); }, [busy, setBusy]);
}

export function UtilityDialogCancel({ disabled = false, label = "Cancel" }: { disabled?: boolean; label?: string }) {
    const context = useContext(CloseContext);
    const registerCancel = context?.registerCancel;
    useLayoutEffect(() => registerCancel?.(), [registerCancel]);
    if (context === undefined) return null;
    return <Button disabled={disabled || context.busy} variant="text" onClick={context.close} type="button">{label}</Button>;
}

export function UtilityActionDialog({ children, onClose, title }: { children: ReactNode; onClose(): void; title: string }) {
    const [open, setOpen] = useState(true);
    const [busy, setBusy] = useState(false);
    const [cancelCount, setCancelCount] = useState(0);
    const close = useCallback(() => { if (!busy) setOpen(false); }, [busy]);
    const registerCancel = useCallback(() => {
        setCancelCount((count) => count + 1);
        return () => setCancelCount((count) => count - 1);
    }, []);
    const context = useMemo(() => ({ busy, close, registerCancel }), [busy, close, registerCancel]);
    return <BusyContext.Provider value={setBusy}><CloseContext.Provider value={context}>
        <Dialog open={open} title={title} dismissible={!busy} onClose={onClose}>
            <div className="utility-action-dialog">{children}</div>
            {cancelCount === 0 ? <div className="dialog-actions"><Button disabled={busy} variant="text" onClick={close} type="button">Cancel</Button></div> : null}
        </Dialog>
    </CloseContext.Provider></BusyContext.Provider>;
}
