import { CoreEvents } from "../../src/core-events";

declare global { interface Window { viewCachePressure: { evict(): boolean; hasActivity(): boolean } } }

/** Fixture-only access to the actual cache; it never edits page state or DOM. */
export function enableViewCachePressure() {
    let current: CoreEvents | undefined;
    const write = CoreEvents.prototype.writeView;
    CoreEvents.prototype.writeView = function (...args: Parameters<CoreEvents["writeView"]>) {
        current = this;
        return write.apply(this, args);
    };
    window.viewCachePressure = {
        evict() {
            if (!current) throw new Error("No GUI view has written its cache");
            const records = Array.from({ length: 5_001 }, (_, index) => ({ fixtureRecord: index }));
            current.writeView("projects", records, records.length);
            return current.readView("activity") === undefined;
        },
        hasActivity() { return current?.readView("activity") !== undefined; }
    };
}
