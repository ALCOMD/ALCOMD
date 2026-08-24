use std::{collections::BTreeMap, thread, time::Duration};

use tauri::{
    WebviewUrl, WebviewWindowBuilder,
    http::{HeaderValue, Request, Response, StatusCode, header},
};

const RESULT_HOST: &str = "m7-probe-result.invalid";
const EXTENSION_HTML: &[u8] = include_bytes!("../../m7-probe-dist/m7-probe-extension.html");
const EXTENSION_JS: &[u8] = include_bytes!("../../m7-probe-dist/m7-probe-extension.js");
const WORKER_JS: &[u8] = include_bytes!("../../m7-probe-dist/m7-probe-worker.js");
const EXTENSION_CSP: &str = "default-src 'none'; script-src 'self'; style-src 'none'; connect-src 'none'; img-src 'none'; media-src 'none'; object-src 'none'; child-src 'none'; frame-src 'none'; worker-src 'none'; manifest-src 'none'; form-action 'none'; base-uri 'none'; frame-ancestors https://tauri.localhost tauri://localhost";
const SESSION_AX: &str = "4bb96884909c4b06a22ef12d35ebf3c1";
const SESSION_BX: &str = "9ab460ff67b847efa8f47544b20def21";
const SESSION_AY: &str = "31c603a567524eaf832e47514b4b3587";
const EXTENSION_A: &str = "dev.example.m7-probe-a";
const EXTENSION_B: &str = "dev.example.m7-probe-b";
const DIGEST_X: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const DIGEST_Y: &str = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";

fn result_path() -> std::io::Result<std::path::PathBuf> {
    Ok(std::env::current_exe()?.with_file_name("m7-webview-probe-result.json"))
}

fn platform() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

fn webview_engine() -> &'static str {
    if cfg!(target_os = "windows") {
        "WebView2"
    } else if cfg!(target_os = "macos") {
        "WKWebView"
    } else {
        "WebKitGTK"
    }
}

fn json_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => escaped.push_str("\\uFFFD"),
            character => escaped.push(character),
        }
    }
    escaped.push('"');
    escaped
}

fn write_evidence(result: &str, fields: &BTreeMap<String, String>) -> std::io::Result<()> {
    const BOOLEAN_FIELDS: &[&str] = &[
        "processEnteredMain",
        "webviewCreated",
        "extensionDocumentLoaded",
        "cspApplied",
        "bridgeEstablished",
        "tauriGlobalPresent",
        "tauriInternalsPresent",
        "rawInvokeReachable",
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
        "mainOnlyCommandSucceeded",
        "confusedDeputySucceeded",
        "originIsolationAxBx",
        "originIsolationAxAy",
        "oldDigestSessionInvalidated",
        "engineDetected",
    ];
    let mut entries = vec![
        "\"schema\":2".to_owned(),
        format!("\"platform\":{}", json_string(platform())),
        format!("\"webviewEngine\":{}", json_string(webview_engine())),
        "\"candidateMode\":\"sandboxed_cross_origin_iframe\"".to_owned(),
    ];
    for field in BOOLEAN_FIELDS {
        let value = fields.get(*field).is_some_and(|value| value == "true");
        entries.push(format!("{}:{value}", json_string(field)));
    }
    for field in [
        "physicalOrigin",
        "physicalScheme",
        "logicalOriginBinding",
        "documentUrl",
        "physicalOrigins",
        "sandboxTokens",
        "cspViolations",
        "failedChecks",
        "engineUserAgent",
    ] {
        entries.push(format!(
            "{}:{}",
            json_string(field),
            json_string(fields.get(field).map_or("", String::as_str))
        ));
    }
    entries.push(format!("\"result\":{}", json_string(result)));
    std::fs::write(result_path()?, format!("{{{}}}\n", entries.join(",")))
}

fn phase_evidence(webview_created: bool) {
    let mut fields = BTreeMap::new();
    fields.insert("processEnteredMain".to_owned(), "true".to_owned());
    fields.insert("webviewCreated".to_owned(), webview_created.to_string());
    fields.insert(
        "failedChecks".to_owned(),
        if webview_created {
            "probe-did-not-complete"
        } else {
            "webview-not-created"
        }
        .to_owned(),
    );
    let _ = write_evidence("harness_unavailable", &fields);
}

fn is_approved_asset_path(path: &str) -> Option<&str> {
    let bindings = [
        (SESSION_AX, EXTENSION_A, DIGEST_X),
        (SESSION_BX, EXTENSION_B, DIGEST_X),
        (SESSION_AY, EXTENSION_A, DIGEST_Y),
    ];
    bindings.iter().find_map(|(session, extension_id, digest)| {
        let prefix = format!("/v1/s/{session}/{extension_id}/{digest}/");
        path.strip_prefix(&prefix)
            .and_then(|relative| match relative {
                "m7-probe-extension.html" | "m7-probe-extension.js" | "m7-probe-worker.js" => {
                    Some(relative)
                }
                _ => None,
            })
    })
}

fn extension_asset(request: Request<Vec<u8>>) -> Response<Vec<u8>> {
    let (status, content_type, body) = match is_approved_asset_path(request.uri().path()) {
        Some("m7-probe-extension.html") => {
            (StatusCode::OK, "text/html; charset=utf-8", EXTENSION_HTML)
        }
        Some("m7-probe-extension.js") => (
            StatusCode::OK,
            "application/javascript; charset=utf-8",
            EXTENSION_JS,
        ),
        Some("m7-probe-worker.js") => (
            StatusCode::OK,
            "application/javascript; charset=utf-8",
            WORKER_JS,
        ),
        _ => (
            StatusCode::NOT_FOUND,
            "text/plain; charset=utf-8",
            &b"not found"[..],
        ),
    };
    let mut response = Response::new(body.to_vec());
    *response.status_mut() = status;
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(EXTENSION_CSP),
    );
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

fn main() {
    phase_evidence(false);
    tauri::Builder::default()
        .register_uri_scheme_protocol("alcomd-extension-ui", |_context, request| {
            extension_asset(request)
        })
        .setup(|app| {
            let timeout_handle = app.handle().clone();
            thread::spawn(move || {
                thread::sleep(Duration::from_secs(45));
                eprintln!("m7-webview-probe: timeout");
                timeout_handle.exit(2);
            });

            let result_handle = app.handle().clone();
            WebviewWindowBuilder::new(
                app,
                "m7-isolation-probe",
                WebviewUrl::App("m7-probe-host.html".into()),
            )
            .title("ALCOMD M7 WebView Isolation Probe")
            .inner_size(720.0, 480.0)
            .visible(true)
            .use_https_scheme(true)
            .on_navigation(move |url| {
                if url.host_str() != Some(RESULT_HOST) {
                    return true;
                }

                let fields = url
                    .query_pairs()
                    .map(|(key, value)| (key.into_owned(), value.into_owned()))
                    .collect::<BTreeMap<_, _>>();
                let requested_result = fields.get("result").map(String::as_str);
                let result = if requested_result == Some("pass") {
                    "pass"
                } else {
                    "isolation_failed"
                };
                if write_evidence(result, &fields).is_err() {
                    result_handle.exit(3);
                } else if result == "pass" {
                    result_handle.exit(0);
                } else {
                    result_handle.exit(1);
                }
                false
            })
            .build()?;
            phase_evidence(true);
            Ok(())
        })
        .run(tauri::generate_context!("tauri.m7-probe.conf.json"))
        .expect("failed to run M7 test-only isolation probe");
}
