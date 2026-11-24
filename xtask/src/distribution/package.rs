use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use alcomd_platform::{publish_path_no_replace, resolve_plain_storage_directory, sync_directory};
use alcomd_updater::distribution::{
    Candidate, MANIFEST_NAME, SIGNATURE_NAME, Target, format_digest, verify_fixture,
};
use anyhow::{Context, ensure};
use clap::ValueEnum;
use ed25519_dalek::SigningKey;

use super::{FIXTURE_KEY, ProductConfig, build::copy_component, publish_file};

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(super) enum Format {
    Inno,
    Appimage,
    Deb,
}

pub(super) fn export(
    root: &Path,
    source: &Path,
    output: &Path,
    product: &ProductConfig,
    candidate: &Candidate,
    format: Format,
) -> anyhow::Result<()> {
    let target = match format {
        Format::Inno => Target::Windows,
        Format::Appimage | Format::Deb => Target::Linux,
    };
    ensure!(
        candidate.target == target,
        "package format does not match target"
    );
    let (source, _) = resolve_plain_storage_directory(source)?;
    let key = SigningKey::from_bytes(&FIXTURE_KEY).verifying_key();
    let verified = verify_fixture(&source, candidate, &key)?;
    let name = output
        .file_name()
        .context("package layout must have a name")?;
    let parent = output
        .parent()
        .context("package layout must have a parent")?;
    let (parent, identity) = resolve_plain_storage_directory(parent)?;
    ensure!(
        !parent.starts_with(&source),
        "package layout must be outside the signed runtime"
    );
    let destination = parent.join(name);
    match destination.symlink_metadata() {
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
        Ok(_) => anyhow::bail!("package layout destination already exists"),
    }
    let temporary = tempfile::Builder::new()
        .prefix(".alcomd-packaging-")
        .tempdir_in(&parent)?;
    let layout = temporary.path();
    let fixture = format!("{}-dist-fixture", product.product.technical_name);
    let version = candidate.version.to_string();
    let linux_root = format!("usr/lib/{fixture}/runtime/{version}");
    let runtime = match format {
        Format::Inno => layout.join("runtime").join(&version),
        Format::Appimage => layout.join("AppDir").join(&linux_root),
        Format::Deb => layout.join("deb-root").join(&linux_root),
    };
    fs::create_dir_all(&runtime)?;
    for relative in verified.files().chain([MANIFEST_NAME, SIGNATURE_NAME]) {
        let path = runtime.join(relative);
        fs::create_dir_all(path.parent().context("runtime file requires a parent")?)?;
        copy_component(&source.join(relative), &path)?;
    }
    ensure!(
        verify_fixture(&runtime, candidate, &key)?.manifest_sha256 == verified.manifest_sha256,
        "package copy does not match verified runtime identity"
    );
    let manifest_digest = format_digest(&verified.manifest_sha256);
    match format {
        Format::Inno => export_inno(layout, &fixture, &version)?,
        Format::Appimage => export_appimage(root, layout, &fixture, &manifest_digest)?,
        Format::Deb => export_deb(layout, &fixture, &candidate.version)?,
    }

    ensure!(
        resolve_plain_storage_directory(&parent)?.1 == identity,
        "package layout parent changed during export"
    );
    sync_directory(layout)?;
    publish_path_no_replace(layout, &destination)?;
    sync_directory(&parent)?;
    println!("Prepared {format:?} packing inputs for fixture runtime {manifest_digest}");
    Ok(())
}

fn export_inno(layout: &Path, fixture: &str, version: &str) -> anyhow::Result<()> {
    let script = format!(
        r#"[Setup]
AppId={fixture}
AppName=ALCOMD Packaging Fixture
AppVersion={version}
DefaultDirName={{localappdata}}\Temp\{fixture}
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
Uninstallable=no
CreateUninstallRegKey=no
CloseApplications=no
RestartApplications=no
UsePreviousAppDir=no
OutputBaseFilename={fixture}-{version}

[Files]
Source: "runtime\{version}\*"; DestDir: "{{app}}\runtime\{version}"; Flags: recursesubdirs

[Code]
function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  Result := '';
  if DirExists(WizardDirValue) or FileExists(WizardDirValue) then
    Result := 'Choose a new directory for the ALCOMD packaging fixture.';
end;
"#
    );
    publish_file(layout, "fixture.iss", script.as_bytes())
}

fn export_appimage(
    root: &Path,
    layout: &Path,
    fixture: &str,
    manifest_digest: &str,
) -> anyhow::Result<()> {
    let app = layout.join("AppDir");
    let script =
        format!("#!/bin/sh\nprintf '%s\\n' 'ALCOMD packaging fixture {manifest_digest}'\n");
    publish_file(&app, "AppRun", script.as_bytes())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(app.join("AppRun"), fs::Permissions::from_mode(0o755))?;
    }
    let desktop = format!(
        "[Desktop Entry]\nType=Application\nName=ALCOMD Packaging Fixture\nExec=AppRun\nIcon={fixture}\nCategories=Development;\nTerminal=true\nNoDisplay=true\n"
    );
    publish_file(&app, &format!("{fixture}.desktop"), desktop.as_bytes())?;
    copy_component(
        &root.join("apps/alcomd-gui/src-tauri/icons/icon.png"),
        &app.join(format!("{fixture}.png")),
    )
}

fn export_deb(layout: &Path, fixture: &str, version: &semver::Version) -> anyhow::Result<()> {
    let base = format!("{}.{}.{}", version.major, version.minor, version.patch);
    let version = if version.pre.is_empty() {
        format!("{base}-1")
    } else {
        format!("{base}~{}-1", version.pre)
    };
    let control = format!(
        "Package: {fixture}\nVersion: {version}\nSection: devel\nPriority: optional\nArchitecture: amd64\nMaintainer: ALCOMD Distribution Testing <noreply@vrclearn.com>\nDescription: ALCOMD packaging fixture\n Contains a signed development runtime for archive verification.\n"
    );
    let metadata = layout.join("deb-root/DEBIAN");
    fs::create_dir(&metadata)?;
    publish_file(&metadata, "control", control.as_bytes())
}
