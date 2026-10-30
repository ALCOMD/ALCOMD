import { useEffect, useState } from "react";

import { Progress } from "./Material";

export function useDelayedPending(pending: boolean): boolean {
    const [visible, setVisible] = useState(false);
    useEffect(() => {
        if (!pending) {
            setVisible(false);
            return;
        }
        const timer = window.setTimeout(() => setVisible(true), 160);
        return () => window.clearTimeout(timer);
    }, [pending]);
    return pending && visible;
}

/** Keep short reads quiet, while exposing pending state immediately to assistive technology. */
export function LoadingState({ title, detail }: { title: string; detail?: string }) {
    const visible = useDelayedPending(true);

    return (
        <section className="loading-state" role="status" aria-label={title} aria-live="polite" aria-busy="true">
            <div className="loading-state-indicator" hidden={!visible}>
                <Progress label={title} />
                <h2>{title}</h2>
                {detail === undefined ? null : <p>{detail}</p>}
            </div>
        </section>
    );
}
