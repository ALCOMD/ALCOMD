(() => {
    window.__m7ProbeHostScriptStarted = true;
    const bindings = {
        ax: {
            frame: document.getElementById("extension-frame-ax"),
            session: "4bb96884909c4b06a22ef12d35ebf3c1",
            extensionId: "dev.example.m7-probe-a",
            digest: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        },
        bx: {
            frame: document.getElementById("extension-frame-bx"),
            session: "9ab460ff67b847efa8f47544b20def21",
            extensionId: "dev.example.m7-probe-b",
            digest: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        },
        ay: {
            frame: document.getElementById("extension-frame-ay"),
            session: "31c603a567524eaf832e47514b4b3587",
            extensionId: "dev.example.m7-probe-a",
            digest: "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789",
        },
    };
    const state = Object.fromEntries(Object.keys(bindings).map((key) => [key, {}]));
    const sourceToKey = (source) =>
        Object.entries(bindings).find(([, binding]) => binding.frame.contentWindow === source)?.[0];
    let mainCommandPassed = false;
    let bridgeAxPassed = false;
    let bridgeAyPassed = false;
    let confusedDeputyRejected = false;
    let oldDigestSessionInvalidated = false;
    let finished = false;
    let finishTimer;

    const physicalUrl = (binding) => {
        const path = `/v1/s/${binding.session}/${binding.extensionId}/${binding.digest}/m7-probe-extension.html`;
        return navigator.userAgent.includes("Windows")
            ? `https://alcomd-extension-ui.localhost${path}`
            : `alcomd-extension-ui://localhost${path}`;
    };

    const sendBridgePing = (key, requestId) => {
        const binding = bindings[key];
        binding.frame.contentWindow.postMessage(
            {
                type: "m7-bridge-request",
                bridgeVersion: 1,
                session: binding.session,
                sequence: 1,
                requestId,
                method: "headless.test.ping",
                params: {value: key},
            },
            "*",
        );
    };

    const finish = (force = false) => {
        if (finished || !Object.values(state).every((entry) => entry.result)) {
            return;
        }
        const ax = state.ax.result;
        const bx = state.bx.result;
        const ay = state.ay.result;
        const all = [ax, bx, ay];
        const any = (field) => all.some((result) => result[field] === true);
        const allFalse = (field) => all.every((result) => result[field] === false);
        const documentsDistinct = new Set(all.map((result) => result.documentUrl)).size === 3;
        const bindingMatches = Object.entries(bindings).every(([key, binding]) => {
            const reported = state[key].result.binding;
            return (
                reported?.session === binding.session &&
                reported?.extensionId === binding.extensionId &&
                reported?.digest === binding.digest
            );
        });
        const originIsolationAxBx =
            documentsDistinct &&
            bindingMatches &&
            ax.parentDomReachable === false &&
            bx.parentDomReachable === false;
        const originIsolationAxAy =
            documentsDistinct &&
            bindingMatches &&
            ax.parentDomReachable === false &&
            ay.parentDomReachable === false;
        const cspApplied = all.every(
            (result) => result.cspApplied === true && result.networkSucceeded === false,
        );
        const engineDetected = all.every((result) => result.engineDetected === true);
        const sandboxTokens = bindings.ax.frame.getAttribute("sandbox") ?? "";
        const bridgeEstablished =
            bridgeAxPassed && bridgeAyPassed && confusedDeputyRejected && oldDigestSessionInvalidated;
        if (!force && !bridgeEstablished) {
            return;
        }
        clearTimeout(finishTimer);
        const failed = [];
        const requireCheck = (condition, name) => {
            if (!condition) {
                failed.push(name);
            }
        };
        requireCheck(mainCommandPassed, "host-main-command-control");
        requireCheck(bridgeEstablished, "bridge-control");
        requireCheck(cspApplied, "csp-enforcement");
        requireCheck(engineDetected, "expected-webview-engine");
        requireCheck(sandboxTokens === "allow-scripts", "sandbox-token-set");
        requireCheck(originIsolationAxBx, "origin-isolation-a-x-b-x");
        requireCheck(originIsolationAxAy, "origin-isolation-a-x-a-y");
        for (const field of [
            "mainOnlyCommandSucceeded",
            "eventTransportReachable",
            "channelTransportReachable",
            "parentDomReachable",
            "openerReachable",
            "topNavigationSucceeded",
            "networkSucceeded",
            "filesystemAuthority",
            "daemonSocketAuthority",
            "clipboardAuthority",
            "notificationAuthority",
            "objectLoadSucceeded",
            "workerStarted",
            "formSubmissionSucceeded",
        ]) {
            requireCheck(allFalse(field), field);
        }
        finished = true;
        const query = new URLSearchParams({
            result: failed.length === 0 ? "pass" : "isolation_failed",
            processEnteredMain: "true",
            webviewCreated: "true",
            extensionDocumentLoaded: "true",
            physicalOrigin: state.ax.readyOrigin,
            physicalScheme: new URL(ax.documentUrl).protocol,
            logicalOriginBinding: `${bindings.ax.extensionId}@sha256:${bindings.ax.digest}`,
            documentUrl: ax.documentUrl,
            physicalOrigins: JSON.stringify({
                ax: {
                    messageOrigin: state.ax.readyOrigin,
                    documentOrigin: ax.physicalOrigin,
                    documentUrl: ax.documentUrl,
                },
                bx: {
                    messageOrigin: state.bx.readyOrigin,
                    documentOrigin: bx.physicalOrigin,
                    documentUrl: bx.documentUrl,
                },
                ay: {
                    messageOrigin: state.ay.readyOrigin,
                    documentOrigin: ay.physicalOrigin,
                    documentUrl: ay.documentUrl,
                },
            }),
            sandboxTokens,
            cspApplied: String(cspApplied),
            cspViolations: JSON.stringify(all.map((result) => result.cspViolations)),
            bridgeEstablished: String(bridgeEstablished),
            tauriGlobalPresent: String(any("tauriGlobalPresent")),
            tauriInternalsPresent: String(any("tauriInternalsPresent")),
            rawInvokeReachable: String(any("rawInvokeReachable")),
            eventTransportReachable: String(any("eventTransportReachable")),
            channelTransportReachable: String(any("channelTransportReachable")),
            parentDomReachable: String(any("parentDomReachable")),
            openerReachable: String(any("openerReachable")),
            topNavigationSucceeded: String(any("topNavigationSucceeded")),
            networkSucceeded: String(any("networkSucceeded")),
            filesystemAuthority: String(any("filesystemAuthority")),
            daemonSocketAuthority: String(any("daemonSocketAuthority")),
            clipboardAuthority: String(any("clipboardAuthority")),
            notificationAuthority: String(any("notificationAuthority")),
            objectLoadSucceeded: String(any("objectLoadSucceeded")),
            workerStarted: String(any("workerStarted")),
            formSubmissionSucceeded: String(any("formSubmissionSucceeded")),
            mainOnlyCommandSucceeded: String(any("mainOnlyCommandSucceeded")),
            confusedDeputySucceeded: String(!confusedDeputyRejected),
            originIsolationAxBx: String(originIsolationAxBx),
            originIsolationAxAy: String(originIsolationAxAy),
            oldDigestSessionInvalidated: String(oldDigestSessionInvalidated),
            engineDetected: String(engineDetected),
            engineUserAgent: ax.engineUserAgent,
            failedChecks: failed.join(","),
        });
        window.location.href = `https://m7-probe-result.invalid/?${query}`;
    };

    const maybeStart = async () => {
        if (!Object.values(state).every((entry) => entry.ready)) {
            return;
        }
        try {
            const name = await window.__TAURI__.app.getName();
            mainCommandPassed = name === "ALCOMD M7 Isolation Probe";
        } catch {
            mainCommandPassed = false;
        }
        for (const [key, binding] of Object.entries(bindings)) {
            binding.frame.contentWindow.postMessage(
                {type: "m7-probe-start", session: binding.session, key},
                "*",
            );
        }
    };

    window.addEventListener("message", (event) => {
        if (typeof event.data !== "object" || event.data === null) {
            return;
        }
        const key = sourceToKey(event.source);
        if (!key) {
            return;
        }
        const binding = bindings[key];
        if (event.data.type === "m7-probe-ready") {
            state[key].ready = event.data.session === binding.session;
            state[key].readyOrigin = event.origin;
            void maybeStart();
            return;
        }
        if (event.data.type === "m7-bridge-response") {
            const validSourceAndSession = event.data.session === binding.session;
            if (!validSourceAndSession || event.data.requestId === "forged-primary-session") {
                confusedDeputyRejected = true;
                return;
            }
            if (key === "ax" && event.data.requestId === "ping-ax" && event.data.ok === true) {
                bridgeAxPassed = true;
                binding.frame.contentWindow.postMessage(
                    {type: "m7-probe-old-session", session: binding.session},
                    "*",
                );
                sendBridgePing("ay", "ping-ay");
            } else if (key === "ay" && event.data.requestId === "ping-ay" && event.data.ok === true) {
                bridgeAyPassed = true;
            }
            finish();
            return;
        }
        if (event.data.type === "m7-bridge-old-session-request") {
            oldDigestSessionInvalidated = key === "ax" && event.data.session === binding.session;
            finish();
            return;
        }
        if (event.data.type === "m7-probe-result" && event.data.session === binding.session) {
            state[key].result = event.data;
            if (key === "ax") {
                sendBridgePing("ax", "ping-ax");
            }
            if (Object.values(state).every((entry) => entry.result) && !finishTimer) {
                finishTimer = setTimeout(() => finish(true), 2000);
            }
            finish();
        }
    });

    for (const binding of Object.values(bindings)) {
        binding.frame.src = physicalUrl(binding);
    }
})();
