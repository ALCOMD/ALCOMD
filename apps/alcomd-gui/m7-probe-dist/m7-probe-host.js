(() => {
    const frame = document.getElementById("extension-frame");
    const session = "4bb96884909c4b06a22ef12d35ebf3c1";
    const extensionId = "dev.example.m7-probe";
    const digest = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    const assetPath = `/v1/s/${session}/${extensionId}/${digest}/m7-probe-extension.html`;
    frame.src = navigator.userAgent.includes("Windows")
        ? `https://alcomd-extension-ui.localhost${assetPath}`
        : `alcomd-extension-ui://localhost${assetPath}`;
    let forgedRejected = false;

    window.addEventListener("message", (event) => {
        if (event.source !== frame.contentWindow || typeof event.data !== "object") {
            return;
        }
        if (event.data.session !== session) {
            forgedRejected = true;
            return;
        }
        if (event.data.type !== "m7-probe-result") {
            return;
        }

        const checks = event.data.checks;
        const required = [
            "tauriGlobalAbsent",
            "tauriInternalsAbsent",
            "rawInvokeAbsent",
            "eventTransportAbsent",
            "channelTransportAbsent",
            "parentDomBlocked",
            "openerBlocked",
            "nodeAbsent",
            "filesystemAuthorityAbsent",
            "clipboardDenied",
            "notificationDenied",
            "networkDenied",
            "topNavigationBlocked",
            "popupBlocked",
        ];
        const failed = required.filter((name) => !checks || checks[name] !== true);
        if (!forgedRejected) {
            failed.push("forged-message-not-rejected");
        }
        if (window.__TAURI__ === undefined) {
            failed.push("host-tauri-global-absent");
        }
        if (window.__TAURI_INTERNALS__ === undefined) {
            failed.push("host-tauri-internals-absent");
        }
        const result = failed.length === 0 ? "pass" : "fail";
        window.location.href =
            `https://m7-probe-result.invalid/?result=${result}&failed=${encodeURIComponent(failed.join(","))}`;
    });

    frame.addEventListener("load", () => {
        frame.contentWindow.postMessage({type: "m7-probe-start", session}, "*");
    });
})();
