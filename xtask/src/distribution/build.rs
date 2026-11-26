use std::fs;
use std::io::{self, Read};
use std::path::Path;
use std::process::Command;

use alcomd_updater::distribution::{
    Candidate, Target, WINDOWS_CRT_COMPANION_FILE, WINDOWS_CRT_FILE, WindowsCrtDigests,
    format_digest, prepare_release,
};
use anyhow::{Context, ensure};
use clap::ValueEnum;

use super::{ProductConfig, publish_manifest, read_toml, runtime_binaries};

pub(super) struct WindowsCrtInputs<'a> {
    pub primary: &'a Path,
    pub companion: &'a Path,
    pub digests: WindowsCrtDigests<'a>,
}

pub(super) fn runtime(
    root: &Path,
    staging: &Path,
    product: &ProductConfig,
    candidate: &Candidate,
    extension: &Path,
    extension_digest: &str,
    windows_crt: Option<WindowsCrtInputs<'_>>,
) -> anyhow::Result<()> {
    let triple = native_target(candidate.target)?;
    ensure!(
        (candidate.target == Target::Windows) == windows_crt.is_some(),
        "the Windows build requires both app-local VC runtime files and their SHA-256 pins"
    );
    require_signed_source(root, &candidate.source_commit)?;
    let parent = staging
        .parent()
        .context("staging requires a parent directory")?;
    let name = staging
        .file_name()
        .context("staging requires a directory name")?;
    let (parent, parent_identity) = alcomd_platform::resolve_plain_storage_directory(parent)?;
    let destination = parent.join(name);
    match destination.symlink_metadata() {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
        Ok(_) => anyhow::bail!("staging destination already exists"),
    }

    let temporary = tempfile::Builder::new()
        .prefix(".alcomd-dist-")
        .tempdir_in(&parent)?;
    let staged = temporary.path();
    let extensions = staged.join("extensions");
    fs::create_dir(&extensions)?;
    let packaged = extensions.join("discord.alcomdextension");
    copy_component(extension, &packaged)?;
    verify_discord_package(root, &packaged, extension_digest)?;
    if let Some(crt) = &windows_crt {
        crt.digests.validate()?;
        for (name, source, digest) in [
            (WINDOWS_CRT_FILE, crt.primary, crt.digests.primary),
            (
                WINDOWS_CRT_COMPANION_FILE,
                crt.companion,
                crt.digests.companion,
            ),
        ] {
            let path = staged.join(name);
            copy_component(source, &path)?;
            let (file, _) = alcomd_platform::open_plain_file(&path)?;
            let size = file.metadata()?.len();
            ensure!(
                format_digest(&alcomd_updater::hash_exact(file, size)?) == digest,
                "app-local {name} does not match the pinned SHA-256"
            );
        }
    }

    let build_cache = root.join("target");
    alcomd_platform::prepare_storage_directory(&build_cache)?;
    let (build_root, _) = alcomd_platform::resolve_plain_storage_directory(&build_cache)?;
    let binaries = runtime_binaries(product);
    compile(root, &build_root, &triple, &binaries, &product.binaries.gui)?;
    let executables = build_root.join(&triple).join("release");
    for binary in binaries {
        let name = format!("{binary}{}", candidate.target.executable_suffix());
        copy_component(&executables.join(&name), &staged.join(&name))?;
    }
    check_platform_baseline(
        root,
        &build_root,
        candidate.target,
        binaries.map(|binary| staged.join(binary)),
    )?;
    copy_component(&root.join("LICENSE"), &staged.join("LICENSE"))?;
    copy_component(
        &root.join("THIRD_PARTY_NOTICES.md"),
        &staged.join("THIRD_PARTY_NOTICES.md"),
    )?;
    alcomd_platform::sync_directory(&extensions)?;
    let bytes = prepare_release(
        staged,
        candidate,
        extension_digest,
        windows_crt.map(|crt| crt.digests),
    )?;
    publish_manifest(staged, &bytes)?;
    check_source_commit(root, &candidate.source_commit)?;
    ensure!(
        alcomd_platform::resolve_plain_storage_directory(&parent)?.1 == parent_identity,
        "staging parent changed during runtime build"
    );
    alcomd_platform::publish_path_no_replace(staged, &destination)?;
    alcomd_platform::sync_directory(&parent)?;
    println!(
        "Prepared unsigned {} runtime for {triple}",
        candidate.version
    );
    Ok(())
}

pub(super) fn native_target(target: Target) -> anyhow::Result<String> {
    let value = target.to_possible_value().context("unsupported target")?;
    let triple = value.get_name();
    ensure!(
        triple.starts_with(std::env::consts::ARCH)
            && match target {
                Target::Windows => cfg!(target_os = "windows"),
                Target::Linux => cfg!(target_os = "linux"),
                Target::MacOs => cfg!(target_os = "macos"),
            },
        "build the product runtime on the matching operating system and architecture"
    );
    Ok(triple.to_owned())
}

pub(super) fn require_signed_source(root: &Path, expected: &str) -> anyhow::Result<()> {
    check_source_commit(root, expected)?;
    let signature = Command::new("git")
        .args(["verify-commit", expected])
        .current_dir(root)
        .output()?;
    ensure!(
        signature.status.success(),
        "source commit signature could not be verified with the configured Git trust"
    );
    Ok(())
}

pub(super) fn check_source_commit(root: &Path, expected: &str) -> anyhow::Result<()> {
    let head = Command::new("git")
        .args(["rev-parse", "--verify", "HEAD"])
        .current_dir(root)
        .output()?;
    ensure!(
        head.status.success() && std::str::from_utf8(&head.stdout)?.trim_end() == expected,
        "build source does not match the requested commit"
    );
    let status = Command::new("git")
        .args([
            "--no-optional-locks",
            "status",
            "--porcelain=v1",
            "--untracked-files=all",
        ])
        .current_dir(root)
        .output()?;
    ensure!(
        status.status.success() && status.stdout.is_empty(),
        "source build requires a clean checkout; stage under an ignored or external directory"
    );
    Ok(())
}

fn compile(
    root: &Path,
    build_root: &Path,
    target: &str,
    binaries: &[&str],
    gui_binary: &str,
) -> anyhow::Result<()> {
    let node = Command::new("node")
        .arg("--version")
        .current_dir(root)
        .output()?;
    ensure!(
        node.status.success() && node.stdout.starts_with(b"v24."),
        "Node.js 24 is required for the product build"
    );
    let mut cargo = Command::new("cargo");
    cargo.args(["build", "--release", "--locked", "--target", target]);
    for package in binaries.iter().copied().filter(|name| *name != gui_binary) {
        cargo.args(["--package", package]);
    }
    run(cargo, root, build_root, "Rust runtime build")?;

    let npm = if cfg!(windows) { "npm.cmd" } else { "npm" };
    let mut install = Command::new(npm);
    install.args(["ci", "--no-audit", "--no-fund"]);
    run(
        install,
        root,
        build_root,
        "locked frontend dependency install",
    )?;
    let mut gui = Command::new(npm);
    gui.args([
        "run",
        "gui:build",
        "--",
        "--target",
        target,
        "--no-bundle",
        "--ci",
        "--no-sign",
        "--",
        "--locked",
    ]);
    run(gui, root, build_root, "Tauri runtime build")
}

pub(super) fn check_platform_baseline(
    root: &Path,
    build_root: &Path,
    target: Target,
    files: impl IntoIterator<Item = std::path::PathBuf>,
) -> anyhow::Result<()> {
    let script = match target {
        Target::Windows => return Ok(()),
        Target::Linux => "scripts/check-glibc.sh",
        Target::MacOs => "scripts/check-macos-target.sh",
    };
    let mut gate = Command::new("bash");
    gate.arg(script).args(files);
    run(gate, root, build_root, "runtime platform baseline check")
}

pub(super) fn run(
    mut command: Command,
    root: &Path,
    build_root: &Path,
    description: &str,
) -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    command.env("MACOSX_DEPLOYMENT_TARGET", "11.0");
    let status = command
        .current_dir(root)
        // Tauri interpolates OUT_DIR into a permission glob. Use the existing
        // ordinary spelling of the same verified directory for child tools.
        .env(
            "CARGO_TARGET_DIR",
            alcomd_platform::external_filesystem_path(build_root).as_os_str(),
        )
        .status()
        .with_context(|| format!("could not start {description}"))?;
    ensure!(status.success(), "{description} failed ({status})");
    Ok(())
}

pub(super) fn verify_discord_package(root: &Path, path: &Path, digest: &str) -> anyhow::Result<()> {
    let expected: alcomd_extensions::ExtensionManifest = read_toml(
        &root.join("extensions/first-party/alcomd-extension-discord/alcomd-extension.toml"),
    )
    .map_err(|error| anyhow::anyhow!("{error}"))?;
    let package = alcomd_extensions::inspect_extension_package_with_archive_digest(path, digest)?;
    let actual = package.manifest;
    ensure!(
        actual.id == expected.id
            && actual.version == expected.version
            && actual.api == expected.api
            && actual.publisher_name == expected.publisher_name
            && actual.license == expected.license
            && actual.entrypoints == expected.entrypoints
            && actual.ui == expected.ui
            && actual.mcp == expected.mcp
            && actual.cli == expected.cli
            && actual.interfaces == expected.interfaces
            && actual.permissions == expected.permissions
            && actual.local_ipc_targets == expected.local_ipc_targets,
        "the pinned extension does not match the first-party Discord contract"
    );
    Ok(())
}

pub(super) fn copy_component(source: &Path, destination: &Path) -> anyhow::Result<()> {
    let parent = source
        .parent()
        .context("component path requires a parent directory")?;
    alcomd_platform::resolve_plain_storage_directory(parent)?;
    let (input, _) = alcomd_platform::open_plain_file(source)?;
    let metadata = input.metadata()?;
    ensure!(
        metadata.len() <= 2 * 1024 * 1024 * 1024,
        "component exceeds staging size limit"
    );
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)?;
    let size = io::copy(&mut input.take(metadata.len() + 1), &mut output)?;
    ensure!(
        size == metadata.len(),
        "component size changed during staging"
    );
    #[cfg(unix)]
    output.set_permissions(metadata.permissions())?;
    output.sync_all()?;
    Ok(())
}
