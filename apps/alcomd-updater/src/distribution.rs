//! Offline validation for a complete v4 runtime on a supported release target.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use anyhow::{Context, ensure};
use ed25519_dalek::{Signature, VerifyingKey};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{
    MANIFEST_LIMIT, MAX_FILES, PRODUCT, Product, RUNTIME_BINARY_ROLES, SignedFile,
    check_payload_size, collect_files, decode_hex, hash_exact, read_limited, release_trust,
    verify_files,
};

pub use alcomd_platform::PRODUCT_PAYLOAD_MANIFEST_NAME as MANIFEST_NAME;
pub const SIGNATURE_NAME: &str = "product-payload.sig";
pub const SIGNATURE_DOMAIN: &[u8] = b"ALCOMD product payload v1\0";
pub const WINDOWS_CRT_FILE: &str = "vcruntime140.dll";
pub const WINDOWS_UPDATE_STAGE_PREFIX: &str = ".update-";
const LICENSE: &str = "LICENSE";
const NOTICES: &str = "THIRD_PARTY_NOTICES.md";
const DISCORD_PACKAGE: &str = "extensions/discord.alcomdextension";
const FIXTURE_COMMIT: &str = "0000000000000000000000000000000000000000";
const DIRECTORIES: &[&str] = &["extensions", "licenses"];
const WINDOWS_SIDECARS: &[&str] = &[WINDOWS_CRT_FILE, "vcruntime140_1.dll", "WebView2Loader.dll"];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, clap::ValueEnum)]
pub enum Target {
    #[value(name = "x86_64-pc-windows-msvc")]
    #[serde(rename = "x86_64-pc-windows-msvc")]
    Windows,
    #[value(name = "x86_64-unknown-linux-gnu")]
    #[serde(rename = "x86_64-unknown-linux-gnu")]
    Linux,
    #[value(name = "aarch64-apple-darwin")]
    #[serde(rename = "aarch64-apple-darwin")]
    MacOs,
}

impl Target {
    pub fn executable_suffix(self) -> &'static str {
        match self {
            Self::Windows => ".exe",
            Self::Linux | Self::MacOs => "",
        }
    }
}

pub struct Candidate {
    pub target: Target,
    pub version: Version,
    pub source_commit: String,
}

impl Candidate {
    fn validate(&self, purpose: Purpose) -> anyhow::Result<()> {
        ensure!(
            self.version.major == 4 && self.version.build.is_empty(),
            "product payload version must be in the v4 release series without build metadata"
        );
        validate_source_commit(&self.source_commit, purpose)
    }
}

fn validate_source_commit(source_commit: &str, purpose: Purpose) -> anyhow::Result<()> {
    ensure!(
        source_commit.len() == 40
            && source_commit.bytes().all(is_lower_hex)
            && (source_commit == FIXTURE_COMMIT) == (purpose == Purpose::Fixture),
        "invalid product payload source commit"
    );
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Purpose {
    Release,
    Fixture,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Payload {
    schema: u32,
    product_id: String,
    target: Target,
    version: Version,
    source_commit: String,
    purpose: Purpose,
    files: Vec<SignedFile>,
}

/// Produces the exact bytes to validate and sign for an already populated runtime directory.
pub fn prepare_release(
    root: &Path,
    candidate: &Candidate,
    discord_sha256: &str,
    windows_crt_sha256: Option<&str>,
) -> anyhow::Result<Vec<u8>> {
    ensure!(
        discord_sha256.len() == 64 && discord_sha256.bytes().all(is_lower_hex),
        "pinned Discord package digest must be a lowercase SHA-256"
    );
    ensure!(
        match windows_crt_sha256 {
            Some(digest) =>
                candidate.target == Target::Windows
                    && digest.len() == 64
                    && digest.bytes().all(is_lower_hex),
            None => candidate.target != Target::Windows,
        },
        "Windows release requires a pinned lowercase SHA-256 for {WINDOWS_CRT_FILE}"
    );
    prepare(
        root,
        candidate,
        Purpose::Release,
        Some(discord_sha256),
        windows_crt_sha256,
    )
}

/// Verifies a complete runtime against the public trust compiled into the release tooling.
pub fn verify_release(root: &Path, candidate: &Candidate) -> anyhow::Result<VerifiedRuntime> {
    candidate.validate(Purpose::Release)?;
    verify(
        root,
        candidate.target,
        &candidate.version,
        Some(&candidate.source_commit),
        Purpose::Release,
        &release_trust()?,
        None,
    )
}

/// Verifies the installed version, taking the source commit from the authenticated manifest.
#[cfg(any(windows, target_os = "linux"))]
pub fn verify_installed_release(
    root: &Path,
    target: Target,
    version: &Version,
) -> anyhow::Result<VerifiedRuntime> {
    verify(
        root,
        target,
        version,
        None,
        Purpose::Release,
        &release_trust()?,
        None,
    )
}

/// Verifies the signed payload and its origin record in a persistent AppImage runtime.
#[cfg(target_os = "linux")]
pub fn verify_portable_runtime(root: &Path, version: &Version) -> anyhow::Result<VerifiedRuntime> {
    let origin = alcomd_platform::PortableOrigin::read(root)?;
    let verified = verify(
        root,
        Target::Linux,
        version,
        None,
        Purpose::Release,
        &release_trust()?,
        Some(alcomd_platform::PORTABLE_ORIGIN_FILE),
    )?;
    ensure!(
        format_digest(&verified.manifest_sha256) == origin.payload_manifest_sha256(),
        "portable origin does not belong to the verified runtime"
    );
    Ok(verified)
}

pub struct VerifiedRuntime {
    pub manifest_sha256: [u8; 32],
    file_digests: BTreeMap<String, [u8; 32]>,
}

impl VerifiedRuntime {
    pub fn files(&self) -> impl Iterator<Item = &str> {
        self.file_digests.keys().map(String::as_str)
    }

    /// Returns a component digest from the verified signed manifest.
    pub fn file_sha256(&self, relative: &str) -> Option<&[u8; 32]> {
        self.file_digests.get(relative)
    }

    /// Verifies a regular file copied to a separate entry point against the signed runtime file.
    pub fn verify_copy(&self, relative: &str, path: &Path) -> anyhow::Result<()> {
        let expected = self
            .file_sha256(relative)
            .context("file is not in the verified product payload")?;
        let (file, _) = alcomd_platform::open_plain_file(path)?;
        let size = file.metadata()?.len();
        let mut total = 0;
        check_payload_size(&mut total, size)?;
        ensure!(
            hash_exact(file, size)? == *expected,
            "file copy does not match the signed product payload"
        );
        Ok(())
    }
}

/// Checks the same file and signature rules with a caller-supplied development trust anchor.
pub fn verify_fixture(
    root: &Path,
    candidate: &Candidate,
    key: &VerifyingKey,
) -> anyhow::Result<VerifiedRuntime> {
    candidate.validate(Purpose::Fixture)?;
    verify(
        root,
        candidate.target,
        &candidate.version,
        Some(&candidate.source_commit),
        Purpose::Fixture,
        key,
        None,
    )
}

/// Produces a synthetic manifest. The release verifier rejects this purpose.
pub fn prepare_fixture(root: &Path, candidate: &Candidate) -> anyhow::Result<Vec<u8>> {
    prepare(root, candidate, Purpose::Fixture, None, None)
}

fn prepare(
    root: &Path,
    candidate: &Candidate,
    purpose: Purpose,
    discord_sha256: Option<&str>,
    windows_crt_sha256: Option<&str>,
) -> anyhow::Result<Vec<u8>> {
    candidate.validate(purpose)?;
    let (root, identity) = alcomd_platform::resolve_plain_storage_directory(root)?;
    let product: Product = toml::from_str(PRODUCT)?;
    let paths = observe(&root)?;
    ensure!(
        !paths.contains(MANIFEST_NAME) && !paths.contains(SIGNATURE_NAME),
        "staging manifest or signature already exists"
    );
    let executables = validate_layout(&product, candidate.target, purpose, &paths)?;
    let mut total = 0_u64;
    let mut files = Vec::with_capacity(paths.len());
    for path in paths {
        let (mut file, _) = alcomd_platform::open_plain_file(&root.join(&path))?;
        let size = file.metadata()?.len();
        ensure!(size > 0, "empty product payload file");
        check_payload_size(&mut total, size)?;
        if executables.contains(&path) {
            check_executable_header(
                &mut file,
                candidate.target,
                WINDOWS_SIDECARS.contains(&path.as_str()),
            )?;
        }
        let sha256 = format_digest(&hash_exact(file, size)?);
        ensure!(
            path != DISCORD_PACKAGE || discord_sha256.is_none_or(|expected| sha256 == expected),
            "staged Discord package does not match the pinned digest"
        );
        ensure!(
            path != WINDOWS_CRT_FILE
                || windows_crt_sha256.is_none_or(|expected| sha256 == expected),
            "staged {WINDOWS_CRT_FILE} does not match the pinned digest"
        );
        files.push(SignedFile { path, size, sha256 });
    }
    ensure!(
        alcomd_platform::resolve_plain_storage_directory(&root)?.1 == identity,
        "product payload directory changed during preparation"
    );
    let payload = Payload {
        schema: 1,
        product_id: product.identity.bundle_identifier,
        target: candidate.target,
        version: candidate.version.clone(),
        source_commit: candidate.source_commit.clone(),
        purpose,
        files,
    };
    let mut bytes = serde_json::to_vec_pretty(&payload)?;
    bytes.push(b'\n');
    ensure!(
        bytes.len() as u64 <= MANIFEST_LIMIT,
        "product payload manifest exceeds its size limit"
    );
    Ok(bytes)
}

fn verify(
    root: &Path,
    target: Target,
    version: &Version,
    source_commit: Option<&str>,
    purpose: Purpose,
    key: &VerifyingKey,
    local_record: Option<&str>,
) -> anyhow::Result<VerifiedRuntime> {
    ensure!(
        version.major == 4 && version.build.is_empty(),
        "invalid product version"
    );
    let (root, identity) = alcomd_platform::resolve_plain_storage_directory(root)?;
    let bytes = read_limited(&root.join(MANIFEST_NAME), MANIFEST_LIMIT)?;
    let signature = read_limited(&root.join(SIGNATURE_NAME), 128)?;
    let signature = std::str::from_utf8(&signature)?;
    ensure!(
        signature.bytes().all(is_lower_hex),
        "invalid product payload signature encoding"
    );
    let signature = Signature::from_bytes(&decode_hex(signature)?);
    let mut message = SIGNATURE_DOMAIN.to_vec();
    message.extend_from_slice(&bytes);
    key.verify_strict(&message, &signature)
        .context("product payload signature is invalid")?;

    let payload: Payload = serde_json::from_slice(&bytes)?;
    validate_source_commit(&payload.source_commit, purpose)?;
    let product: Product = toml::from_str(PRODUCT)?;
    ensure!(
        payload.schema == 1
            && payload.product_id == product.identity.bundle_identifier
            && payload.target == target
            && &payload.version == version
            && source_commit.is_none_or(|source_commit| payload.source_commit == source_commit)
            && payload.purpose == purpose,
        "product payload does not match the candidate"
    );
    ensure!(
        payload
            .files
            .iter()
            .all(|file| file.size > 0 && file.sha256.bytes().all(is_lower_hex)),
        "invalid product payload file metadata"
    );

    let mut observed = observe(&root)?;
    if let Some(record) = local_record {
        ensure!(observed.remove(record), "missing local installation record");
    }
    let content_paths = observed
        .iter()
        .filter(|path| ![MANIFEST_NAME, SIGNATURE_NAME].contains(&path.as_str()))
        .cloned()
        .collect();
    let executables = validate_layout(&product, target, purpose, &content_paths)?;
    let file_digests = verify_files(
        &root,
        payload.files,
        observed,
        [MANIFEST_NAME, SIGNATURE_NAME],
        |path, file| {
            if executables.contains(path) {
                check_executable_header(file, target, WINDOWS_SIDECARS.contains(&path))?;
            }
            Ok(())
        },
    )?;
    ensure!(
        alcomd_platform::resolve_plain_storage_directory(&root)?.1 == identity,
        "product payload directory changed during verification"
    );
    Ok(VerifiedRuntime {
        manifest_sha256: Sha256::digest(bytes).into(),
        file_digests,
    })
}

fn observe(root: &Path) -> anyhow::Result<BTreeSet<String>> {
    let mut paths = BTreeSet::new();
    collect_files(root, root, 0, &mut paths, Some(DIRECTORIES))?;
    Ok(paths)
}

fn validate_layout(
    product: &Product,
    target: Target,
    purpose: Purpose,
    paths: &BTreeSet<String>,
) -> anyhow::Result<BTreeSet<String>> {
    let mut executables = BTreeSet::new();
    for &role in RUNTIME_BINARY_ROLES {
        let base = product
            .binaries
            .get(role)
            .context("missing product binary role")?;
        executables.insert(format!("{base}{}", target.executable_suffix()));
    }
    ensure!(
        executables.is_subset(paths)
            && paths.contains(LICENSE)
            && paths.contains(NOTICES)
            && paths.contains(DISCORD_PACKAGE),
        "missing required product component"
    );
    ensure!(
        target != Target::Windows
            || purpose != Purpose::Release
            || paths.contains(WINDOWS_CRT_FILE),
        "missing required {WINDOWS_CRT_FILE} in the Windows release"
    );
    ensure!(paths.len() <= MAX_FILES, "too many product payload files");
    let mut names = BTreeSet::new();
    for path in paths {
        ensure!(
            names.insert(path.to_ascii_lowercase()),
            "duplicate product payload path"
        );
        if target == Target::Windows && WINDOWS_SIDECARS.contains(&path.as_str()) {
            executables.insert(path.clone());
        }
        ensure!(
            executables.contains(path)
                || matches!(path.as_str(), LICENSE | NOTICES | DISCORD_PACKAGE)
                || is_license_path(path),
            "unexpected product payload component"
        );
    }
    Ok(executables)
}

fn is_license_path(path: &str) -> bool {
    let Some(stem) = path
        .strip_prefix("licenses/")
        .and_then(|name| name.strip_suffix(".txt"))
    else {
        return false;
    };
    if stem.is_empty()
        || !stem
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
    {
        return false;
    }
    let upper = stem.to_ascii_uppercase();
    !matches!(upper.as_str(), "NUL" | "CON" | "PRN" | "AUX")
        && !["COM", "LPT"].iter().any(|prefix| {
            upper
                .strip_prefix(prefix)
                .is_some_and(|number| number.len() == 1 && number.as_bytes()[0].is_ascii_digit())
        })
}

fn check_executable_header(file: &mut File, target: Target, dll: bool) -> anyhow::Result<()> {
    #[cfg(unix)]
    if target != Target::Windows {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = file.metadata()?.permissions().mode();
        ensure!(
            mode & 0o100 != 0 && mode & 0o6022 == 0,
            "unsafe or non-executable product file mode"
        );
    }
    let mut header = [0; 64];
    file.read_exact(&mut header)
        .context("truncated product executable header")?;
    let matches = match target {
        Target::Windows if header.starts_with(b"MZ") => {
            let offset = u64::from(u32::from_le_bytes(header[60..64].try_into()?));
            ensure!(
                (64..=1024 * 1024).contains(&offset),
                "invalid product PE header offset"
            );
            file.seek(SeekFrom::Start(offset))?;
            let mut pe = [0; 26];
            file.read_exact(&mut pe)?;
            let characteristics = u16::from_le_bytes(pe[22..24].try_into()?);
            pe.starts_with(b"PE\0\0\x64\x86")
                && pe[24..26] == [0x0b, 0x02]
                && (characteristics & 0x2000 != 0) == dll
        }
        Target::Linux => header.starts_with(b"\x7fELF\x02\x01\x01") && header[18..20] == [62, 0],
        Target::MacOs => header.starts_with(&[0xcf, 0xfa, 0xed, 0xfe, 12, 0, 0, 1]),
        Target::Windows => false,
    };
    ensure!(
        matches,
        "product executable header does not match the target"
    );
    file.rewind()?;
    Ok(())
}

fn is_lower_hex(byte: u8) -> bool {
    byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
}

pub fn format_digest(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut formatted = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(formatted, "{byte:02x}").expect("writing to a String cannot fail");
    }
    formatted
}
