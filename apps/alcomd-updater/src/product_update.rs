//! Authenticates the v4 product-update response before interpreting its manifest.

use anyhow::{Context, anyhow, ensure};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, TimeDelta, Utc};
use ed25519_dalek::{Signature, VerifyingKey};
use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use url::Url;

use crate::distribution::Target;
use crate::{Channel, PRODUCT, ProductIdentity, decode_hex, release_trust};

mod download;
mod replay;
pub use download::{StagedUpdate, download_update};
pub use replay::verify_and_record_update;

pub const UPDATE_SIGNATURE_DOMAIN: &[u8] = b"ALCOMD product update manifest v1\0";
pub const MAX_UPDATE_RESPONSE_BYTES: usize = 64 * 1024;
const MAX_MANIFEST_BYTES: usize = 32 * 1024;
const MAX_ENCODED_MANIFEST_BYTES: usize = 43_691;
const MAX_ASSET_BYTES: u64 = 512 * 1024 * 1024;
const MAX_SERIAL: u64 = 9_007_199_254_740_991;
const MAX_URL_BYTES: usize = 2048;

/// Self-update formats. The system package manager owns DEB installations.
#[derive(Clone, Copy, Debug, Deserialize, serde::Serialize, Eq, PartialEq, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum UpdateFormat {
    Inno,
    Appimage,
    Dmg,
}

impl UpdateFormat {
    const fn target(self) -> Target {
        match self {
            Self::Inno => Target::Windows,
            Self::Appimage => Target::Linux,
            Self::Dmg => Target::MacOs,
        }
    }

    const fn asset_suffix(self) -> &'static str {
        match self {
            Self::Inno => "windows_x86_64_setup.exe",
            Self::Appimage => "linux_x86_64.AppImage",
            Self::Dmg => "macos_aarch64.dmg",
        }
    }

    const fn feed_name(self) -> &'static str {
        match self {
            Self::Inno => "inno",
            Self::Appimage => "appimage",
            Self::Dmg => "dmg",
        }
    }
}

/// The replay sequence is independent for each feed.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateStream {
    pub channel: Channel,
    pub target: Target,
    pub format: UpdateFormat,
}

/// The last accepted sequence number and digest to persist for one feed.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AcceptedUpdate {
    pub stream: UpdateStream,
    pub serial: u64,
    pub manifest_sha256: [u8; 32],
}

pub struct UpdateRequest<'a> {
    pub stream: UpdateStream,
    pub installed_version: &'a Version,
    pub allowed_asset_hosts: &'a [&'a str],
    pub now: DateTime<Utc>,
    pub previous: Option<AcceptedUpdate>,
}

/// Authenticated metadata. The asset still requires download and content verification.
pub struct VerifiedUpdate {
    version: Version,
    accepted: AcceptedUpdate,
    issued_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    asset: UpdateAsset,
    payload_manifest_sha256: [u8; 32],
}

impl VerifiedUpdate {
    pub fn version(&self) -> &Version {
        &self.version
    }

    pub fn acceptance(&self) -> AcceptedUpdate {
        self.accepted
    }

    pub fn issued_at(&self) -> DateTime<Utc> {
        self.issued_at
    }

    pub fn expires_at(&self) -> DateTime<Utc> {
        self.expires_at
    }

    pub fn asset(&self) -> &UpdateAsset {
        &self.asset
    }

    pub fn payload_manifest_sha256(&self) -> [u8; 32] {
        self.payload_manifest_sha256
    }
}

pub struct UpdateAsset {
    name: String,
    url: Url,
    bytes: u64,
    sha256: [u8; 32],
}

impl UpdateAsset {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn url(&self) -> &Url {
        &self.url
    }

    pub fn bytes(&self) -> u64 {
        self.bytes
    }

    pub fn sha256(&self) -> [u8; 32] {
        self.sha256
    }

    /// Validates a local asset before handing it to a platform transaction.
    pub fn verify_file(&self, path: &std::path::Path) -> anyhow::Result<()> {
        let (file, _) = alcomd_platform::open_plain_file(path)?;
        ensure!(
            crate::hash_exact(file, self.bytes)? == self.sha256,
            "product update asset digest does not match the signed manifest"
        );
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    manifest: String,
    signature: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Manifest {
    schema: u32,
    product_id: String,
    channel: Channel,
    target: Target,
    format: UpdateFormat,
    version: String,
    serial: u64,
    issued_at: String,
    expires_at: String,
    asset: ManifestAsset,
    payload_manifest_sha256: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestAsset {
    name: String,
    url: String,
    bytes: u64,
    sha256: String,
}

#[derive(Deserialize)]
struct Product {
    product: ProductName,
    identity: ProductIdentity,
}

#[derive(Deserialize)]
struct ProductName {
    family_name: String,
    homepage_url: String,
}

/// Returns the update origin configured for the current product.
pub fn product_update_origin() -> anyhow::Result<Url> {
    let product: Product = toml::from_str(PRODUCT).context("invalid embedded product metadata")?;
    Url::parse(&product.product.homepage_url).context("invalid embedded product update origin")
}

/// Accepts the exact envelope bytes a Windows installation transaction will retain.
#[cfg(windows)]
pub fn accept_windows_product_update(
    bytes: &[u8],
    asset: &std::path::Path,
    replay_directory: &std::path::Path,
    channel: Channel,
    installed_version: &Version,
) -> anyhow::Result<(VerifiedUpdate, i64)> {
    let origin = product_update_origin()?;
    let host = origin
        .host_str()
        .context("missing product update origin host")?;
    let now = Utc::now();
    let update = verify_and_record_update(
        replay_directory,
        bytes,
        &UpdateRequest {
            stream: UpdateStream {
                channel,
                target: Target::Windows,
                format: UpdateFormat::Inno,
            },
            installed_version,
            allowed_asset_hosts: &[host],
            now,
            previous: None,
        },
        asset,
    )?;
    Ok((update, now.timestamp_micros()))
}

/// Rechecks only the signed update previously accepted into a durable Windows transaction.
#[cfg(windows)]
pub fn reverify_interrupted_windows_update(
    bytes: &[u8],
    acceptance: AcceptedUpdate,
    accepted_at: i64,
    installed_version: &Version,
) -> anyhow::Result<VerifiedUpdate> {
    ensure!(
        acceptance.stream.target == Target::Windows
            && acceptance.stream.format == UpdateFormat::Inno,
        "interrupted update has the wrong target or format"
    );
    let origin = product_update_origin()?;
    let host = origin
        .host_str()
        .context("missing product update origin host")?;
    let now = DateTime::from_timestamp_micros(accepted_at)
        .context("invalid Windows update acceptance time")?;
    let update = verify_update(
        bytes,
        &UpdateRequest {
            stream: acceptance.stream,
            installed_version,
            allowed_asset_hosts: &[host],
            now,
            previous: Some(acceptance),
        },
    )?;
    ensure!(
        update.acceptance() == acceptance,
        "interrupted update does not match the accepted manifest"
    );
    Ok(update)
}

/// Verifies the body of a successful JSON response with the embedded release key.
///
/// A caller handling HTTP 204 keeps its previous replay state without calling this function.
pub fn verify_update(bytes: &[u8], request: &UpdateRequest<'_>) -> anyhow::Result<VerifiedUpdate> {
    verify_update_with_key(bytes, request, &release_trust()?)
}

fn verify_update_with_key(
    bytes: &[u8],
    request: &UpdateRequest<'_>,
    key: &VerifyingKey,
) -> anyhow::Result<VerifiedUpdate> {
    ensure!(
        bytes.len() <= MAX_UPDATE_RESPONSE_BYTES,
        "product update response exceeds the size limit"
    );
    let Envelope {
        manifest,
        signature,
    } = serde_json::from_slice(bytes).map_err(|_| anyhow!("invalid product update envelope"))?;
    ensure!(
        (2..=MAX_ENCODED_MANIFEST_BYTES).contains(&manifest.len()),
        "invalid encoded product update manifest length"
    );
    let manifest_bytes = URL_SAFE_NO_PAD
        .decode(manifest)
        .map_err(|_| anyhow!("invalid product update manifest encoding"))?;
    ensure!(
        manifest_bytes.len() <= MAX_MANIFEST_BYTES,
        "product update manifest exceeds the size limit"
    );
    let signature = Signature::from_bytes(&strict_hex(&signature)?);
    let mut signed = Vec::with_capacity(UPDATE_SIGNATURE_DOMAIN.len() + manifest_bytes.len());
    signed.extend_from_slice(UPDATE_SIGNATURE_DOMAIN);
    signed.extend_from_slice(&manifest_bytes);
    key.verify_strict(&signed, &signature)
        .context("product update signature is invalid")?;

    let manifest: Manifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|_| anyhow!("invalid product update manifest structure"))?;
    let product: Product = toml::from_str(PRODUCT).context("invalid embedded product metadata")?;
    let stream = UpdateStream {
        channel: manifest.channel,
        target: manifest.target,
        format: manifest.format,
    };
    ensure!(
        manifest.schema == 1
            && manifest.product_id == product.identity.bundle_identifier
            && stream == request.stream
            && stream.target == stream.format.target(),
        "product update manifest does not match the requested feed"
    );
    let version = validate_version(&manifest.version, stream.channel, request.installed_version)?;
    let issued_at = parse_utc_seconds(&manifest.issued_at)?;
    let expires_at = parse_utc_seconds(&manifest.expires_at)?;
    ensure!(
        expires_at > issued_at
            && expires_at - issued_at <= TimeDelta::days(7)
            && issued_at - request.now <= TimeDelta::minutes(5)
            && expires_at > request.now,
        "product update validity window is invalid"
    );

    let acceptance = AcceptedUpdate {
        stream,
        serial: manifest.serial,
        manifest_sha256: Sha256::digest(&manifest_bytes).into(),
    };
    check_replay(acceptance, request.previous)?;
    let expected_name = format!(
        "{}_{version}_{}",
        product.product.family_name,
        stream.format.asset_suffix()
    );
    ensure!(
        manifest.asset.name == expected_name && (1..=160).contains(&manifest.asset.name.len()),
        "product update asset name does not match the requested feed"
    );
    ensure!(
        (1..=MAX_ASSET_BYTES).contains(&manifest.asset.bytes),
        "product update asset size is outside the allowed range"
    );
    let asset = UpdateAsset {
        name: manifest.asset.name,
        url: validate_update_asset_url(&manifest.asset.url, request.allowed_asset_hosts)?,
        bytes: manifest.asset.bytes,
        sha256: strict_hex(&manifest.asset.sha256)?,
    };

    Ok(VerifiedUpdate {
        version,
        accepted: acceptance,
        issued_at,
        expires_at,
        asset,
        payload_manifest_sha256: strict_hex(&manifest.payload_manifest_sha256)?,
    })
}

fn validate_version(raw: &str, channel: Channel, installed: &Version) -> anyhow::Result<Version> {
    ensure!(
        (5..=80).contains(&raw.len()),
        "invalid product update version length"
    );
    let version = Version::parse(raw).context("invalid product update version")?;
    ensure!(
        version.build.is_empty()
            && version.to_string() == raw
            && (channel == Channel::Beta || version.pre.is_empty()),
        "product update version is not valid for the channel"
    );
    ensure!(
        version.cmp_precedence(installed).is_gt(),
        "product update version must be newer than the installed version"
    );
    Ok(version)
}

fn parse_utc_seconds(raw: &str) -> anyhow::Result<DateTime<Utc>> {
    ensure!(
        raw.len() == 20
            && raw.bytes().enumerate().all(|(index, byte)| match index {
                4 | 7 => byte == b'-',
                10 => byte == b'T',
                13 | 16 => byte == b':',
                19 => byte == b'Z',
                _ => byte.is_ascii_digit(),
            }),
        "product update time must have UTC second precision"
    );
    let parsed = DateTime::parse_from_rfc3339(raw).context("invalid product update time")?;
    ensure!(
        parsed.timestamp_subsec_nanos() == 0,
        "invalid product update second"
    );
    Ok(parsed.to_utc())
}

fn check_replay(current: AcceptedUpdate, previous: Option<AcceptedUpdate>) -> anyhow::Result<()> {
    ensure!(
        (1..=MAX_SERIAL).contains(&current.serial),
        "product update serial is outside the allowed range"
    );
    if let Some(previous) = previous {
        ensure!(
            previous.stream == current.stream && (1..=MAX_SERIAL).contains(&previous.serial),
            "product update replay state does not match the requested feed"
        );
        ensure!(
            current.serial > previous.serial
                || (current.serial == previous.serial
                    && current.manifest_sha256 == previous.manifest_sha256),
            "product update replay state rejected the manifest"
        );
    }
    Ok(())
}

/// Checks an absolute asset URL against the approved host list. Apply this to
/// every resolved redirect before requesting it.
pub fn validate_update_asset_url(raw: &str, allowed_hosts: &[&str]) -> anyhow::Result<Url> {
    let authority = raw
        .strip_prefix("https://")
        .and_then(|rest| rest.split(['/', '?', '#']).next())
        .ok_or_else(|| anyhow!("product update asset URL must use HTTPS"))?;
    ensure!(
        raw.len() <= MAX_URL_BYTES
            && !authority.is_empty()
            && !authority.contains('@')
            && raw
                .bytes()
                .all(|byte| byte.is_ascii_graphic() && !b"\"<>\\^`{|}".contains(&byte))
            && raw.split('%').skip(1).all(|tail| tail
                .as_bytes()
                .get(..2)
                .is_some_and(|pair| pair.iter().all(u8::is_ascii_hexdigit))),
        "invalid product update asset URL"
    );
    let url = Url::parse(raw).map_err(|_| anyhow!("invalid product update asset URL"))?;
    ensure!(
        url.scheme() == "https"
            && url.username().is_empty()
            && url.password().is_none()
            && url.fragment().is_none()
            && url.port_or_known_default() == Some(443)
            && url.host_str().is_some_and(|host| {
                allowed_hosts
                    .iter()
                    .any(|allowed| host.eq_ignore_ascii_case(allowed))
            }),
        "product update asset URL violates the download policy"
    );
    Ok(url)
}

fn strict_hex<const N: usize>(value: &str) -> anyhow::Result<[u8; N]> {
    ensure!(
        value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "invalid product update digest or signature encoding"
    );
    decode_hex(value)
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer, SigningKey};
    use serde_json::{Value, json};

    use super::*;
    use crate::distribution::format_digest;

    const HOSTS: &[&str] = &["updates.example.test", "mirror.example.test"];
    const WINDOWS_STABLE: UpdateStream = UpdateStream {
        channel: Channel::Stable,
        target: Target::Windows,
        format: UpdateFormat::Inno,
    };

    fn request(installed: &Version) -> UpdateRequest<'_> {
        UpdateRequest {
            stream: WINDOWS_STABLE,
            installed_version: installed,
            allowed_asset_hosts: HOSTS,
            now: parse_utc_seconds("2026-11-25T12:00:00Z").expect("fixed UTC time"),
            previous: None,
        }
    }

    fn signed_manifest() -> Value {
        json!({
            "schema": 1,
            "productId": "com.vrclearn.alcomd",
            "channel": "stable",
            "target": "x86_64-pc-windows-msvc",
            "format": "inno",
            "version": "4.1.0",
            "serial": 17,
            "issuedAt": "2026-11-25T12:05:00Z",
            "expiresAt": "2026-12-02T12:05:00Z",
            "payloadManifestSha256": "2a".repeat(32),
            "asset": {
                "name": "ALCOMD_4.1.0_windows_x86_64_setup.exe",
                "url": "https://updates.example.test/assets/windows?opaque=%2F",
                "bytes": 536870912_u64,
                "sha256": "1b".repeat(32)
            },
        })
    }

    fn signing_key() -> SigningKey {
        SigningKey::from_bytes(&[73; 32])
    }

    fn envelope(raw: &[u8], domain: &[u8]) -> Value {
        let signature = signing_key().sign(&[domain, raw].concat());
        json!({
            "manifest": URL_SAFE_NO_PAD.encode(raw),
            "signature": format_digest(&signature.to_bytes())
        })
    }

    fn verify_body(body: &[u8], request: &UpdateRequest<'_>) -> anyhow::Result<VerifiedUpdate> {
        verify_update_with_key(body, request, &signing_key().verifying_key())
    }

    fn verify_raw(raw: &[u8], request: &UpdateRequest<'_>) -> anyhow::Result<VerifiedUpdate> {
        let body = envelope(raw, UPDATE_SIGNATURE_DOMAIN).to_string();
        verify_body(body.as_bytes(), request)
    }

    fn verify_fixture(data: &Value, request: &UpdateRequest<'_>) -> anyhow::Result<VerifiedUpdate> {
        verify_raw(data.to_string().as_bytes(), request)
    }

    #[test]
    fn verifies_bytes_and_scoped_replay() {
        let installed = Version::new(4, 0, 0);
        let mut request = request(&installed);
        let mut manifest = signed_manifest();
        let accepted = verify_fixture(&manifest, &request).expect("signed update");
        assert_eq!(accepted.version(), &Version::new(4, 1, 0));
        assert_eq!(accepted.asset().bytes(), MAX_ASSET_BYTES);
        assert_eq!(accepted.asset().sha256(), [0x1b; 32]);
        assert_eq!(accepted.payload_manifest_sha256(), [0x2a; 32]);
        assert_eq!(accepted.acceptance().serial, 17);
        request.previous = Some(accepted.acceptance());
        assert!(verify_fixture(&manifest, &request).is_ok());
        assert!(
            verify_raw(
                &serde_json::to_vec_pretty(&manifest).expect("JSON"),
                &request
            )
            .is_err()
        );
        manifest["serial"] = json!(16);
        assert!(verify_fixture(&manifest, &request).is_err());
        manifest["serial"] = json!(MAX_SERIAL);
        assert!(verify_fixture(&manifest, &request).is_ok());
        manifest["serial"] = json!(MAX_SERIAL + 1);
        assert!(verify_fixture(&manifest, &request).is_err());
        manifest["serial"] = json!(0);
        assert!(verify_fixture(&manifest, &request).is_err());
        manifest["serial"] = json!(18);
        if let Some(previous) = &mut request.previous {
            previous.stream.channel = Channel::Beta;
        }
        assert!(verify_fixture(&manifest, &request).is_err());
    }

    #[test]
    fn authenticates_before_parsing_and_rejects_duplicate_fields() {
        let installed = Version::new(4, 0, 0);
        let request = request(&installed);
        let wrong_domain = envelope(b"not JSON", b"ALCOMD product payload v1\0").to_string();
        let result = verify_body(wrong_domain.as_bytes(), &request);
        assert_eq!(
            result.err().expect("wrong domain").to_string(),
            "product update signature is invalid"
        );
        let raw = signed_manifest().to_string();
        let duplicate = raw.replacen(r#""serial":17"#, r#""serial":17,"seri\u0061l":17"#, 1);
        assert_ne!(duplicate, raw);
        assert!(verify_raw(duplicate.as_bytes(), &request).is_err());
        let duplicate_asset =
            raw.replacen(r#""bytes":536870912"#, r#""bytes":536870912,"bytes":1"#, 1);
        assert_ne!(duplicate_asset, raw);
        assert!(verify_raw(duplicate_asset.as_bytes(), &request).is_err());
        let body = envelope(raw.as_bytes(), UPDATE_SIGNATURE_DOMAIN).to_string();
        let duplicate_envelope = format!("{{\"signature\":\"duplicate\",{}", &body[1..]);
        assert!(verify_body(duplicate_envelope.as_bytes(), &request).is_err());

        let mut invalid = envelope(raw.as_bytes(), UPDATE_SIGNATURE_DOMAIN);
        invalid["manifest"] = json!(format!(
            "{}=",
            invalid["manifest"].as_str().expect("encoded body")
        ));
        assert!(verify_body(invalid.to_string().as_bytes(), &request).is_err());
        assert!(verify_body(&vec![b' '; MAX_UPDATE_RESPONSE_BYTES + 1], &request).is_err());
        assert!(verify_raw(&vec![b' '; MAX_MANIFEST_BYTES + 1], &request).is_err());
    }

    #[test]
    fn matches_requested_channel_and_platform_asset() {
        let installed = Version::new(4, 0, 0);
        let mut request = request(&installed);
        let mut manifest = signed_manifest();
        for version in ["4.0.0", "3.9.0", "04.1.0", "4.1.0+meta", "4.1.0-beta.1"] {
            manifest["version"] = json!(version);
            manifest["asset"]["name"] = json!(format!("ALCOMD_{version}_windows_x86_64_setup.exe"));
            assert!(verify_fixture(&manifest, &request).is_err(), "{version}");
        }
        manifest["channel"] = json!("beta");
        manifest["version"] = json!("4.1.0-beta.1");
        manifest["asset"]["name"] = json!("ALCOMD_4.1.0-beta.1_windows_x86_64_setup.exe");
        assert!(verify_fixture(&manifest, &request).is_err());
        request.stream.channel = Channel::Beta;
        assert!(verify_fixture(&manifest, &request).is_ok());
        manifest["target"] = json!("x86_64-unknown-linux-gnu");
        request.stream.target = Target::Linux;
        assert!(verify_fixture(&manifest, &request).is_err());
        manifest["format"] = json!("deb");
        assert!(verify_fixture(&manifest, &request).is_err());
        manifest["format"] = json!("appimage");
        request.stream.format = UpdateFormat::Appimage;
        assert!(verify_fixture(&manifest, &request).is_err());
        manifest["asset"]["name"] = json!("ALCOMD_4.1.0-beta.1_linux_x86_64.AppImage");
        assert!(verify_fixture(&manifest, &request).is_ok());
        manifest["asset"]["bytes"] = json!(MAX_ASSET_BYTES + 1);
        assert!(verify_fixture(&manifest, &request).is_err());
    }

    #[test]
    fn validates_utc_windows_and_exact_approved_https_authorities() {
        let installed = Version::new(4, 0, 0);
        let request = request(&installed);
        for (field, value) in [
            ("issuedAt", "2026-11-25T12:05:01Z"),
            ("issuedAt", "2026-11-25T12:04:60Z"),
            ("issuedAt", "2026-11-25T12:05:00.0Z"),
            ("issuedAt", "2026-11-25T12:05:00+00:00"),
            ("expiresAt", "2026-12-02T12:05:01Z"),
            ("expiresAt", "2026-11-25T12:00:00Z"),
            ("expiresAt", "2026-11-25T12:05:00Z"),
        ] {
            let mut manifest = signed_manifest();
            manifest[field] = json!(value);
            assert!(
                verify_fixture(&manifest, &request).is_err(),
                "{field} {value}"
            );
        }
        let accepted = validate_update_asset_url("https://MIRROR.example.test:443/a?q=%2f", HOSTS)
            .expect("approved TLS host");
        assert_eq!(accepted.host_str(), Some("mirror.example.test"));
        for url in [
            "http://updates.example.test/a",
            "https:///updates.example.test/a",
            "https://user@updates.example.test/a",
            "https://@updates.example.test/a",
            "https://updates.example.test.evil.test/a",
            "https://updates.example.test:8443/a",
            "https://updates.example.test/a#fragment",
            "https://updates.example.test/a%ZZ",
            "https://updates.example.test\\@evil.test/a",
        ] {
            assert!(validate_update_asset_url(url, HOSTS).is_err(), "{url}");
        }
    }
}
