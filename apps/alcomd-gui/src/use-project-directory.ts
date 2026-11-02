import { useCallback, useEffect, useRef, useState } from "react";
import type { RpcError } from "@alcomd/sdk";
import type { GuiRpcClient } from "./rpc";
import { normalizeRequestError } from "./request-errors";

/** Seed each opened form once; later reads never replace the user's target. */
export function useProjectDirectory(client: GuiRpcClient, open: boolean) {
    const [directory, setValue] = useState("");
    const [error, setError] = useState<RpcError>();
    const edited = useRef(false);
    const setDirectory = useCallback((value: string) => { edited.current = true; setValue(value); setError(undefined); }, []);
    useEffect(() => {
        let active = true;
        edited.current = false;
        setValue("");
        setError(undefined);
        if (open) void client.settingsGet().then((snapshot) => {
            if (active && !edited.current) setValue(snapshot.directories.projectDirectory);
        }).catch((caught: unknown) => {
            if (active && !edited.current) setError(normalizeRequestError(caught));
        });
        return () => { active = false; };
    }, [client, open]);
    return { directory, setDirectory, error };
}
