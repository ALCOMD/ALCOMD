use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use alcomd_platform::{publish_path_no_replace, resolve_plain_storage_directory, sync_directory};
use alcomd_updater::bridge::appimage::{
    AssetPins, HEALTH_REPORT, HEALTH_SIGNATURE, LAUNCHER, MIGRATION, OFFER, RELEASE,
};
use alcomd_updater::distribution::{
    Candidate, MANIFEST_NAME, SIGNATURE_NAME, Target, verify_appimage_bridge_release,
    verify_release,
};
use alcomd_updater::product_update::verify_v3_appimage_offer;
use alcomd_updater::{
    Channel, VerifiedPayload, read_limited, verify_health_report, verify_staged_payload,
};
use anyhow::{Context, ensure};

use super::{ProductConfig, bridge, build};

pub(super) struct Inputs {
    pub runtime: PathBuf,
    pub migration: PathBuf,
    pub image: PathBuf,
    pub offer: PathBuf,
    pub health: PathBuf,
}

pub(super) fn package(
    root: &Path,
    output: &Path,
    product: &ProductConfig,
    candidate: &Candidate,
    channel: Channel,
    inputs: &Inputs,
) -> anyhow::Result<()> {
    ensure!(
        candidate.target == Target::Linux,
        "the standalone AppImage bridge requires the Linux target"
    );
    let triple = build::native_target(candidate.target)?;
    build::require_signed_source(root, &candidate.source_commit)?;
    let (runtime, _) = resolve_plain_storage_directory(&std::path::absolute(&inputs.runtime)?)?;
    let (migration, _) = resolve_plain_storage_directory(&std::path::absolute(&inputs.migration)?)?;
    let (health, _) = resolve_plain_storage_directory(&std::path::absolute(&inputs.health)?)?;
    let (parent, identity) = resolve_plain_storage_directory(
        output
            .parent()
            .context("missing archive output directory")?,
    )?;
    let destination = parent.join(output.file_name().context("missing bridge archive name")?);
    ensure!(
        output
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".tar.gz")),
        "the standalone bridge output must be a .tar.gz archive"
    );
    ensure!(
        matches!(fs::symlink_metadata(&destination), Err(error) if error.kind() == std::io::ErrorKind::NotFound)
            && ![&runtime, &migration, &health]
                .iter()
                .any(|path| parent.starts_with(path)),
        "the bridge archive must be new and outside the signed inputs"
    );

    let verified = verify_staged_payload(&migration, channel)?;
    let release = verify_release(&runtime, candidate)?;
    release.verify_bridge_pair(&verified)?;
    let offer_path = std::path::absolute(&inputs.offer)?;
    let image_path = std::path::absolute(&inputs.image)?;
    let offer = verify_v3_appimage_offer(&offer_path, channel)?;
    ensure!(
        offer.version() == &candidate.version,
        "the AppImage offer differs from the signed build version"
    );
    verify_appimage_bridge_release(&runtime, offer.payload_manifest_sha256(), &verified)?;
    offer.asset().verify_file(&image_path)?;
    check_health(&health, &verified)?;

    let work = tempfile::tempdir_in(&parent)?;
    let bundle = work.path().join("bundle");
    fs::create_dir(&bundle)?;
    let copied_product = bundle.join(RELEASE);
    let copied_migration = bundle.join(MIGRATION);
    fs::create_dir(&copied_product)?;
    fs::create_dir(&copied_migration)?;
    bridge::copy_runtime(&migration, &copied_migration, &release)?;
    for name in [
        product.binaries.migration_v3.as_str(),
        alcomd_updater::bridge::MANIFEST_NAME,
        "bridge-payload.sig",
    ] {
        build::copy_component(&migration.join(name), &copied_migration.join(name))?;
    }
    for name in [MANIFEST_NAME, SIGNATURE_NAME] {
        build::copy_component(&runtime.join(name), &copied_product.join(name))?;
    }
    for name in [HEALTH_REPORT, HEALTH_SIGNATURE] {
        build::copy_component(&health.join(name), &bundle.join(name))?;
    }
    build::copy_component(&offer_path, &bundle.join(OFFER))?;
    let copied_image = bundle.join(offer.asset().name());
    build::copy_component(&image_path, &copied_image)?;
    #[cfg(target_os = "linux")]
    alcomd_platform::PortableImage::open(&copied_image)
        .context("the AppImage bridge asset must be a protected executable")?;
    let copied_offer = verify_v3_appimage_offer(&bundle.join(OFFER), channel)?;
    let copied_bridge = verify_staged_payload(&copied_migration, channel)?;
    ensure!(
        copied_offer.acceptance() == offer.acceptance()
            && copied_bridge.manifest_sha256() == verified.manifest_sha256(),
        "signed bridge input changed while assembling the archive"
    );
    verify_appimage_bridge_release(
        &copied_product,
        copied_offer.payload_manifest_sha256(),
        &copied_bridge,
    )?;
    copied_offer.asset().verify_file(&copied_image)?;
    check_health(&bundle, &copied_bridge)?;

    let build_cache = root.join("target");
    alcomd_platform::prepare_storage_directory(&build_cache)?;
    let (build_root, _) = resolve_plain_storage_directory(&build_cache)?;
    let mut cargo = Command::new("cargo");
    cargo.args([
        "build",
        "--locked",
        "--release",
        "--target",
        &triple,
        "--package",
        &product.binaries.bootstrap,
        "--features",
        "appimage-bridge",
        "--bin",
        LAUNCHER,
    ]);
    let pins = AssetPins::from_offer(&copied_offer, copied_bridge.manifest_sha256())?;
    cargo.env("ALCOMD_APPIMAGE_BRIDGE_PINS", serde_json::to_string(&pins)?);
    build::run(
        cargo,
        root,
        &build_root,
        "Linux AppImage bridge launcher build",
    )?;
    let launcher = bundle.join(LAUNCHER);
    build::copy_component(
        &build_root.join(&triple).join("release").join(LAUNCHER),
        &launcher,
    )?;
    build::check_platform_baseline(root, &build_root, candidate.target, [launcher])?;

    let archive = work.path().join("bridge.tar.gz");
    let entries = [
        LAUNCHER,
        MIGRATION,
        RELEASE,
        OFFER,
        HEALTH_REPORT,
        HEALTH_SIGNATURE,
        offer.asset().name(),
    ];
    let status = Command::new("tar")
        .env_remove("TAR_OPTIONS")
        .env_remove("GZIP")
        .args([
            "--format=ustar",
            "--sort=name",
            "--numeric-owner",
            "--owner=0",
            "--group=0",
            "--mtime=@0",
            "-czf",
        ])
        .arg(&archive)
        .arg("-C")
        .arg(&bundle)
        .arg("--")
        .args(entries.into_iter().map(OsString::from))
        .status()
        .context("could not start Linux bridge archive creation")?;
    ensure!(status.success(), "Linux bridge archive creation failed");
    fs::File::open(&archive)?.sync_all()?;
    build::check_source_commit(root, &candidate.source_commit)?;
    ensure!(
        resolve_plain_storage_directory(&parent)?.1 == identity,
        "bridge archive output directory changed"
    );
    publish_path_no_replace(&archive, &destination)?;
    sync_directory(&parent)?;
    println!(
        "Prepared unsigned standalone AppImage bridge archive for {}",
        candidate.version
    );
    Ok(())
}

fn check_health(root: &Path, bridge: &VerifiedPayload) -> anyhow::Result<()> {
    let bytes = read_limited(&root.join(HEALTH_REPORT), 64 * 1024)?;
    let signature = read_limited(&root.join(HEALTH_SIGNATURE), 130)?;
    verify_health_report(&bytes, &signature)?;
    let receipt: serde_json::Value = serde_json::from_slice(&bytes)?;
    let evidence: [u8; 32] = serde_json::from_value(receipt["testEvidenceSha256"].clone())?;
    ensure!(
        receipt.as_object().is_some_and(|fields| fields.len() == 10)
            && receipt["schema"] == 1
            && receipt["productVersion"] == bridge.version().to_string()
            && receipt["installScope"] == "appimage"
            && receipt["payloadManifestSha256"] == serde_json::to_value(bridge.manifest_sha256())?
            && evidence != [0; 32]
            && ["coreRpc", "cli", "gui", "mcp", "firstPartyExtensions"]
                .iter()
                .all(|name| receipt[*name] == true),
        "the signed health proof is incomplete or for a different AppImage bridge"
    );
    Ok(())
}
