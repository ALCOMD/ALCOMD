//! Bounded anonymous HTTPS transport shared with extension preparation.
//! It reuses the proxy/client manager and immutable package cache, not VPM parsers.
use crate::{
    cache::PackageCache,
    http_clients::{ClientPair, HttpClients, with_proxy},
};
use alcomd_application::{
    ExtensionSourceBytes, ExtensionSourceFetcher, ExtensionSourceFuture, M6Error, M6ErrorCode,
    M7OfficialApplication,
};
use reqwest::{Client, Url, redirect::Policy};
use std::{path::PathBuf, time::Duration};
#[derive(Clone)]
pub struct ExtensionSourceHttp {
    http: HttpClients,
    cache: PackageCache,
}
impl ExtensionSourceHttp {
    pub fn new(root: PathBuf, settings: Option<M7OfficialApplication>) -> Result<Self, M6Error> {
        let http = HttpClients::new(settings, |proxy| build_client(proxy, None))
            .map_err(|_| unavailable())?;
        let cache = PackageCache::for_extension_source(root, http.clone()).map_err(map_cache)?;
        Ok(Self { http, cache })
    }
    /// Isolated HTTPS fixture trust only; no production command/environment bypass.
    #[cfg(feature = "test-kill-gates")]
    pub fn with_test_certificate(root: PathBuf, der: &[u8]) -> Result<Self, M6Error> {
        let certificate = reqwest::Certificate::from_der(der).map_err(|_| invalid())?;
        let client = build_client(&alcomd_application::ConfigProxy::Direct, Some(certificate))
            .map_err(|_| unavailable())?
            .anonymous;
        let http = HttpClients::fixed_for_test(client);
        let cache = PackageCache::for_extension_source(root, http.clone()).map_err(map_cache)?;
        Ok(Self { http, cache })
    }
}
impl ExtensionSourceFetcher for ExtensionSourceHttp {
    fn normalize_url(&self, url: &str) -> Result<String, M6Error> {
        Ok(https(url)?.to_string())
    }
    fn index<'a>(&'a self, url: &'a str) -> ExtensionSourceFuture<'a, ExtensionSourceBytes> {
        Box::pin(async move {
            let url = https(url)?;
            let _transfer =
                tokio::time::timeout(Duration::from_secs(30), crate::HTTP_DOWNLOADS.acquire())
                    .await
                    .map_err(|_| unavailable())?
                    .map_err(|_| unavailable())?;
            let client = self
                .http
                .for_request()
                .await
                .map_err(|_| unavailable())?
                .anonymous;
            let mut response = client
                .get(url.clone())
                .timeout(Duration::from_secs(30))
                .send()
                .await
                .map_err(|e| unavailable().with_source(e.without_url()))?;
            if response.status() != reqwest::StatusCode::OK {
                return Err(unavailable());
            }
            if response
                .content_length()
                .is_some_and(|size| size > 1_048_576)
            {
                return Err(invalid());
            }
            let mut bytes = Vec::new();
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|e| unavailable().with_source(e.without_url()))?
            {
                if bytes.len().saturating_add(chunk.len()) > 1_048_576 {
                    return Err(invalid());
                }
                bytes.extend_from_slice(&chunk);
            }
            Ok(ExtensionSourceBytes {
                url: url.to_string(),
                bytes,
            })
        })
    }
    fn archive<'a>(&'a self, url: &'a str, sha256: [u8; 32]) -> ExtensionSourceFuture<'a, PathBuf> {
        Box::pin(async move {
            let url = https(url)?;
            self.cache
                .get(sha256, url.as_str(), false)
                .await
                .map_err(map_cache)
        })
    }
}
fn https(value: &str) -> Result<Url, M6Error> {
    let authority = value
        .get(8..)
        .and_then(|tail| tail.split(['/', '?', '#']).next());
    if !value
        .get(..8)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("https://"))
        || authority.is_none_or(|authority| authority.contains('@'))
        || value.len() > 8192
        || value.bytes().any(|b| b.is_ascii_control())
    {
        return Err(invalid());
    }
    let url = Url::parse(value).map_err(|_| invalid())?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || url.as_str().len() > 8192
    {
        return Err(invalid());
    }
    Ok(url)
}
fn map_cache(error: crate::CacheError) -> M6Error {
    use crate::CacheErrorCode as C;
    let code = match error.code() {
        C::IntegrityMismatch | C::Corrupt => M6ErrorCode::IntegrityMismatch,
        C::DownloadTooLarge => M6ErrorCode::DownloadTooLarge,
        C::InvalidUrl => M6ErrorCode::SourceInvalid,
        C::QuotaExceeded => M6ErrorCode::ResourceLimit,
        _ => M6ErrorCode::SourceUnavailable,
    };
    M6Error::new(code).with_source(error)
}
fn invalid() -> M6Error {
    M6Error::new(M6ErrorCode::SourceInvalid)
}
fn unavailable() -> M6Error {
    M6Error::new(M6ErrorCode::SourceUnavailable)
}

fn build_client(
    proxy: &alcomd_application::ConfigProxy,
    certificate: Option<reqwest::Certificate>,
) -> Result<ClientPair, reqwest::Error> {
    let mut builder = Client::builder()
        .referer(false)
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(600))
        .redirect(Policy::custom(|attempt| {
            if attempt.previous().len() > 10 || https(attempt.url().as_str()).is_err() {
                attempt.error("extension source redirect rejected")
            } else {
                attempt.follow()
            }
        }));
    if let Some(certificate) = certificate {
        builder = builder.add_root_certificate(certificate);
    }
    let client = with_proxy(builder, proxy)?.build()?;
    Ok(ClientPair {
        anonymous: client.clone(),
        authenticated: client,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_urls_preserve_query_and_reject_untrusted_transport_forms() {
        let url = https("https://EXAMPLE.invalid:443/f?a=2&a=1&x=%2F").expect("HTTPS URL");
        assert_eq!(url.as_str(), "https://example.invalid/f?a=2&a=1&x=%2F");
        for invalid in [
            "http://example.invalid/f",
            "file:///C:/f",
            "https://user@example.invalid/f",
            "https://@example.invalid/f",
            "https:example.invalid/f",
            "https://user:secret@example.invalid/f",
            "https://example.invalid/f#x",
            "https://example.invalid/f\n",
        ] {
            assert!(https(invalid).is_err());
        }
        assert!(https(&format!("https://example.invalid/{}", "a".repeat(8192))).is_err());
    }
}
