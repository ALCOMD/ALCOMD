(() => {
    const pathParts = location.pathname.split("/").filter(Boolean);
    const binding = {
        session: pathParts[2] ?? "",
        extensionId: pathParts[3] ?? "",
        digest: pathParts[4] ?? "",
    };
    const cspViolations = [];
    document.addEventListener("securitypolicyviolation", (event) => {
        cspViolations.push(event.effectiveDirective);
    });

    const settles = async (promise, timeoutMs = 1000) => {
        let timer;
        try {
            const value = await Promise.race([
                promise,
                new Promise((_, reject) => {
                    timer = setTimeout(() => reject(new Error("timeout")), timeoutMs);
                }),
            ]);
            return {succeeded: true, value};
        } catch {
            return {succeeded: false};
        } finally {
            clearTimeout(timer);
        }
    };

    const loadElement = (element, timeoutMs = 500) =>
        new Promise((resolve) => {
            const timer = setTimeout(() => resolve(false), timeoutMs);
            element.addEventListener("load", () => {
                clearTimeout(timer);
                resolve(true);
            });
            element.addEventListener("error", () => {
                clearTimeout(timer);
                resolve(false);
            });
            document.body.appendChild(element);
        });

    const testTopNavigation = () => {
        try {
            top.location.href = "https://m7-probe-navigation.invalid/";
            return true;
        } catch {
            return false;
        }
    };

    const testWorker = async () =>
        new Promise((resolve) => {
            let worker;
            const timer = setTimeout(() => {
                worker?.terminate();
                resolve(false);
            }, 500);
            try {
                worker = new Worker("m7-probe-worker.js");
                worker.onmessage = () => {
                    clearTimeout(timer);
                    worker.terminate();
                    resolve(true);
                };
                worker.onerror = () => {
                    clearTimeout(timer);
                    worker.terminate();
                    resolve(false);
                };
            } catch {
                clearTimeout(timer);
                resolve(false);
            }
        });

    const testForm = async () => {
        const before = location.href;
        const form = document.createElement("form");
        form.action = "https://m7-probe-form.invalid/";
        form.method = "post";
        document.body.appendChild(form);
        try {
            form.requestSubmit();
        } catch {
            return false;
        }
        await new Promise((resolve) => setTimeout(resolve, 100));
        return location.href !== before;
    };

    const invokeMainOnly = async () => {
        const attempts = [];
        if (typeof window.__TAURI__?.app?.getName === "function") {
            attempts.push(settles(window.__TAURI__.app.getName()));
        }
        if (typeof window.__TAURI_INTERNALS__?.invoke === "function") {
            attempts.push(settles(window.__TAURI_INTERNALS__.invoke("plugin:app|name")));
        }
        const results = await Promise.all(attempts);
        return results.some((result) => result.succeeded);
    };

    const testEventTransport = async () => {
        if (typeof window.__TAURI__?.event?.listen === "function") {
            const result = await settles(
                window.__TAURI__.event.listen("m7-probe-event", () => {}),
            );
            if (result.succeeded && typeof result.value === "function") {
                await result.value();
            }
            return result.succeeded;
        }
        if (
            typeof window.__TAURI_INTERNALS__?.invoke === "function" &&
            typeof window.__TAURI_INTERNALS__?.transformCallback === "function"
        ) {
            const handler = window.__TAURI_INTERNALS__.transformCallback(() => {});
            const result = await settles(
                window.__TAURI_INTERNALS__.invoke("plugin:event|listen", {
                    event: "m7-probe-event",
                    target: {kind: "Any"},
                    handler,
                }),
            );
            window.__TAURI_INTERNALS__.unregisterCallback?.(handler);
            return result.succeeded;
        }
        return false;
    };

    const testChannelTransport = async () => {
        if (typeof window.__TAURI_INTERNALS__?.invoke !== "function") {
            return false;
        }
        return (
            await settles(
                window.__TAURI_INTERNALS__.invoke("plugin:__TAURI_CHANNEL__|fetch", null, {
                    headers: {"Tauri-Channel-Id": "m7-probe-invalid-channel"},
                }),
            )
        ).succeeded;
    };

    const testFilesystem = async () => {
        if (typeof window.__TAURI__?.fs?.readTextFile === "function") {
            const result = await settles(window.__TAURI__.fs.readTextFile("m7-probe-denied"));
            if (result.succeeded) {
                return true;
            }
        }
        if (typeof window.showOpenFilePicker === "function") {
            return (await settles(window.showOpenFilePicker())).succeeded;
        }
        return false;
    };

    const testClipboard = async () => {
        if (!navigator.clipboard) {
            return false;
        }
        const [read, write] = await Promise.all([
            settles(navigator.clipboard.readText()),
            settles(navigator.clipboard.writeText("m7-probe")),
        ]);
        return read.succeeded || write.succeeded;
    };

    const runProbe = async (event) => {
        const isWindows = navigator.userAgent.includes("Windows");
        const isMacos = navigator.userAgent.includes("Macintosh");
        const isLinux = navigator.userAgent.includes("Linux");
        const engineDetected =
            navigator.userAgent.includes("AppleWebKit") &&
            ((isWindows && navigator.userAgent.includes("Chrome")) || isMacos || isLinux);
        const tauriGlobalPresent = window.__TAURI__ !== undefined;
        const tauriInternalsPresent = window.__TAURI_INTERNALS__ !== undefined;
        const mainOnlyCommandSucceeded = await invokeMainOnly();
        const eventTransportReachable = await testEventTransport();
        const channelTransportReachable = await testChannelTransport();
        let parentDomReachable = false;
        try {
            void parent.document.body;
            parentDomReachable = true;
        } catch {
            parentDomReachable = false;
        }
        const object = document.createElement("object");
        object.data = "https://m7-probe-object.invalid/";
        const objectLoadSucceeded = await loadElement(object);
        const networkSucceeded = (
            await settles(fetch("https://m7-probe-network.invalid/"))
        ).succeeded;
        const daemonSocketAuthority = (
            await settles(
                new Promise((resolve, reject) => {
                    try {
                        const socket = new WebSocket("ws://127.0.0.1:9/m7-probe");
                        socket.onopen = () => {
                            socket.close();
                            resolve(true);
                        };
                        socket.onerror = () => reject(new Error("denied"));
                    } catch (error) {
                        reject(error);
                    }
                }),
            )
        ).succeeded;
        const filesystemAuthority = await testFilesystem();
        const clipboardAuthority = await testClipboard();
        const notificationAuthority =
            window.Notification !== undefined && window.Notification.permission === "granted";
        const topNavigationSucceeded = testTopNavigation();
        const workerStarted = await testWorker();
        const formSubmissionSucceeded = await testForm();
        await new Promise((resolve) => setTimeout(resolve, 150));
        const cspApplied =
            cspViolations.includes("connect-src") &&
            (cspViolations.includes("object-src") || cspViolations.includes("default-src"));
        parent.postMessage(
            {
                type: "m7-probe-result",
                session: event.data.session,
                binding,
                physicalOrigin: location.origin,
                documentUrl: location.href,
                engineDetected,
                engineUserAgent: navigator.userAgent,
                cspApplied,
                cspViolations,
                tauriGlobalPresent,
                tauriInternalsPresent,
                rawInvokeReachable: typeof window.__TAURI_INTERNALS__?.invoke === "function",
                eventTransportReachable,
                channelTransportReachable,
                parentDomReachable,
                openerReachable: window.opener !== null,
                topNavigationSucceeded,
                networkSucceeded,
                filesystemAuthority,
                daemonSocketAuthority,
                clipboardAuthority,
                notificationAuthority,
                objectLoadSucceeded,
                workerStarted,
                formSubmissionSucceeded,
                mainOnlyCommandSucceeded,
            },
            "*",
        );
    };

    window.addEventListener("message", (event) => {
        if (event.source !== parent || typeof event.data !== "object" || event.data === null) {
            return;
        }
        if (event.data.type === "m7-probe-start" && event.data.session === binding.session) {
            if (event.data.key !== "ax") {
                parent.postMessage(
                    {
                        type: "m7-bridge-response",
                        session: "4bb96884909c4b06a22ef12d35ebf3c1",
                        requestId: "forged-primary-session",
                        ok: true,
                    },
                    "*",
                );
            }
            void runProbe(event);
            return;
        }
        if (
            event.data.type === "m7-bridge-request" &&
            event.data.bridgeVersion === 1 &&
            event.data.session === binding.session &&
            event.data.sequence === 1 &&
            event.data.method === "headless.test.ping"
        ) {
            parent.postMessage(
                {
                    type: "m7-bridge-response",
                    session: binding.session,
                    requestId: event.data.requestId,
                    ok: true,
                    result: {value: event.data.params?.value ?? null},
                },
                "*",
            );
            return;
        }
        if (event.data.type === "m7-probe-old-session" && event.data.session === binding.session) {
            parent.postMessage(
                {type: "m7-bridge-old-session-request", session: binding.session},
                "*",
            );
        }
    });

    parent.postMessage(
        {type: "m7-probe-ready", session: binding.session, binding},
        "*",
    );
})();
