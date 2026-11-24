//! Validates the complete local payload a verified bridge is about to stage.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, ensure};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub mod distribution;

const MANIFEST: &str = "bridge-payload.json";
const SIGNATURE: &str = "bridge-payload.sig";
const PRODUCT: &str = include_str!("../../../alcomd.product.toml");
const MANIFEST_LIMIT: u64 = 1024 * 1024;
const MAX_FILES: usize = 4096;
const MAX_ENTRY: u64 = 2 * 1024 * 1024 * 1024;
const MAX_CONTENT: u64 = 4 * MAX_ENTRY;
const MAX_PATH_DEPTH: usize = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    Stable,
    Beta,
}

#[derive(Debug, Deserialize)]
struct Product {
    identity: ProductIdentity,
    binaries: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct ProductIdentity {
    bundle_identifier: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Payload {
    schema: u32,
    product_id: String,
    version: semver::Version,
    target: String,
    channel: Channel,
    files: Vec<SignedFile>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SignedFile {
    path: String,
    size: u64,
    sha256: String,
}

const RUNTIME_BINARY_ROLES: &[&str] = &[
    "daemon",
    "gui",
    "cli",
    "mcp",
    "extension_host",
    "bootstrap",
    "updater",
];

fn bridge_binary_roles() -> impl Iterator<Item = &'static str> {
    RUNTIME_BINARY_ROLES.iter().copied().chain(["migration_v3"])
}

/// Immutable authority for one host-specific, digest-checked and signed staging directory.
#[derive(Debug)]
pub struct VerifiedPayload {
    root: PathBuf,
    identity: Vec<u8>,
    manifest_sha256: [u8; 32],
    version: semver::Version,
    binaries: BTreeMap<String, PathBuf>,
    file_digests: BTreeMap<String, [u8; 32]>,
}

impl VerifiedPayload {
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn identity(&self) -> &[u8] {
        &self.identity
    }

    pub fn manifest_sha256(&self) -> [u8; 32] {
        self.manifest_sha256
    }

    pub fn version(&self) -> &semver::Version {
        &self.version
    }

    pub fn file(&self, relative: &str) -> Option<PathBuf> {
        self.file_digests
            .contains_key(relative)
            .then(|| self.root.join(relative))
    }

    pub fn file_sha256(&self, relative: &str) -> Option<[u8; 32]> {
        self.file_digests.get(relative).copied()
    }

    pub fn binary(&self, role: &str) -> Option<PathBuf> {
        self.binaries
            .get(role)
            .map(|relative| self.root.join(relative))
    }
}

/// Fails closed when the release provider has not embedded the application's public trust key.
///
/// This is the v4 bridge payload authority. The already published v3 updater authenticates
/// the bridge installer using its own pre-existing trust and channel metadata.
pub fn verify_staged_payload(path: &Path, channel: Channel) -> anyhow::Result<VerifiedPayload> {
    verify_payload(path, channel, &release_trust()?)
}

fn release_trust() -> anyhow::Result<VerifyingKey> {
    let key = option_env!("ALCOMD_RELEASE_PUBLIC_KEY_HEX")
        .context("release verification trust is unavailable in this build")?;
    Ok(VerifyingKey::from_bytes(&decode_hex(key)?)?)
}

/// Verifies an externally produced platform health report using the release trust anchor.
pub fn verify_health_report(bytes: &[u8], signature: &[u8]) -> anyhow::Result<()> {
    verify_health_report_with_key(bytes, signature, &release_trust()?)
}

fn verify_health_report_with_key(
    bytes: &[u8],
    signature: &[u8],
    key: &VerifyingKey,
) -> anyhow::Result<()> {
    let signature = Signature::from_bytes(&decode_hex(std::str::from_utf8(signature)?.trim_end())?);
    let mut message = b"ALCOMD bridge health v1\0".to_vec();
    message.extend_from_slice(bytes);
    key.verify_strict(&message, &signature)
        .context("platform health report signature is invalid")
}

fn verify_payload(
    path: &Path,
    channel: Channel,
    key: &VerifyingKey,
) -> anyhow::Result<VerifiedPayload> {
    let (root, identity) = alcomd_platform::resolve_plain_storage_directory(path)?;
    let bytes = read_limited(&root.join(MANIFEST), MANIFEST_LIMIT)?;
    let signature = read_limited(&root.join(SIGNATURE), 130)?;
    let signature =
        Signature::from_bytes(&decode_hex(std::str::from_utf8(&signature)?.trim_end())?);
    key.verify_strict(&bytes, &signature)
        .context("staged payload signature is invalid")?;
    let payload: Payload =
        serde_json::from_slice(&bytes).context("staged payload manifest is invalid")?;
    let product: Product = toml::from_str(PRODUCT).context("product identity is unavailable")?;
    ensure!(
        payload.schema == 1
            && payload.product_id == product.identity.bundle_identifier
            && payload.version.major == 4
            && payload.target == format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
            && payload.channel == channel
            && (channel == Channel::Beta || payload.version.pre.is_empty()),
        "staged payload does not match the product, version series, host target or channel"
    );
    let mut observed = BTreeSet::new();
    collect_files(&root, &root, 0, &mut observed, None)?;
    let file_digests = verify_files(
        &root,
        payload.files,
        observed,
        [MANIFEST, SIGNATURE],
        |_, _| Ok(()),
    )?;

    let mut binaries = BTreeMap::new();
    for role in bridge_binary_roles() {
        let base = product
            .binaries
            .get(role)
            .context("required binary name missing from product identity")?;
        let name = format!("{base}{}", std::env::consts::EXE_SUFFIX);
        ensure!(
            file_digests.contains_key(&name),
            "staged payload is missing a required product binary"
        );
        binaries.insert(role.to_owned(), PathBuf::from(name));
    }
    ensure!(
        alcomd_platform::resolve_plain_storage_directory(&root)?.1 == identity,
        "staged payload directory changed during verification"
    );
    Ok(VerifiedPayload {
        root,
        identity,
        manifest_sha256: Sha256::digest(&bytes).into(),
        version: payload.version,
        binaries,
        file_digests,
    })
}

fn verify_files(
    root: &Path,
    files: Vec<SignedFile>,
    mut observed: BTreeSet<String>,
    metadata: [&str; 2],
    inspect: impl Fn(&str, &mut fs::File) -> anyhow::Result<()>,
) -> anyhow::Result<BTreeMap<String, [u8; 32]>> {
    ensure!(
        files.len() <= MAX_FILES && metadata.iter().all(|name| observed.remove(*name)),
        "staged payload has too many files or missing metadata"
    );
    let mut folded_names = metadata
        .map(str::to_ascii_lowercase)
        .into_iter()
        .collect::<BTreeSet<_>>();
    let mut total = 0_u64;
    let mut digests = BTreeMap::new();
    for entry in files {
        ensure!(valid_relative(&entry.path), "invalid staged payload path");
        ensure!(
            folded_names.insert(entry.path.to_ascii_lowercase()),
            "duplicate staged payload path"
        );
        ensure!(observed.remove(&entry.path), "missing staged payload file");
        check_payload_size(&mut total, entry.size)?;
        let expected = decode_hex(&entry.sha256)?;
        let (mut file, _) = alcomd_platform::open_plain_file(&root.join(&entry.path))?;
        inspect(&entry.path, &mut file)?;
        ensure!(
            hash_exact(file, entry.size)? == expected,
            "staged payload file hash mismatch"
        );
        digests.insert(entry.path, expected);
    }
    ensure!(observed.is_empty(), "unlisted staged payload files");
    Ok(digests)
}

fn check_payload_size(total: &mut u64, size: u64) -> anyhow::Result<()> {
    *total = total
        .checked_add(size)
        .context("staged payload exceeds its size limit")?;
    ensure!(
        size <= MAX_ENTRY && *total <= MAX_CONTENT,
        "staged payload exceeds its size limit"
    );
    Ok(())
}

fn valid_relative(path: &str) -> bool {
    path.len() <= 255
        && path.split('/').count() <= MAX_PATH_DEPTH
        && path.split('/').all(|part| {
            !part.is_empty()
                && !matches!(part, "." | "..")
                && !part.ends_with([' ', '.'])
                && part
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.' | b' '))
        })
}

fn collect_files(
    root: &Path,
    directory: &Path,
    depth: usize,
    output: &mut BTreeSet<String>,
    allowed_directories: Option<&[&str]>,
) -> anyhow::Result<()> {
    ensure!(
        depth < MAX_PATH_DEPTH,
        "staged payload exceeds its path depth limit"
    );
    alcomd_platform::resolve_plain_storage_directory(directory)?;
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let relative = path
            .strip_prefix(root)?
            .to_str()
            .context("staged payload path is not Unicode")?;
        #[cfg(windows)]
        let relative = relative.replace('\\', "/");
        #[cfg(not(windows))]
        let relative = relative.to_owned();
        ensure!(valid_relative(&relative), "invalid staged payload path");
        let attributes = entry.file_type()?;
        if attributes.is_dir() {
            ensure!(
                allowed_directories.is_none_or(|dirs| dirs.contains(&relative.as_str())),
                "unexpected staged payload directory"
            );
            collect_files(root, &path, depth + 1, output, allowed_directories)?;
        } else {
            // Reparse points and special files are rejected by the same no-follow check.
            alcomd_platform::open_plain_file(&path)?;
            ensure!(
                output.len() < MAX_FILES + 2 && output.insert(relative),
                "staged payload exceeds its file limit"
            );
        }
    }
    Ok(())
}

pub fn read_limited(path: &Path, limit: u64) -> anyhow::Result<Vec<u8>> {
    let (file, _) = alcomd_platform::open_plain_file(path)?;
    ensure!(
        file.metadata()?.len() <= limit,
        "file exceeds its size limit"
    );
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    ensure!(
        u64::try_from(bytes.len())? <= limit,
        "file exceeds its size limit"
    );
    Ok(bytes)
}

pub fn hash_exact(file: fs::File, expected_size: u64) -> anyhow::Result<[u8; 32]> {
    ensure!(file.metadata()?.len() == expected_size, "file size changed");
    let mut digest = Sha256::new();
    let bytes_read = std::io::copy(
        &mut file.take(expected_size.checked_add(1).context("file too large")?),
        &mut HashWriter(&mut digest),
    )?;
    ensure!(bytes_read == expected_size, "file size changed");
    Ok(digest.finalize().into())
}

struct HashWriter<'a>(&'a mut Sha256);

impl Write for HashWriter<'_> {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.0.update(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub fn decode_hex<const N: usize>(value: &str) -> anyhow::Result<[u8; N]> {
    ensure!(
        value.len() == N * 2 && value.is_ascii(),
        "invalid encoded digest or signature"
    );
    let mut decoded = [0_u8; N];
    for (index, byte) in decoded.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
            .context("invalid encoded digest or signature")?;
    }
    Ok(decoded)
}

#[cfg(test)]
mod tests;
