use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use alcomd_platform::{
    open_plain_file, publish_path_no_replace, resolve_plain_storage_directory, sync_directory,
};
use alcomd_updater::distribution::{
    Candidate, MANIFEST_NAME, SIGNATURE_NAME, Target, format_digest, verify_release,
};
use alcomd_updater::{
    Channel, VerifiedPayload, hash_exact, read_limited, verify_health_report, verify_staged_payload,
};
use anyhow::{Context, ensure};

use super::{ProductConfig, build};

const WINDOWS_CAP: u64 = 256 * 1024 * 1024;
const BRIDGE_BIN: &str = "alcomd-v3-windows-bridge";
const BRIDGE_MANIFEST: &str = "bridge-payload.json";
const BRIDGE_SIGNATURE: &str = "bridge-payload.sig";

pub(super) struct Inputs {
    pub runtime: PathBuf,
    pub migration: PathBuf,
    pub installer: PathBuf,
    pub installer_sha256: String,
    pub current_user_health: PathBuf,
    pub all_users_health: PathBuf,
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
        candidate.target == Target::Windows,
        "the v3 standalone bridge is Windows-only"
    );
    let triple = build::native_target(candidate.target)?;
    build::require_signed_source(root, &candidate.source_commit)?;
    let (runtime, _) = resolve_local_dir(&inputs.runtime)?;
    let (migration, _) = resolve_local_dir(&inputs.migration)?;
    let release = verify_release(&runtime, candidate)?;
    let verified = verify_staged_payload(&migration, channel)?;
    ensure!(
        verified.version() == &candidate.version,
        "bridge version differs from release"
    );
    release.verify_bridge_pair(&verified)?;

    let name = output
        .file_name()
        .context("Windows bridge output has no name")?;
    let output_parent = output
        .parent()
        .context("Windows bridge output has no parent")?;
    let (parent, identity) = resolve_plain_storage_directory(output_parent)?;
    let destination = parent.join(name);
    ensure!(
        !destination.try_exists()?
            && !parent.starts_with(&runtime)
            && !parent.starts_with(&migration),
        "Windows bridge output must be new and separate from the signed inputs"
    );
    let work = tempfile::tempdir_in(&parent)?;
    let assets = work.path().join("verified-inputs");
    fs::create_dir(&assets)?;
    let mut components = String::new();
    let mut verified_size = 0;
    embed(
        &assets,
        "installer.exe",
        &inputs.installer,
        &inputs.installer_sha256,
        &mut components,
        &mut verified_size,
    )?;
    ensure!(
        read_source_magic(&assets.join("installer.exe"))? == *b"MZ",
        "reviewed Windows product installer is not a Windows executable"
    );
    let reader = format!("{}.exe", product.binaries.migration_v3);
    let mut program = String::new();
    for relative in release
        .files()
        .chain([reader.as_str(), BRIDGE_MANIFEST, BRIDGE_SIGNATURE])
    {
        let source = migration.join(relative);
        let digest = if let Some(digest) = release.file_sha256(relative) {
            let digest = format_digest(digest);
            writeln!(
                program,
                "ProgramFile {{ path: {relative:?}, sha256: {digest:?} }},"
            )?;
            digest
        } else {
            match relative {
                BRIDGE_MANIFEST => format_digest(&verified.manifest_sha256()),
                BRIDGE_SIGNATURE => digest_file(&source)?.0,
                _ => format_digest(
                    &verified
                        .file_sha256(relative)
                        .context("missing bridge file digest")?,
                ),
            }
        };
        embed(
            &assets,
            &format!("migration/{relative}"),
            &source,
            &digest,
            &mut components,
            &mut verified_size,
        )?;
    }
    let embedded_bridge = verify_staged_payload(&assets.join("migration"), channel)?;
    release.verify_bridge_pair(&embedded_bridge)?;
    ensure!(
        embedded_bridge.manifest_sha256() == verified.manifest_sha256(),
        "embedded migration manifest changed"
    );
    for relative in [MANIFEST_NAME, SIGNATURE_NAME] {
        let digest = if relative == MANIFEST_NAME {
            format_digest(&release.manifest_sha256)
        } else {
            digest_file(&runtime.join(relative))?.0
        };
        writeln!(
            program,
            "ProgramFile {{ path: {relative:?}, sha256: {digest:?} }},"
        )?;
    }
    for (scope, health) in [
        ("current-user", &inputs.current_user_health),
        ("all-users", &inputs.all_users_health),
    ] {
        stage_health(
            &assets,
            scope,
            health,
            &verified,
            &mut components,
            &mut verified_size,
        )?;
    }
    let discord = "extensions/discord.alcomdextension";
    let discord_digest = format_digest(
        release
            .file_sha256(discord)
            .context("missing signed Discord package")?,
    );
    let discord = alcomd_extensions::inspect_extension_package_with_archive_digest(
        &assets.join("migration").join(discord),
        &discord_digest,
    )?;
    let embed_source = work.path().join("windows-bridge-inputs.rs");
    fs::write(
        &embed_source,
        format!(
            "Bundle {{ version: {:?}, channel: {:?}, install_directory: {:?}, source_commit: {:?}, product_manifest: {:?}, bridge_manifest: {:?}, bootstrap: {:?}, daemon: {:?}, migration_reader: {:?}, discord_sha256: {:?}, files: &[{}], runtime: &[{}] }}",
            candidate.version.to_string(),
            match channel {
                Channel::Stable => "stable",
                Channel::Beta => "beta",
            },
            product.identity.install_directory,
            candidate.source_commit,
            format_digest(&release.manifest_sha256),
            format_digest(&verified.manifest_sha256()),
            format!("{}.exe", product.binaries.bootstrap),
            format!("{}.exe", product.binaries.daemon),
            reader,
            format_digest(&discord.package_digest),
            components,
            program,
        ),
    )?;

    let build_cache = root.join("target");
    alcomd_platform::prepare_storage_directory(&build_cache)?;
    let (build_root, _) = resolve_plain_storage_directory(&build_cache)?;
    let migration_manifest = root
        .join("migrations/v3/app")
        .join(&product.binaries.migration_v3)
        .join("Cargo.toml");
    let mut cargo = Command::new("cargo");
    cargo
        .args(["build", "--release", "--locked", "--target", &triple])
        .arg("--manifest-path")
        .arg(migration_manifest)
        .args(["--features", "windows-bridge", "--bin", BRIDGE_BIN])
        .env("ALCOMD_WINDOWS_BRIDGE_EMBED", &embed_source);
    build::run(
        cargo,
        root,
        &build_root,
        "standalone Windows v3 bridge build",
    )?;
    build::check_source_commit(root, &candidate.source_commit)?;
    let executable = build_root
        .join(&triple)
        .join("release")
        .join(format!("{BRIDGE_BIN}.exe"));
    let layout = work.path().join("output");
    fs::create_dir(&layout)?;
    let name = format!("ALCOMD3_{}_windows_x86_64_setup.exe", candidate.version);
    let asset = layout.join(&name);
    build::copy_component(&executable, &asset)?;
    let (digest, length) = digest_file(&asset)?;
    ensure!(
        length <= WINDOWS_CAP,
        "standalone Windows bridge exceeds the frozen updater limit"
    );
    sync_directory(&layout)?;
    ensure!(
        resolve_plain_storage_directory(&parent)?.1 == identity,
        "Windows bridge output parent changed"
    );
    publish_path_no_replace(&layout, &destination)?;
    sync_directory(&parent)?;
    println!("Prepared unsigned frozen-updater asset {name} ({length} bytes, SHA-256 {digest})");
    Ok(())
}

fn resolve_local_dir(path: &Path) -> anyhow::Result<(PathBuf, Vec<u8>)> {
    Ok(resolve_plain_storage_directory(&std::path::absolute(
        path,
    )?)?)
}

fn stage_health(
    assets: &Path,
    scope: &str,
    source: &Path,
    verified: &VerifiedPayload,
    components: &mut String,
    total: &mut u64,
) -> anyhow::Result<()> {
    let (health, _) = resolve_local_dir(source)?;
    for extension in ["json", "sig"] {
        let path = health.join(format!("health.{extension}"));
        let digest = digest_file(&path)?.0;
        embed(
            assets,
            &format!("health/{scope}.{extension}"),
            &path,
            &digest,
            components,
            total,
        )?;
    }
    let report = read_limited(&assets.join(format!("health/{scope}.json")), 64 * 1024)?;
    let signature = read_limited(&assets.join(format!("health/{scope}.sig")), 130)?;
    verify_health_report(&report, &signature)?;
    let receipt: serde_json::Value = serde_json::from_slice(&report)?;
    let evidence: [u8; 32] = serde_json::from_value(receipt["testEvidenceSha256"].clone())?;
    ensure!(
        receipt.as_object().is_some_and(|fields| fields.len() == 10)
            && receipt["schema"] == 1
            && receipt["productVersion"] == verified.version().to_string()
            && receipt["installScope"] == format!("windows-{scope}")
            && receipt["payloadManifestSha256"]
                == serde_json::to_value(verified.manifest_sha256())?
            && evidence != [0; 32]
            && ["coreRpc", "cli", "gui", "mcp", "firstPartyExtensions"]
                .iter()
                .all(|flag| receipt[*flag] == true),
        "platform health receipt does not cover the exact Windows bridge and scope"
    );
    Ok(())
}

fn embed(
    assets: &Path,
    relative: &str,
    source: &Path,
    expected: &str,
    components: &mut String,
    total: &mut u64,
) -> anyhow::Result<()> {
    let source = std::path::absolute(source)?;
    let destination = assets.join(relative);
    fs::create_dir_all(
        destination
            .parent()
            .context("embedded component has no parent")?,
    )?;
    build::copy_component(&source, &destination)?;
    let (digest, size) = digest_file(&destination)?;
    ensure!(digest == expected, "embedded Windows bridge input changed");
    *total = total
        .checked_add(size)
        .context("Windows bridge input size overflow")?;
    ensure!(
        *total < WINDOWS_CAP,
        "Windows bridge inputs exceed the frozen updater limit"
    );
    let destination = destination
        .to_str()
        .context("embedded Windows bridge path is not Unicode")?;
    writeln!(
        components,
        "Component {{ path: {relative:?}, sha256: {digest:?}, bytes: include_bytes!({destination:?}) }},"
    )?;
    Ok(())
}

fn digest_file(path: &Path) -> anyhow::Result<(String, u64)> {
    let (file, _) = open_plain_file(path)?;
    let length = file.metadata()?.len();
    ensure!(
        length > 0 && length <= WINDOWS_CAP,
        "Windows bridge component exceeds its bound"
    );
    Ok((format_digest(&hash_exact(file, length)?), length))
}

fn read_source_magic(path: &Path) -> anyhow::Result<[u8; 2]> {
    use std::io::Read as _;
    let (mut file, _) = open_plain_file(path)?;
    let mut magic = [0; 2];
    file.read_exact(&mut magic)?;
    Ok(magic)
}
