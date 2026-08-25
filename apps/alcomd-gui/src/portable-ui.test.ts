import type { UiDocument, UiSnapshot } from "@alcomd/sdk";

import discordDocumentJson from "../../../crates/alcomd-testing/fixtures/m7/discord-presence-snapshot.json" with { type: "json" };
import expectedJson from "../../../crates/alcomd-testing/fixtures/m7/headless-renderer-conformance.json" with { type: "json" };
import mcpDocumentJson from "../../../crates/alcomd-testing/fixtures/m7/mcp-management-snapshot.json" with { type: "json" };
import {
    PortableUiConsumerError,
    acceptSnapshot,
    buildTree,
    createFormDrafts,
    hasDirtyDraft,
    orderedNodeKinds,
    submitFormAction,
    updateDraft
} from "./portable-ui.ts";

const mcpDocument = mcpDocumentJson as unknown as UiDocument;
const discordDocument = discordDocumentJson as unknown as UiDocument;
const expected = expectedJson as {
    fixtures: Array<{ orderedKinds: string[] }>;
    requiredNodeKinds: string[];
};

assertDeepEqual(orderedNodeKinds(mcpDocument), expected.fixtures[0]?.orderedKinds);
assertDeepEqual(orderedNodeKinds(discordDocument), expected.fixtures[1]?.orderedKinds);
assertDeepEqual(
    [...new Set([...orderedNodeKinds(mcpDocument), ...orderedNodeKinds(discordDocument)])].sort(),
    [...expected.requiredNodeKinds].sort()
);

const snapshot = makeSnapshot("session-one", 1, discordDocument);
const drafts = createFormDrafts(snapshot);
const original = drafts["settings-form"];
assert(original !== undefined, "settings form draft is present");
assertEqual(original.sessionId, snapshot.sessionId);
assertEqual(original.snapshotRevision, snapshot.snapshotRevision);
assertEqual(original.formNodeId, "settings-form");
assertEqual(hasDirtyDraft(drafts), false);

const changed = {
    ...drafts,
    "settings-form": updateDraft(original, "presence-text", {
        kind: "text",
        value: "available"
    })
};
assertEqual(hasDirtyDraft(changed), true);
const submit = submitFormAction(discordDocument, "settings-form", changed["settings-form"]!);
assertEqual(submit.kind, "submit-form");
if (submit.kind === "submit-form") {
    assertDeepEqual(
        submit.values.map((field) => field.fieldId),
        ["presence-enabled", "presence-mode", "presence-text", "refresh-interval"]
    );
    assertDeepEqual(
        submit.values.map((field) => field.value.kind),
        ["boolean", "selection", "text", "integer"]
    );
}

const authorityDocument = structuredClone(discordDocument);
const disabledSwitch = authorityDocument.nodes.find((node) => node.nodeId === "presence-enabled");
const readOnlyText = authorityDocument.nodes.find((node) => node.nodeId === "presence-text");
if (disabledSwitch?.kind !== "switch" || readOnlyText?.kind !== "text-field") {
    throw new Error("fixture fields missing");
}
disabledSwitch.payload.disabled = true;
readOnlyText.payload.readOnly = true;
const authoritySnapshot = makeSnapshot("session-authority", 1, authorityDocument);
const authorityDraft = createFormDrafts(authoritySnapshot)["settings-form"];
assert(authorityDraft !== undefined, "authority draft is present");
const authoritySubmit = submitFormAction(authorityDocument, "settings-form", authorityDraft);
if (authoritySubmit.kind === "submit-form") {
    assertDeepEqual(
        authoritySubmit.values.map((field) => field.fieldId),
        ["presence-mode", "refresh-interval"]
    );
}

const revisionTwo = makeSnapshot("session-one", 2, mcpDocument);
assertEqual(acceptSnapshot(snapshot, revisionTwo, false).snapshotRevision, 2);
assertEqual(acceptSnapshot(revisionTwo, revisionTwo, true).snapshotRevision, 2);
expectCode(() => acceptSnapshot(revisionTwo, snapshot, false), "extension_ui_snapshot_stale");
expectCode(() => acceptSnapshot(revisionTwo, revisionTwo, false), "extension_ui_snapshot_stale");
expectCode(
    () => acceptSnapshot(revisionTwo, makeSnapshot("other-session", 3, mcpDocument), false),
    "extension_ui_snapshot_stale"
);

const unknownProtocol = structuredClone(mcpDocument) as unknown as { protocol: string };
unknownProtocol.protocol = "portable-v2";
expectCode(() => buildTree(unknownProtocol as UiDocument), "extension_ui_protocol_unsupported");

const unknownNode = structuredClone(mcpDocument) as unknown as {
    protocol: "portable-v1";
    title: string;
    nodes: Array<Record<string, unknown>>;
};
unknownNode.nodes[1]!["kind"] = "custom-html";
expectCode(() => buildTree(unknownNode as unknown as UiDocument), "extension_ui_document_invalid");

const missingParent = structuredClone(mcpDocument);
missingParent.nodes[1]!.parentId = "missing";
expectCode(() => buildTree(missingParent), "extension_ui_document_invalid");

function makeSnapshot(sessionId: string, snapshotRevision: number, document: UiDocument): UiSnapshot {
    return { sessionId, snapshotRevision, document };
}

function expectCode(run: () => unknown, code: PortableUiConsumerError["code"]): void {
    try {
        run();
    } catch (error: unknown) {
        assert(error instanceof PortableUiConsumerError, `expected ${code}`);
        assertEqual(error.code, code);
        return;
    }
    throw new Error(`expected ${code}`);
}

function assert(value: unknown, message: string): asserts value {
    if (!value) {
        throw new Error(message);
    }
}

function assertEqual<Actual>(actual: Actual, expectedValue: Actual): void {
    if (actual !== expectedValue) {
        throw new Error(`expected ${String(expectedValue)}, received ${String(actual)}`);
    }
}

function assertDeepEqual(actual: unknown, expectedValue: unknown): void {
    if (JSON.stringify(actual) !== JSON.stringify(expectedValue)) {
        throw new Error("values are not deeply equal");
    }
}
