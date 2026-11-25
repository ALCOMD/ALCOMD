//! Network transport and local staging for authenticated self-update assets.

use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use alcomd_platform::{
    open_owned_installation_record, open_plain_file, publish_path_no_replace,
    resolve_plain_storage_directory, sync_directory, validate_installation_record_parent,
};
use anyhow::{Context, anyhow, ensure};
use reqwest::header::{
    ACCEPT, ACCEPT_ENCODING, CONTENT_ENCODING, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue,
    LOCATION,
};
use reqwest::{Client, Response, StatusCode, redirect::Policy};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt as _;
use url::Url;

use super::replay::{ReplayJournal, verify_download_replay};
use super::{
    MAX_UPDATE_RESPONSE_BYTES, MAX_URL_BYTES, UpdateAsset, UpdateRequest, UpdateStream,
    VerifiedUpdate, check_replay, validate_update_asset_url, verify_update,
};
use crate::{
    Channel,
    distribution::{Target, format_digest},
};

const MAX_REDIRECTS: usize = 5;
const FEED_DEADLINE: Duration = Duration::from_secs(30);
const ASSET_DEADLINE: Duration = Duration::from_secs(600);
static NEXT_DOWNLOAD: AtomicU64 = AtomicU64::new(0);

/// A verified, published asset for the caller's separate platform transaction.
pub struct StagedUpdate {
    pub path: PathBuf,
    pub envelope_path: PathBuf,
    pub verified: VerifiedUpdate,
}

/// Downloads a self-update from a trusted HTTPS origin into an existing private directory.
///
/// A 204 response leaves staging and the replay receipt untouched. The asset and signed
/// response are durably staged for offline verification. A retry can reuse exact files.
pub async fn download_update(
    origin: &Url,
    request: &UpdateRequest<'_>,
    download_directory: &Path,
    replay_directory: &Path,
) -> anyhow::Result<Option<StagedUpdate>> {
    ensure!(
        request.previous.is_none(),
        "product update download requires its persisted replay state"
    );
    let started = Instant::now();
    let host = origin
        .host_str()
        .context("missing product update origin host")?;
    let endpoint = feed_endpoint(origin, request.stream)?;
    let client = Client::builder()
        .https_only(true)
        .redirect(Policy::none())
        .referer(false)
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .no_zstd()
        .connect_timeout(Duration::from_secs(10))
        .read_timeout(Duration::from_secs(30))
        .build()
        .map_err(|_| anyhow!("cannot initialize product update transport"))?;
    let body = tokio::time::timeout(FEED_DEADLINE, fetch_feed(&client, endpoint, &[host]))
        .await
        .context("product update feed timed out")??;
    let Some(body) = body else {
        return Ok(None);
    };
    let verified = verify_update(
        &body,
        &UpdateRequest {
            now: request.now + chrono::TimeDelta::from_std(started.elapsed())?,
            ..*request
        },
    )?;
    let replay_directory = replay_directory.to_path_buf();
    let replay_check_directory = replay_directory.clone();
    let acceptance = verified.acceptance();
    tokio::task::spawn_blocking(move || {
        verify_download_replay(&replay_check_directory, acceptance)
    })
    .await
    .context("product update replay check stopped")??;
    let stage = Stage::open(download_directory, verified.asset().name())?;
    let pending = if stage.has_destination()? {
        None
    } else {
        Some(
            tokio::time::timeout(
                ASSET_DEADLINE,
                stream_asset(
                    &client,
                    verified.asset(),
                    request.allowed_asset_hosts,
                    &stage,
                ),
            )
            .await
            .context("product update asset transfer timed out")??,
        )
    };

    let stream = request.stream;
    let installed_version = request.installed_version.clone();
    let hosts: Vec<_> = request
        .allowed_asset_hosts
        .iter()
        .map(|host| (*host).to_owned())
        .collect();
    let initial_time = request.now;
    let staged = tokio::task::spawn_blocking(move || {
        let hosts: Vec<_> = hosts.iter().map(String::as_str).collect();
        let request = UpdateRequest {
            stream,
            installed_version: &installed_version,
            allowed_asset_hosts: &hosts,
            now: initial_time + chrono::TimeDelta::from_std(started.elapsed())?,
            previous: None,
        };
        publish_verified_update(stage, pending, &replay_directory, &body, &request)
    })
    .await
    .context("product update staging task stopped")??;
    Ok(Some(staged))
}

fn publish_verified_update(
    stage: Stage,
    pending: Option<PendingFile>,
    replay_directory: &Path,
    body: &[u8],
    request: &UpdateRequest<'_>,
) -> anyhow::Result<StagedUpdate> {
    let verified = verify_update(body, request)?;
    let acceptance = verified.acceptance();
    let journal = ReplayJournal::open(replay_directory, request.stream)?;
    let previous = journal.previous()?;
    check_replay(acceptance, previous)?;
    stage.ensure_directory()?;
    if let Some(pending) = pending {
        pending.publish(&stage)?;
    }
    drop(open_owned_installation_record(&stage.destination)?);
    verified.asset().verify_file(&stage.destination)?;
    let envelope_path = publish_signed_envelope(&stage, body)?;
    if previous != Some(acceptance) {
        journal.accept(acceptance)?;
    }
    Ok(StagedUpdate {
        path: stage.destination,
        envelope_path,
        verified,
    })
}

fn publish_signed_envelope(asset: &Stage, body: &[u8]) -> anyhow::Result<PathBuf> {
    // The manifest digest controls replay. The envelope digest identifies the
    // exact HTTPS response, including its outer JSON encoding.
    let filename = format!("update-{}.json", format_digest(&Sha256::digest(body)));
    let stage = Stage {
        directory: asset.directory.clone(),
        directory_identity: asset.directory_identity.clone(),
        destination: asset.directory.join(filename),
    };
    stage.ensure_directory()?;
    if !stage.has_destination()? {
        let (pending, mut file) = PendingFile::create(&stage)?;
        file.write_all(body)?;
        file.sync_all()?;
        drop(file);
        pending.publish(&stage)?;
    }
    let file = open_owned_installation_record(&stage.destination)?;
    let mut staged = Vec::new();
    file.take((MAX_UPDATE_RESPONSE_BYTES + 1) as u64)
        .read_to_end(&mut staged)?;
    ensure!(staged == body, "staged signed update response changed");
    Ok(stage.destination)
}

fn feed_endpoint(origin: &Url, stream: UpdateStream) -> anyhow::Result<Url> {
    let host = origin
        .host_str()
        .context("missing product update origin host")?;
    validate_update_asset_url(origin.as_str(), &[host])?;
    ensure!(
        origin.path() == "/" && origin.query().is_none() && stream.target == stream.format.target(),
        "invalid product update origin or feed"
    );
    let channel = match stream.channel {
        Channel::Stable => "stable",
        Channel::Beta => "beta",
    };
    let target = match stream.target {
        Target::Windows => "x86_64-pc-windows-msvc",
        Target::Linux => "x86_64-unknown-linux-gnu",
        Target::MacOs => "aarch64-apple-darwin",
    };
    let mut url = origin.clone();
    url.set_path(&format!(
        "/api/v1/product-updates/{channel}/{target}/{}.json",
        stream.format.feed_name()
    ));
    Ok(url)
}

async fn fetch_feed(client: &Client, url: Url, hosts: &[&str]) -> anyhow::Result<Option<Vec<u8>>> {
    let mut response = fetch(client, url, hosts, "application/json").await?;
    if response.status() == StatusCode::NO_CONTENT {
        return Ok(None);
    }
    ensure!(
        response.status() == StatusCode::OK,
        "product update feed rejected the request"
    );
    let content_type = only_header(response.headers(), CONTENT_TYPE)?
        .and_then(|header| header.to_str().ok())
        .context("product update feed did not return JSON")?;
    ensure!(
        is_json(content_type),
        "product update feed did not return JSON"
    );
    ensure_identity_encoding(&response)?;
    ensure!(
        response
            .content_length()
            .is_none_or(|size| size <= MAX_UPDATE_RESPONSE_BYTES as u64),
        "product update response exceeds the size limit"
    );
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| anyhow!("product update stream was interrupted"))?
    {
        ensure!(
            chunk.len() <= MAX_UPDATE_RESPONSE_BYTES - body.len(),
            "product update response exceeds the size limit"
        );
        body.extend_from_slice(&chunk);
    }
    Ok(Some(body))
}

fn is_json(content_type: &str) -> bool {
    let mut parts = content_type.split(';').map(str::trim);
    parts
        .next()
        .is_some_and(|mime| mime.eq_ignore_ascii_case("application/json"))
        && parts.next().is_none_or(|param| {
            param.split_once('=').is_some_and(|(key, value)| {
                key.trim().eq_ignore_ascii_case("charset")
                    && ["utf-8", "\"utf-8\""]
                        .iter()
                        .any(|expected| value.trim().eq_ignore_ascii_case(expected))
            })
        })
        && parts.next().is_none()
}

async fn fetch(
    client: &Client,
    mut url: Url,
    hosts: &[&str],
    accept: &str,
) -> anyhow::Result<Response> {
    for redirect_count in 0..=MAX_REDIRECTS {
        let response = client
            .get(url.clone())
            .header(ACCEPT, accept)
            .header(ACCEPT_ENCODING, "identity")
            .send()
            .await
            .map_err(|_| anyhow!("cannot reach product update server"))?;
        if !matches!(
            response.status(),
            StatusCode::MOVED_PERMANENTLY
                | StatusCode::FOUND
                | StatusCode::SEE_OTHER
                | StatusCode::TEMPORARY_REDIRECT
                | StatusCode::PERMANENT_REDIRECT
        ) {
            return Ok(response);
        }
        ensure!(
            redirect_count < MAX_REDIRECTS,
            "too many product update redirects"
        );
        let location = only_header(response.headers(), LOCATION)?
            .and_then(|header| header.to_str().ok())
            .context("invalid product update redirect destination")?;
        url = redirect_target(&url, location, hosts)?;
    }
    Err(anyhow!("too many product update redirects"))
}

fn redirect_target(current: &Url, location: &str, hosts: &[&str]) -> anyhow::Result<Url> {
    ensure!(
        location.len() <= MAX_URL_BYTES,
        "invalid product update redirect destination"
    );
    if location.starts_with("https://") {
        return validate_update_asset_url(location, hosts);
    }
    if location.starts_with("//") {
        return validate_update_asset_url(&format!("https:{location}"), hosts);
    }
    ensure!(
        !location.is_empty()
            && location
                .bytes()
                .all(|byte| byte.is_ascii_graphic() && byte != b'\\')
            && location
                .split(['/', '?', '#'])
                .next()
                .is_some_and(|segment| !segment.contains(':')),
        "invalid product update redirect destination"
    );
    let next = current
        .join(location)
        .map_err(|_| anyhow!("invalid product update redirect destination"))?;
    validate_update_asset_url(next.as_str(), hosts)
}

fn only_header(headers: &HeaderMap, name: HeaderName) -> anyhow::Result<Option<&HeaderValue>> {
    let values = headers.get_all(name);
    let mut entries = values.iter();
    let value = entries.next();
    ensure!(
        entries.next().is_none(),
        "duplicate product update response header"
    );
    Ok(value)
}

fn ensure_identity_encoding(response: &Response) -> anyhow::Result<()> {
    let encoding = only_header(response.headers(), CONTENT_ENCODING)?;
    ensure!(
        encoding.is_none_or(|header| header.as_bytes().eq_ignore_ascii_case(b"identity")),
        "unsupported product update content encoding"
    );
    Ok(())
}

async fn stream_asset(
    client: &Client,
    asset: &UpdateAsset,
    hosts: &[&str],
    stage: &Stage,
) -> anyhow::Result<PendingFile> {
    let mut response = fetch(
        client,
        asset.url().clone(),
        hosts,
        "application/octet-stream",
    )
    .await?;
    ensure!(
        response.status() == StatusCode::OK,
        "product update asset request failed"
    );
    ensure_identity_encoding(&response)?;
    ensure!(
        response
            .content_length()
            .is_none_or(|size| size == asset.bytes()),
        "product update asset length does not match the signed manifest"
    );
    let (pending, file) = PendingFile::create(stage)?;
    let mut file = tokio::fs::File::from_std(file);
    let mut bytes = 0;
    let mut hash = Sha256::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| anyhow!("product update stream was interrupted"))?
    {
        ensure!(
            chunk.len() as u64 <= asset.bytes() - bytes,
            "product update asset exceeds its signed length"
        );
        file.write_all(&chunk)
            .await
            .context("cannot write the product update staging file")?;
        hash.update(chunk.as_ref());
        bytes += chunk.len() as u64;
    }
    let digest: [u8; 32] = hash.finalize().into();
    ensure!(
        bytes == asset.bytes() && digest == asset.sha256(),
        "product update asset does not match its signed length and digest"
    );
    file.flush().await?;
    file.sync_all().await?;
    Ok(pending)
}

struct Stage {
    directory: PathBuf,
    directory_identity: Vec<u8>,
    destination: PathBuf,
}

impl Stage {
    fn open(path: &Path, filename: &str) -> anyhow::Result<Self> {
        let (directory, directory_identity) = resolve_plain_storage_directory(path)?;
        let destination = directory.join(filename);
        validate_installation_record_parent(&destination)?;
        Ok(Self {
            directory,
            directory_identity,
            destination,
        })
    }

    fn ensure_directory(&self) -> anyhow::Result<()> {
        validate_installation_record_parent(&self.destination)?;
        ensure!(
            resolve_plain_storage_directory(&self.directory)?.1 == self.directory_identity,
            "product update staging directory changed"
        );
        Ok(())
    }

    fn has_destination(&self) -> anyhow::Result<bool> {
        match open_owned_installation_record(&self.destination) {
            Ok(_) => Ok(true),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error).context("unsafe existing product update staging destination"),
        }
    }
}

struct PendingFile {
    path: PathBuf,
    directory_identity: Vec<u8>,
    file_identity: Vec<u8>,
    keep: bool,
}

impl PendingFile {
    fn create(stage: &Stage) -> anyhow::Result<(Self, File)> {
        stage.ensure_directory()?;
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let path = stage.directory.join(format!(
            ".update-{}-{stamp}-{}.partial",
            std::process::id(),
            NEXT_DOWNLOAD.fetch_add(1, Ordering::Relaxed)
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        let file = options.open(&path)?;
        let (_, file_identity) = open_plain_file(&path)?;
        Ok((
            Self {
                path,
                directory_identity: stage.directory_identity.clone(),
                file_identity,
                keep: false,
            },
            file,
        ))
    }

    fn publish(mut self, stage: &Stage) -> anyhow::Result<()> {
        stage.ensure_directory()?;
        ensure!(
            open_plain_file(&self.path)?.1 == self.file_identity,
            "product update staging file changed"
        );
        match publish_path_no_replace(&self.path, &stage.destination) {
            Ok(()) => {
                // A retry can verify this complete file even if syncing or
                // recording its receipt fails after the move.
                self.keep = true;
                sync_directory(&stage.directory)?;
                Ok(())
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => Ok(()),
            Err(error) => Err(error).context("cannot exclusively publish product update file"),
        }
    }
}

impl Drop for PendingFile {
    fn drop(&mut self) {
        if !self.keep
            && self.path.parent().is_some_and(|parent| {
                resolve_plain_storage_directory(parent)
                    .is_ok_and(|(_, id)| id == self.directory_identity)
            })
            && open_plain_file(&self.path).is_ok_and(|(_, id)| id == self.file_identity)
        {
            let _ = fs::remove_file(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::product_update::UpdateFormat;
    use tokio::io::AsyncReadExt as _;

    #[test]
    fn signed_response_is_atomically_saved_for_offline_verification() -> anyhow::Result<()> {
        #[cfg(windows)]
        let parent = alcomd_platform::local_app_data_directory()?;
        #[cfg(unix)]
        let parent = std::env::temp_dir().canonicalize()?;
        let path = parent.join(format!("alcomd-update-envelope-{}", uuid::Uuid::new_v4()));
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt as _;
            fs::DirBuilder::new().mode(0o700).create(&path)?;
        }
        #[cfg(windows)]
        fs::create_dir(&path)?;
        let stage = Stage::open(&path, "fixture.exe")?;
        let body = b"{ \"manifest\": \"a\", \"signature\": \"b\" }\n";
        let envelope = publish_signed_envelope(&stage, body)?;
        assert_eq!(fs::read(&envelope)?, body);
        assert_eq!(publish_signed_envelope(&stage, body)?, envelope);
        fs::write(&envelope, b"tampered")?;
        assert!(publish_signed_envelope(&stage, body).is_err());
        assert_eq!(fs::read(&envelope)?, b"tampered");
        assert_eq!(fs::read_dir(&path)?.count(), 1);
        fs::remove_file(envelope)?;
        fs::remove_dir(path)?;
        Ok(())
    }

    #[test]
    fn derives_the_exact_feed_and_rejects_untrusted_origin_shapes() {
        let origin = Url::parse("https://updates.example.test/").expect("test origin");
        let mut stream = UpdateStream {
            channel: Channel::Beta,
            target: Target::MacOs,
            format: UpdateFormat::Dmg,
        };
        let feed = feed_endpoint(&origin, stream).expect("Darwin beta feed");
        assert_eq!(
            feed.as_str(),
            "https://updates.example.test/api/v1/product-updates/beta/aarch64-apple-darwin/dmg.json"
        );
        for untrusted in [
            "http://updates.example.test/",
            "https://updates.example.test:8443/",
            "https://user@updates.example.test/",
            "https://updates.example.test/custom/",
            "https://updates.example.test/?query",
            "https://updates.example.test/#fragment",
        ] {
            let url = Url::parse(untrusted).expect("syntactically valid URL");
            assert!(feed_endpoint(&url, stream).is_err(), "{untrusted}");
        }
        stream.format = UpdateFormat::Appimage;
        assert!(feed_endpoint(&origin, stream).is_err());
    }

    #[test]
    fn validates_redirect_targets_before_each_request() {
        let current = Url::parse("https://updates.example.test/one/asset").expect("test origin");
        let hosts = &["updates.example.test", "cdn.example.test"];
        for (source, expected) in [
            ("../two", "https://updates.example.test/two"),
            ("//cdn.example.test/asset", "https://cdn.example.test/asset"),
            (
                "https://cdn.example.test:443/asset",
                "https://cdn.example.test/asset",
            ),
        ] {
            assert_eq!(
                redirect_target(&current, source, hosts)
                    .expect("test redirect")
                    .as_str(),
                expected
            );
        }
        for target in [
            "",
            "https://@cdn.example.test/asset",
            "https://cdn.example.test:444/asset",
            "//external.example.test/asset",
            "http://updates.example.test/asset",
            "https:asset",
            "\\external.example.test/asset",
            " /asset",
            "#fragment",
        ] {
            assert!(
                redirect_target(&current, target, hosts).is_err(),
                "{target}"
            );
        }
    }

    #[test]
    fn requires_one_json_content_type() {
        for content_type in ["application/json", "Application/JSON; charset=\"UTF-8\""] {
            assert!(is_json(content_type), "{content_type}");
        }
        for content_type in [
            "application/json-evil",
            "application/json; charset=iso-8859-1",
            "application/json; charset=utf-8; charset=utf-8",
            "text/html",
        ] {
            assert!(!is_json(content_type), "{content_type}");
        }
        let mut headers = HeaderMap::new();
        headers.append(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.append(CONTENT_TYPE, HeaderValue::from_static("text/html"));
        assert!(only_header(&headers, CONTENT_TYPE).is_err());
    }

    #[tokio::test]
    async fn reads_no_update_json_and_bounded_chunked_responses_from_a_loopback_peer() {
        // Exercise the response parser over real HTTP without changing the
        // production client's HTTPS-only transport.
        let client = Client::builder()
            .no_proxy()
            .redirect(Policy::none())
            .build()
            .expect("loopback HTTP client");
        for (response, expected) in [
            ("HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n".to_owned(), Ok(None)),
            (
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}".to_owned(),
                Ok(Some(b"{}".to_vec())),
            ),
            (
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n{:x}\r\n{}\r\n0\r\n\r\n",
                    MAX_UPDATE_RESPONSE_BYTES + 1,
                    "x".repeat(MAX_UPDATE_RESPONSE_BYTES + 1)
                ),
                Err("product update response exceeds the size limit"),
            ),
        ] {
            let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
                .await
                .expect("loopback server");
            let endpoint = Url::parse(&format!(
                "http://{}/fixture",
                listener.local_addr().expect("loopback address")
            ))
            .expect("loopback endpoint");
            let server = tokio::spawn(async move {
                let (mut connection, _) = listener.accept().await.expect("accept request");
                let mut request = Vec::new();
                while !request.ends_with(b"\r\n\r\n") && request.len() < 2048 {
                    request.push(connection.read_u8().await.expect("read request"));
                }
                assert!(request.starts_with(b"GET /fixture HTTP/1.1\r\n"));
                let _ = connection.write_all(response.as_bytes()).await;
            });
            let result = tokio::time::timeout(
                Duration::from_secs(3),
                fetch_feed(&client, endpoint, &["127.0.0.1"]),
            )
            .await
            .expect("loopback update response");
            match expected {
                Ok(body) => assert_eq!(result.expect("successful update response"), body),
                Err(message) => assert_eq!(
                    result.expect_err("rejected oversized response").to_string(),
                    message
                ),
            }
            server.await.expect("loopback server task");
        }
    }
}
