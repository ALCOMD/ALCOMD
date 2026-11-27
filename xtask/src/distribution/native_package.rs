use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Read, Seek};
use std::path::{Path, PathBuf};
use std::process::Command;

use alcomd_platform::{
    external_filesystem_path, open_plain_file, publish_path_no_replace,
    resolve_plain_storage_directory, sync_directory,
};
use alcomd_updater::{
    distribution::{Candidate, format_digest},
    hash_exact,
};
use anyhow::{Context, ensure};

use super::{
    ProductConfig, build,
    package::{self, Format, Variant},
    publish_file, webview2,
};

pub(super) struct PinnedFile {
    pub path: PathBuf,
    pub sha256: String,
}

pub(super) struct Inputs<'a> {
    pub runtime: PathBuf,
    pub output: PathBuf,
    pub format: Format,
    pub packager: PinnedFile,
    pub appimage_runtime: Option<PinnedFile>,
    pub webview2: Option<&'a webview2::Inputs>,
}

pub(super) fn assemble(
    root: &Path,
    product: &ProductConfig,
    candidate: &Candidate,
    inputs: &Inputs<'_>,
) -> anyhow::Result<()> {
    build::native_target(candidate.target)?;
    build::require_signed_source(root, &candidate.source_commit)?;
    ensure!(
        (inputs.format == Format::Appimage) == inputs.appimage_runtime.is_some(),
        "the pinned AppImage outer runtime is required for AppImage assembly only"
    );
    let packager = std::path::absolute(&inputs.packager.path)?;
    resolve_plain_storage_directory(
        packager
            .parent()
            .context("native packager needs a parent")?,
    )?;
    verify_pin(&packager, &inputs.packager.sha256).context("native packager")?;
    let (source, _) = resolve_plain_storage_directory(&inputs.runtime)?;
    let name = inputs
        .output
        .file_name()
        .context("asset directory needs a name")?;
    let output_parent = inputs
        .output
        .parent()
        .context("asset directory needs a parent")?;
    let (parent, parent_identity) = resolve_plain_storage_directory(output_parent)?;
    ensure!(
        !parent.starts_with(&source),
        "asset directory must be outside the signed runtime"
    );
    let destination = parent.join(name);
    match destination.symlink_metadata() {
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
        Ok(_) => anyhow::bail!("asset directory already exists"),
    }

    let scratch = tempfile::Builder::new()
        .prefix(".alcomd-native-package-")
        .tempdir_in(&parent)?;
    let layout = scratch.path().join("layout");
    let output = scratch.path().join("output");
    fs::create_dir(&output)?;
    let manifest = package::export(
        root,
        &source,
        &layout,
        product,
        candidate,
        inputs.format,
        Variant::Release(inputs.webview2),
    )?;

    let suffix = match inputs.format {
        Format::Inno => "windows_x86_64_setup.exe",
        Format::Appimage => "linux_x86_64.AppImage",
        Format::Deb => "linux_amd64.deb",
    };
    let name = format!(
        "{}_{}_{suffix}",
        product.product.family_name, candidate.version
    );
    let asset = output.join(&name);
    let mut command = Command::new(external_filesystem_path(&packager).as_ref());
    command.current_dir(external_filesystem_path(&layout).as_ref());
    match inputs.format {
        Format::Inno => {
            let mut out_dir = std::ffi::OsString::from("/O");
            out_dir.push(external_filesystem_path(&output).as_ref());
            command.arg("/Qp").arg(out_dir).arg("alcomd.iss");
        }
        Format::Appimage => {
            let loader = inputs
                .appimage_runtime
                .as_ref()
                .context("missing AppImage outer runtime")?;
            let staged = scratch.path().join("appimage-runtime");
            build::copy_component(&std::path::absolute(&loader.path)?, &staged)?;
            verify_pin(&staged, &loader.sha256).context("AppImage outer runtime")?;
            command
                .env("ARCH", "x86_64")
                .args([
                    "--appimage-extract-and-run",
                    "--no-appstream",
                    "--runtime-file",
                ])
                .arg(staged)
                .arg("AppDir")
                .arg(&asset);
        }
        Format::Deb => {
            command
                .args(["--root-owner-group", "--build", "deb-root"])
                .arg(&asset);
        }
    }
    let status = command.status().context("start native packager")?;
    ensure!(status.success(), "native packager failed with {status}");
    let mut files = fs::read_dir(&output)?;
    ensure!(
        files
            .next()
            .transpose()?
            .is_some_and(|entry| entry.file_name() == name.as_str())
            && files.next().is_none(),
        "native packager did not create exactly the expected asset"
    );
    let (bytes, digest) = measure_asset(&asset, inputs.format)?;
    let receipt = serde_json::json!({
        "sourceCommit": candidate.source_commit,
        "target": candidate.target,
        "version": candidate.version.to_string(),
        "format": inputs.format,
        "asset": { "name": name, "bytes": bytes, "sha256": digest },
        "payloadManifestSha256": format_digest(&manifest),
        "payloadSignature": "verified",
        "platformSigning": "not-requested",
        "packagerName": packager.file_name().and_then(|name| name.to_str()),
        "packagerSha256": inputs.packager.sha256,
        "appimageRuntimeSha256": inputs.appimage_runtime.as_ref().map(|file| &file.sha256),
    });
    let mut receipt = serde_json::to_vec_pretty(&receipt)?;
    receipt.push(b'\n');
    publish_file(&output, "asset.json", &receipt)?;
    build::check_source_commit(root, &candidate.source_commit)?;
    ensure!(
        resolve_plain_storage_directory(&parent)?.1 == parent_identity,
        "asset directory parent changed during packaging"
    );
    publish_path_no_replace(&output, &destination)?;
    sync_directory(&parent)?;
    println!("Assembled {name} ({bytes} bytes, SHA-256 {digest})");
    Ok(())
}

fn verify_pin(path: &Path, expected: &str) -> anyhow::Result<()> {
    ensure!(
        expected.len() == 64
            && expected
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "invalid SHA-256 pin"
    );
    let (file, _) = open_plain_file(path)?;
    let size = file.metadata()?.len();
    ensure!(
        size > 0 && format_digest(&hash_exact(file, size)?) == expected,
        "input does not match its SHA-256 pin"
    );
    Ok(())
}

fn measure_asset(asset: &Path, format: Format) -> anyhow::Result<(u64, String)> {
    let (mut file, _) = open_plain_file(asset)?;
    let mut header = [0; 11];
    file.read_exact(&mut header)
        .context("incomplete native package")?;
    let appropriate = match format {
        Format::Inno => header.starts_with(b"MZ"),
        Format::Appimage => header.starts_with(b"\x7fELF") && &header[8..] == b"AI\x02",
        Format::Deb => header.starts_with(b"!<arch>\n"),
    };
    ensure!(
        appropriate,
        "native package header does not match its format"
    );
    OpenOptions::new().write(true).open(asset)?.sync_all()?;
    let size = file.metadata()?.len();
    file.rewind()?;
    Ok((size, format_digest(&hash_exact(file, size)?)))
}
