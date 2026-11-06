//! Validates write-only repository credentials and handles authenticated redirects.
use std::{collections::BTreeMap, time::Duration};

use alcomd_application::{M3Error, M3ErrorCode, RepositoryCredentials};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, LOCATION};
use reqwest::{Client, RequestBuilder, Response, Url};

const MAX_SECRET_BYTES: usize = 2560;
const MAX_HEADERS: usize = 16;

pub(crate) struct RepositoryAuth {
    headers: HeaderMap,
}

fn invalid() -> M3Error {
    M3Error::new(M3ErrorCode::RepositorySourceInvalid)
}

fn unavailable() -> M3Error {
    M3Error::new(M3ErrorCode::RepositoryUnavailable)
}

fn valid_http_url(url: &Url) -> bool {
    matches!(url.scheme(), "http" | "https")
        && url.has_host()
        && url.username().is_empty()
        && url.password().is_none()
}

impl RepositoryAuth {
    pub(crate) fn for_package(
        value: Option<&serde_json::Value>,
        artifact: &Url,
    ) -> Result<Option<Self>, M3Error> {
        let Some(value) = value.filter(|value| !value.is_null()) else {
            return Ok(None);
        };
        let object = value.as_object().ok_or_else(invalid)?;
        if object.is_empty() {
            return Ok(None);
        }
        if !valid_http_url(artifact) || object.len() > MAX_HEADERS {
            return Err(invalid());
        }
        let headers = object
            .iter()
            .map(|(name, value)| Ok((name.clone(), value.as_str().ok_or_else(invalid)?.to_owned())))
            .collect::<Result<BTreeMap<_, _>, M3Error>>()?;
        let auth = Self::new(&RepositoryCredentials { headers })?;
        if auth.contains_url_secret(artifact) {
            return Err(invalid());
        }
        Ok(Some(auth))
    }

    pub(crate) fn new(credentials: &RepositoryCredentials) -> Result<Self, M3Error> {
        if credentials.headers.is_empty()
            || credentials.headers.len() > MAX_HEADERS
            || serde_json::to_vec(credentials)
                .map_err(|_| invalid())?
                .len()
                > MAX_SECRET_BYTES
        {
            return Err(invalid());
        }
        let mut headers = HeaderMap::new();
        for (spelling, value) in &credentials.headers {
            let name = HeaderName::from_bytes(spelling.as_bytes()).map_err(|_| invalid())?;
            if matches!(
                name.as_str(),
                "host"
                    | "connection"
                    | "content-length"
                    | "transfer-encoding"
                    | "proxy-authorization"
                    | "proxy-connection"
                    | "upgrade"
                    | "te"
                    | "trailer"
                    | "if-none-match"
                    | "if-modified-since"
            ) || value.trim().is_empty()
                || !value.is_ascii()
            {
                return Err(invalid());
            }
            let mut value = HeaderValue::from_str(value).map_err(|_| invalid())?;
            value.to_str().map_err(|_| invalid())?;
            value.set_sensitive(true);
            if headers.insert(name, value).is_some() {
                return Err(invalid().with_reason("duplicate HTTP request header name"));
            }
        }
        Ok(Self { headers })
    }

    pub(crate) fn contains_secret(&self, text: &str) -> bool {
        self.headers
            .values()
            .filter_map(|value| value.to_str().ok())
            .map(str::trim)
            .any(|secret| {
                text == secret
                    || (secret.len() >= 8 && text.contains(secret))
                    || secret
                        .split_once(' ')
                        .is_some_and(|(_, token)| token.len() >= 8 && text.contains(token))
            })
    }

    pub(crate) fn contains_url_secret(&self, url: &Url) -> bool {
        let contains_encoded = |component: &str| {
            let mut remaining = component.as_bytes().iter().copied();
            let mut decoded = Vec::with_capacity(component.len());
            while let Some(byte) = remaining.next() {
                if byte == b'%' {
                    let mut lookahead = remaining.clone();
                    if let Some((high, low)) =
                        lookahead
                            .next()
                            .zip(lookahead.next())
                            .and_then(|(high, low)| {
                                char::from(high)
                                    .to_digit(16)
                                    .zip(char::from(low).to_digit(16))
                            })
                    {
                        decoded.push((high * 16 + low) as u8);
                        remaining = lookahead;
                        continue;
                    }
                }
                decoded.push(byte);
            }
            self.contains_secret(&String::from_utf8_lossy(&decoded))
        };
        self.contains_secret(url.as_str())
            || url.path().split('/').any(contains_encoded)
            || (url.path().contains('%') && contains_encoded(url.path()))
            || url
                .query_pairs()
                .any(|(name, value)| self.contains_secret(&name) || self.contains_secret(&value))
    }

    pub(crate) fn check_document(&self, bytes: &[u8]) -> Result<(), M3Error> {
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(
            bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes),
        ) else {
            return Ok(());
        };
        let contains_secret = |text: &str| {
            self.contains_secret(text)
                || (text.contains("://")
                    && Url::parse(text).is_ok_and(|url| self.contains_url_secret(&url)))
        };
        let mut pending = vec![&value];
        while let Some(value) = pending.pop() {
            match value {
                serde_json::Value::String(text) if contains_secret(text) => {
                    return Err(unavailable());
                }
                serde_json::Value::Object(values) => {
                    if values.keys().any(|key| contains_secret(key)) {
                        return Err(unavailable());
                    }
                    pending.extend(values.values());
                }
                serde_json::Value::Array(values) => pending.extend(values),
                _ => {}
            }
        }
        Ok(())
    }
}

pub(crate) fn client(
    timeout: Duration,
    connect_timeout: Duration,
    proxy: &alcomd_application::ConfigProxy,
) -> Result<Client, reqwest::Error> {
    crate::http_clients::with_proxy(
        Client::builder()
            .referer(false)
            .timeout(timeout)
            .connect_timeout(connect_timeout)
            .redirect(reqwest::redirect::Policy::none()),
        proxy,
    )?
    .build()
}

pub(crate) async fn send(
    mut url: Url,
    layers: &[&RepositoryAuth],
    request: impl Fn(Url) -> RequestBuilder,
) -> Result<Response, M3Error> {
    if layers.iter().any(|layer| layer.contains_url_secret(&url)) {
        return Err(unavailable());
    }
    let mut headers = request_headers(layers);
    for _ in 0..=10 {
        let response = request(url.clone())
            .headers(headers.clone())
            .send()
            .await
            .map_err(|error| unavailable().with_source(error.without_url()))?;
        if !matches!(response.status().as_u16(), 301 | 302 | 303 | 307 | 308) {
            return Ok(response);
        }
        let location = response
            .headers()
            .get(LOCATION)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(unavailable)?;
        let next = url.join(location).map_err(|_| unavailable())?;
        if !valid_http_url(&next) || layers.iter().any(|layer| layer.contains_url_secret(&next)) {
            return Err(unavailable());
        }
        if url.origin() != next.origin() {
            // Keep the same redirect header behavior as the v3 HTTP client.
            for name in [
                "authorization",
                "cookie",
                "cookie2",
                "proxy-authorization",
                "www-authenticate",
            ] {
                headers.remove(name);
            }
        }
        url = next;
    }
    Err(unavailable())
}

fn request_headers(layers: &[&RepositoryAuth]) -> HeaderMap {
    let mut headers = HeaderMap::new();
    for layer in layers {
        for (name, value) in &layer.headers {
            headers.insert(name.clone(), value.clone());
        }
    }
    headers
}

pub(crate) async fn save(
    reference: String,
    credentials: RepositoryCredentials,
) -> Result<(), M3Error> {
    RepositoryAuth::new(&credentials)?;
    let bytes = serde_json::to_vec(&credentials).map_err(|_| invalid())?;
    tokio::task::spawn_blocking(move || alcomd_platform::write_credential(&reference, &bytes))
        .await
        .map_err(|_| unavailable())?
        .map_err(|_| unavailable())
}

pub(crate) async fn load(reference: String) -> Result<RepositoryCredentials, M3Error> {
    tokio::task::spawn_blocking(move || load_from_vault(&reference))
        .await
        .map_err(|_| unavailable())?
}

pub(crate) fn load_from_vault(reference: &str) -> Result<RepositoryCredentials, M3Error> {
    let bytes = alcomd_platform::read_credential(reference).map_err(|_| unavailable())?;
    serde_json::from_slice(&bytes).map_err(|_| unavailable())
}

pub(crate) async fn delete(reference: String) -> Result<(), M3Error> {
    tokio::task::spawn_blocking(move || alcomd_platform::delete_credential(&reference))
        .await
        .map_err(|_| unavailable())?
        .map_err(|_| unavailable())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn package_headers_override_by_http_name_on_the_initial_archive_request() {
        let archive = Url::parse("https://zip.example.test/archive.zip").expect("archive URL");
        let repository = RepositoryAuth::new(&RepositoryCredentials {
            headers: BTreeMap::from([
                ("Authorization".into(), "Bearer repository-fixture".into()),
                ("X-Repository".into(), "repository-fixture".into()),
                ("X-Shared".into(), "repository-shared-fixture".into()),
            ]),
        })
        .expect("repository headers");
        let package = RepositoryAuth::for_package(
            Some(&json!({
                "authorization": "Bearer package-fixture",
                "X-Shared": "package-shared-fixture",
                "X-Package": "package-fixture"
            })),
            &archive,
        )
        .expect("valid package headers")
        .expect("package layer");
        let layers = [&repository, &package];
        let zipped = request_headers(&layers);
        assert_eq!(zipped.get_all("authorization").iter().count(), 1);
        assert_eq!(zipped["authorization"], "Bearer package-fixture");
        assert_eq!(zipped["x-repository"], "repository-fixture");
        assert_eq!(zipped.get_all("x-shared").iter().count(), 1);
        assert_eq!(zipped["x-shared"], "package-shared-fixture");
        assert_eq!(zipped["x-package"], "package-fixture");
        assert!(zipped["x-package"].is_sensitive());
        assert_eq!(
            request_headers(&[&repository])["authorization"],
            "Bearer repository-fixture"
        );
    }

    #[test]
    fn package_headers_accept_http_and_reject_invalid_values_without_exposing_them() {
        let insecure = Url::parse("http://zip.example.test/archive.zip").expect("HTTP URL");
        let secure = Url::parse("https://zip.example.test/archive.zip").expect("HTTPS URL");
        assert!(
            RepositoryAuth::for_package(Some(&json!({"Authorization": "fixture"})), &insecure)
                .expect("HTTP artifact is allowed")
                .is_some()
        );
        for invalid in [
            json!({"Host": "fixture"}),
            json!({"X-Example": 42}),
            json!({"X-Example": "newline\nfixture"}),
            json!({"Authorization": "first-fixture", "authorization": "second-fixture"}),
        ] {
            assert!(RepositoryAuth::for_package(Some(&invalid), &secure).is_err());
        }
        assert!(
            RepositoryAuth::for_package(Some(&json!({})), &insecure)
                .expect("no package headers")
                .is_none()
        );
    }
}
