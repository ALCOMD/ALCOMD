use std::{thread, time::Duration};

use tauri::{
    WebviewUrl, WebviewWindowBuilder,
    http::{HeaderValue, Request, Response, StatusCode, header},
};

const RESULT_HOST: &str = "m7-probe-result.invalid";
const ASSET_PREFIX: &str = "/v1/s/4bb96884909c4b06a22ef12d35ebf3c1/dev.example.m7-probe/0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef/";
const EXTENSION_HTML: &[u8] = include_bytes!("../../m7-probe-dist/m7-probe-extension.html");
const EXTENSION_JS: &[u8] = include_bytes!("../../m7-probe-dist/m7-probe-extension.js");
const EXTENSION_CSP: &str = "default-src 'none'; script-src 'self'; style-src 'none'; connect-src 'none'; img-src 'none'; media-src 'none'; object-src 'none'; child-src 'none'; frame-src 'none'; worker-src 'none'; manifest-src 'none'; form-action 'none'; base-uri 'none'; frame-ancestors https://tauri.localhost tauri://localhost";

fn result_path() -> std::io::Result<std::path::PathBuf> {
    Ok(std::env::current_exe()?.with_file_name("m7-webview-probe-result.json"))
}

fn record_phase(phase: &str) {
    if let Ok(path) = result_path() {
        let evidence = format!("{{\"schema\":1,\"result\":\"{phase}\"}}\n");
        let _ = std::fs::write(path, evidence);
    }
}

fn extension_asset(request: Request<Vec<u8>>) -> Response<Vec<u8>> {
    let relative = request.uri().path().strip_prefix(ASSET_PREFIX);
    let (status, content_type, body) = match relative {
        Some("m7-probe-extension.html") => {
            (StatusCode::OK, "text/html; charset=utf-8", EXTENSION_HTML)
        }
        Some("m7-probe-extension.js") => (
            StatusCode::OK,
            "application/javascript; charset=utf-8",
            EXTENSION_JS,
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
    record_phase("started");
    tauri::Builder::default()
        .register_uri_scheme_protocol("alcomd-extension-ui", |_context, request| {
            extension_asset(request)
        })
        .setup(|app| {
            record_phase("setup");
            let timeout_handle = app.handle().clone();
            thread::spawn(move || {
                thread::sleep(Duration::from_secs(30));
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
            .visible(false)
            .use_https_scheme(true)
            .on_navigation(move |url| {
                if url.host_str() != Some(RESULT_HOST) {
                    return true;
                }

                let result = url
                    .query_pairs()
                    .find_map(|(key, value)| (key == "result").then(|| value.into_owned()));
                let failed = url
                    .query_pairs()
                    .find_map(|(key, value)| (key == "failed").then(|| value.into_owned()))
                    .unwrap_or_else(|| "missing-result-details".to_owned());
                match result.as_deref() {
                    Some("pass") => {
                        let evidence =
                            "{\"schema\":1,\"container\":\"sandboxed-iframe\",\"result\":\"pass\"}\n";
                        if result_path()
                            .and_then(|path| std::fs::write(path, evidence))
                            .is_err()
                        {
                            result_handle.exit(3);
                            return false;
                        }
                        result_handle.exit(0);
                    }
                    _ => {
                        let safe_failed = failed.replace(['"', '\\'], "");
                        let evidence = format!(
                            "{{\"schema\":1,\"container\":\"sandboxed-iframe\",\"result\":\"fail\",\"failed\":\"{safe_failed}\"}}\n"
                        );
                        if result_path()
                            .and_then(|path| std::fs::write(path, evidence))
                            .is_err()
                        {
                            result_handle.exit(3);
                            return false;
                        }
                        result_handle.exit(1);
                    }
                }
                false
            })
            .build()?;
            record_phase("window-built");
            Ok(())
        })
        .run(tauri::generate_context!("tauri.m7-probe.conf.json"))
        .expect("failed to run M7 test-only isolation probe");
}
