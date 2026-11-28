//! Offline preparation for the existing signed v3 bridge payload contract.

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Context, ensure};

use crate::distribution::{
    Candidate, Target, VerifiedRuntime, check_executable_header, format_digest,
};
use crate::{
    Channel, MANIFEST, MANIFEST_LIMIT, MAX_FILES, PRODUCT, Payload, Product, SignedFile,
    check_payload_size, collect_files, hash_exact,
};

pub const MANIFEST_NAME: &str = MANIFEST;

/// Paths inside the standalone AppImage bridge archive.
pub mod appimage {
    use std::path::Path;

    use anyhow::ensure;
    use serde::{Deserialize, Serialize};

    use crate::Channel;
    use crate::product_update::{UpdateFormat, VerifiedUpdate};

    pub const LAUNCHER: &str = "alcomd-v3-appimage-bridge";
    pub const MIGRATION: &str = "migration";
    pub const RELEASE: &str = "product";
    pub const OFFER: &str = "offer.json";
    pub const HEALTH_REPORT: &str = "health.json";
    pub const HEALTH_SIGNATURE: &str = "health.sig";

    /// Exact release inputs compiled into the separately authenticated launcher.
    #[derive(Debug, Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    pub struct AssetPins {
        pub name: String,
        pub bytes: u64,
        pub sha256: [u8; 32],
        pub product_manifest_sha256: [u8; 32],
        pub migration_manifest_sha256: [u8; 32],
        pub channel: Channel,
    }

    impl AssetPins {
        pub fn from_offer(
            offer: &VerifiedUpdate,
            migration_manifest_sha256: [u8; 32],
        ) -> anyhow::Result<Self> {
            ensure!(
                offer.acceptance().stream.format == UpdateFormat::Appimage,
                "not an AppImage release offer"
            );
            let asset = offer.asset();
            Ok(Self {
                name: asset.name().to_owned(),
                bytes: asset.bytes(),
                sha256: asset.sha256(),
                product_manifest_sha256: offer.payload_manifest_sha256(),
                migration_manifest_sha256,
                channel: offer.acceptance().stream.channel,
            })
        }

        pub fn verify_file(&self, path: &Path) -> anyhow::Result<()> {
            let (file, _) = alcomd_platform::open_plain_file(path)?;
            ensure!(
                crate::hash_exact(file, self.bytes)? == self.sha256,
                "AppImage bytes differ from the verified standalone release"
            );
            Ok(())
        }
    }
}

/// Returns the exact v1 bridge signature message, which is the raw manifest file.
///
/// The caller supplies a complete runtime previously authenticated by
/// `distribution::verify_release` and stages its files alongside the independently built
/// migration executable. The ordinary product manifest and signature are excluded.
pub fn prepare_manifest(
    root: &Path,
    candidate: &Candidate,
    runtime: &VerifiedRuntime,
    channel: Channel,
) -> anyhow::Result<Vec<u8>> {
    ensure!(
        candidate.version.major == 4
            && candidate.version.build.is_empty()
            && (channel == Channel::Beta || candidate.version.pre.is_empty()),
        "bridge version does not match the v4 release series or selected channel"
    );
    let product: Product = toml::from_str(PRODUCT)?;
    let migration = product
        .binaries
        .get("migration_v3")
        .context("migration binary is missing from product identity")?;
    let migration = format!("{migration}{}", candidate.target.executable_suffix());
    let expected: BTreeSet<&str> = runtime.files().chain([migration.as_str()]).collect();
    let (root, identity) = alcomd_platform::resolve_plain_storage_directory(root)?;
    let mut observed = BTreeSet::new();
    collect_files(&root, &root, 0, &mut observed, None)?;
    ensure!(
        observed.len() <= MAX_FILES
            && observed
                .iter()
                .map(String::as_str)
                .eq(expected.iter().copied()),
        "bridge staging must contain the signed product files and only the migration reader"
    );

    let mut total = 0;
    let mut files = Vec::with_capacity(observed.len());
    for path in observed {
        let (mut file, _) = alcomd_platform::open_plain_file(&root.join(&path))?;
        let size = file.metadata()?.len();
        ensure!(size > 0, "empty bridge payload file");
        check_payload_size(&mut total, size)?;
        if path == migration {
            check_executable_header(&mut file, candidate.target, false)?;
        }
        let digest = hash_exact(file, size)?;
        if path != migration {
            ensure!(
                runtime.file_sha256(&path) == Some(&digest),
                "bridge file does not match the signed product runtime"
            );
        }
        files.push(SignedFile {
            path,
            size,
            sha256: format_digest(&digest),
        });
    }
    ensure!(
        alcomd_platform::resolve_plain_storage_directory(&root)?.1 == identity,
        "bridge staging changed during manifest preparation"
    );
    let target = match candidate.target {
        Target::Windows => "windows-x86_64",
        Target::Linux => "linux-x86_64",
        Target::MacOs => "macos-aarch64",
    };
    let payload = Payload {
        schema: 1,
        product_id: product.identity.bundle_identifier,
        version: candidate.version.clone(),
        target: target.to_owned(),
        channel,
        files,
    };
    let mut bytes = serde_json::to_vec_pretty(&payload)?;
    bytes.push(b'\n');
    ensure!(
        bytes.len() as u64 <= MANIFEST_LIMIT,
        "bridge payload manifest exceeds its size limit"
    );
    Ok(bytes)
}
