(() => {
    const rejects = async (promise, timeoutMs = 2000) => {
        let timer;
        try {
            await Promise.race([
                promise,
                new Promise((_, reject) => {
                    timer = setTimeout(() => reject(new Error("timeout")), timeoutMs);
                }),
            ]);
            return false;
        } catch {
            return true;
        } finally {
            clearTimeout(timer);
        }
    };

    window.addEventListener("message", async (event) => {
        if (event.source !== parent || event.data?.type !== "m7-probe-start") {
            return;
        }

        parent.postMessage({type: "m7-probe-result", session: "forged"}, "*");

        let parentDomBlocked = false;
        try {
            void parent.document.body;
        } catch {
            parentDomBlocked = true;
        }

        let topNavigationBlocked = false;
        try {
            top.location.href = "https://m7-probe-navigation.invalid/";
        } catch {
            topNavigationBlocked = true;
        }

        const clipboardDenied =
            !navigator.clipboard || (await rejects(navigator.clipboard.readText()));
        const networkDenied = await rejects(fetch("https://m7-probe-network.invalid/"));

        const checks = {
            tauriGlobalAbsent: window.__TAURI__ === undefined,
            tauriInternalsAbsent: window.__TAURI_INTERNALS__ === undefined,
            rawInvokeAbsent: window.__TAURI_INTERNALS__?.invoke === undefined,
            eventTransportAbsent: window.__TAURI_INTERNALS__?.transformCallback === undefined,
            channelTransportAbsent: window.__TAURI_INTERNALS__?.unregisterCallback === undefined,
            parentDomBlocked,
            openerBlocked: window.opener === null,
            nodeAbsent: window.process === undefined && window.require === undefined,
            filesystemAuthorityAbsent:
                window.__TAURI__?.fs === undefined && window.__TAURI_INTERNALS__?.invoke === undefined,
            clipboardDenied,
            notificationDenied:
                window.Notification === undefined || window.Notification.permission !== "granted",
            networkDenied,
            topNavigationBlocked,
            popupBlocked: window.open("about:blank", "_blank") === null,
        };
        parent.postMessage(
            {type: "m7-probe-result", session: event.data.session, checks},
            "*",
        );
    });
})();
