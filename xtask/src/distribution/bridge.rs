use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::process::Command;

use alcomd_platform::{publish_path_no_replace, resolve_plain_storage_directory, sync_directory};
use alcomd_updater::Channel;
use alcomd_updater::bridge::{MANIFEST_NAME, prepare_manifest};
use alcomd_updater::distribution::{Candidate, VerifiedRuntime, format_digest, verify_release};
use anyhow::{Context, ensure};

use super::{ProductConfig, build, publish_file};

pub(super) fn export(
    root: &Path,
    source: &Path,
    output: &Path,
    product: &ProductConfig,
    candidate: &Candidate,
    channel: Channel,
) -> anyhow::Result<()> {
    ensure!(
        channel == Channel::Beta || candidate.version.pre.is_empty(),
        "stable bridge staging requires a stable product version"
    );
    let triple = build::native_target(candidate.target)?;
    let (source, source_identity) = resolve_plain_storage_directory(source)?;
    let verified = verify_release(&source, candidate)?;
    let name = output.file_name().context("bridge staging needs a name")?;
    let parent = output
        .parent()
        .context("bridge staging needs a parent directory")?;
    let (parent, identity) = resolve_plain_storage_directory(parent)?;
    ensure!(
        !parent.starts_with(&source)
            && !parent
                .file_name()
                .is_some_and(|name| name.eq_ignore_ascii_case("runtime")),
        "bridge staging needs a separate directory outside an installed runtime layout"
    );
    let destination = parent.join(name);
    match destination.symlink_metadata() {
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
        Ok(_) => anyhow::bail!("bridge staging destination already exists"),
    }

    build::require_signed_source(root, &candidate.source_commit)?;
    let build_cache = root.join("target");
    alcomd_platform::prepare_storage_directory(&build_cache)?;
    let (build_root, _) = resolve_plain_storage_directory(&build_cache)?;
    let migration = &product.binaries.migration_v3;
    let migration_manifest = root
        .join("migrations/v3/app")
        .join(migration)
        .join("Cargo.toml");
    let mut cargo = Command::new("cargo");
    cargo
        .args(["build", "--release", "--locked", "--target", &triple])
        .arg("--manifest-path")
        .arg(migration_manifest)
        .args(["--package", migration, "--bin", migration]);
    build::run(
        cargo,
        root,
        &build_root,
        "isolated v3 migration reader build",
    )?;
    let executable = format!("{migration}{}", candidate.target.executable_suffix());
    let reader = build_root.join(&triple).join("release").join(&executable);

    let temporary = tempfile::Builder::new()
        .prefix(".alcomd-bridge-")
        .tempdir_in(&parent)?;
    let staging = temporary.path();
    copy_runtime(&source, staging, &verified)?;
    let staged_reader = staging.join(&executable);
    build::copy_component(&reader, &staged_reader)?;
    build::check_platform_baseline(root, &build_root, candidate.target, [staged_reader])?;
    let bytes = prepare_manifest(staging, candidate, &verified, channel)?;
    publish_file(staging, MANIFEST_NAME, &bytes)?;
    build::check_source_commit(root, &candidate.source_commit)?;
    ensure!(
        resolve_plain_storage_directory(&source)?.1 == source_identity
            && resolve_plain_storage_directory(&parent)?.1 == identity,
        "bridge staging or product source directory changed"
    );
    publish_path_no_replace(staging, &destination)?;
    sync_directory(&parent)?;
    println!(
        "Prepared unsigned {MANIFEST_NAME} for {} from product manifest {}",
        candidate.version,
        format_digest(&verified.manifest_sha256)
    );
    Ok(())
}

pub(super) fn copy_runtime(
    source: &Path,
    staging: &Path,
    runtime: &VerifiedRuntime,
) -> anyhow::Result<()> {
    for relative in runtime.files() {
        let path = staging.join(relative);
        let parent = path.parent().context("bridge file needs a directory")?;
        fs::create_dir_all(parent)?;
        build::copy_component(&source.join(relative), &path)?;
        sync_directory(parent)?;
    }
    Ok(())
}
