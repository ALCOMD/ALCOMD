use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use alcomd_platform::{publish_path_no_replace, resolve_plain_storage_directory, sync_directory};
use alcomd_updater::distribution::{
    Candidate, MANIFEST_NAME, SIGNATURE_NAME, Target, VerifiedRuntime, WINDOWS_CRT_FILE,
    format_digest, verify_fixture, verify_release,
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
    for_fixture: bool,
) -> anyhow::Result<()> {
    let target = match format {
        Format::Inno => Target::Windows,
        Format::Appimage | Format::Deb => Target::Linux,
    };
    ensure!(
        candidate.target == target,
        "package format does not match target"
    );
    ensure!(
        for_fixture
            || format == Format::Inno
            || (format == Format::Deb && cfg!(target_os = "linux")),
        "release layout export supports Inno and native Linux DEB packaging"
    );
    let (source, _) = resolve_plain_storage_directory(source)?;
    let verify = |path: &Path| {
        if for_fixture {
            let key = SigningKey::from_bytes(&FIXTURE_KEY).verifying_key();
            verify_fixture(path, candidate, &key)
        } else {
            verify_release(path, candidate)
        }
    };
    let verified = verify(&source)?;
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
    let package = if for_fixture {
        fixture.as_str()
    } else {
        product.product.technical_name.as_str()
    };
    let linux_root = format!("usr/lib/{package}/runtime/{version}");
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
        verify(&runtime)?.manifest_sha256 == verified.manifest_sha256,
        "package copy does not match verified runtime identity"
    );
    let manifest_digest = format_digest(&verified.manifest_sha256);
    if for_fixture {
        match format {
            Format::Inno => export_inno(layout, &fixture, &version)?,
            Format::Appimage => export_appimage(root, layout, &fixture, &manifest_digest)?,
            Format::Deb => export_fixture_deb(layout, &fixture, &candidate.version)?,
        }
    } else if format == Format::Inno {
        export_windows_bin(layout, &runtime, product, &verified)?;
        super::inno::export(layout, product, candidate, &verified)?;
    } else {
        export_product_deb(
            root,
            layout,
            &runtime,
            product,
            &candidate.version,
            &verified,
        )?;
    }
    #[cfg(unix)]
    match format {
        Format::Inno => {}
        Format::Appimage => {
            let tree = layout.join("AppDir");
            set_unix_package_modes(&tree, &runtime, product, Some(&tree.join("AppRun")))?;
        }
        Format::Deb => {
            let tree = layout.join("deb-root");
            let launcher = (!for_fixture).then(|| tree.join("usr/bin").join(&product.binaries.cli));
            set_unix_package_modes(&tree, &runtime, product, launcher.as_deref())?;
        }
    }

    ensure!(
        resolve_plain_storage_directory(&parent)?.1 == identity,
        "package layout parent changed during export"
    );
    sync_directory(layout)?;
    publish_path_no_replace(layout, &destination)?;
    sync_directory(&parent)?;
    let purpose = if for_fixture { "fixture" } else { "release" };
    println!("Prepared {format:?} packing inputs for {purpose} runtime {manifest_digest}");
    Ok(())
}

fn export_windows_bin(
    layout: &Path,
    runtime: &Path,
    product: &ProductConfig,
    verified: &VerifiedRuntime,
) -> anyhow::Result<()> {
    let bin = layout.join("bin");
    fs::create_dir(&bin)?;
    let cli = format!(
        "{}{}",
        product.binaries.cli,
        Target::Windows.executable_suffix()
    );
    for relative in [cli.as_str(), WINDOWS_CRT_FILE] {
        let output = bin.join(relative);
        copy_component(&runtime.join(relative), &output)?;
        verified.verify_copy(relative, &output)?;
    }
    sync_directory(&bin)?;
    Ok(())
}

#[cfg(unix)]
fn set_unix_package_modes(
    directory: &Path,
    runtime: &Path,
    product: &ProductConfig,
    launcher: Option<&Path>,
) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;

    fs::set_permissions(directory, fs::Permissions::from_mode(0o755))?;
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let path = entry.path();
        if kind.is_dir() {
            set_unix_package_modes(&path, runtime, product, launcher)?;
        } else {
            ensure!(kind.is_file(), "unexpected entry in Linux package layout");
            let name = entry.file_name();
            let executable = launcher == Some(path.as_path())
                || (directory == runtime
                    && super::runtime_binaries(product)
                        .iter()
                        .any(|binary| name == *binary));
            let mode = if executable { 0o755 } else { 0o644 };
            fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
        }
    }
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
    let desktop = format!(
        "[Desktop Entry]\nType=Application\nName=ALCOMD Packaging Fixture\nExec=AppRun\nIcon={fixture}\nCategories=Development;\nTerminal=true\nNoDisplay=true\n"
    );
    publish_file(&app, &format!("{fixture}.desktop"), desktop.as_bytes())?;
    copy_component(
        &root.join("apps/alcomd-gui/src-tauri/icons/icon.png"),
        &app.join(format!("{fixture}.png")),
    )
}

fn deb_version(version: &semver::Version) -> String {
    let base = format!("{}.{}.{}", version.major, version.minor, version.patch);
    if version.pre.is_empty() {
        format!("{base}-1")
    } else {
        format!("{base}~{}-1", version.pre)
    }
}

fn export_fixture_deb(
    layout: &Path,
    fixture: &str,
    version: &semver::Version,
) -> anyhow::Result<()> {
    let control = format!(
        "Package: {fixture}\nVersion: {}\nSection: devel\nPriority: optional\nArchitecture: amd64\nMaintainer: ALCOMD Distribution Testing <noreply@vrclearn.com>\nDescription: ALCOMD packaging fixture\n Contains a signed development runtime for archive verification.\n",
        deb_version(version)
    );
    publish_deb_control(layout, &control)
}

fn export_product_deb(
    root: &Path,
    layout: &Path,
    runtime: &Path,
    product: &ProductConfig,
    version: &semver::Version,
    verified: &VerifiedRuntime,
) -> anyhow::Result<()> {
    let tree = layout.join("deb-root");
    let bin = tree.join("usr/bin");
    fs::create_dir_all(&bin)?;
    let cli = bin.join(&product.binaries.cli);
    copy_component(&runtime.join(&product.binaries.cli), &cli)?;
    verified.verify_copy(&product.binaries.cli, &cli)?;

    let package = &product.product.technical_name;
    let desktop_id = &product.identity.bundle_identifier;
    let display = &product.product.display_name;
    let scheme = &product.identity.uri_scheme;
    let gui = &product.binaries.gui;
    let applications = tree.join("usr/share/applications");
    fs::create_dir_all(&applications)?;
    let desktop = format!(
        "[Desktop Entry]\nType=Application\nName={display}\nExec=/usr/lib/{package}/runtime/{version}/{gui} %u\nIcon={desktop_id}\nCategories=Development;\nMimeType=x-scheme-handler/{scheme};\n"
    );
    publish_file(
        &applications,
        &format!("{desktop_id}.desktop"),
        desktop.as_bytes(),
    )?;
    let icons = tree.join("usr/share/icons/hicolor/256x256/apps");
    fs::create_dir_all(&icons)?;
    copy_component(
        &root.join("apps/alcomd-gui/src-tauri/icons/128x128@2x.png"),
        &icons.join(format!("{desktop_id}.png")),
    )?;

    let maintainer = &product.product.publisher_name;
    let homepage = &product.product.homepage_url;
    let control = format!(
        "Package: {package}\nVersion: {}\nSection: utils\nPriority: optional\nArchitecture: amd64\nMaintainer: {maintainer} <noreply@vrclearn.com>\nDepends: libc6 (>= 2.34), libcairo2, libdbus-1-3, libgcc-s1, libgdk-pixbuf-2.0-0, libglib2.0-0, libgtk-3-0, libjavascriptcoregtk-4.1-0, libsoup-3.0-0, libwebkit2gtk-4.1-0\nHomepage: {homepage}\nDescription: {display} project and package management\n Contains the desktop client, Core and command line interface for Unity projects.\n",
        deb_version(version)
    );
    publish_deb_control(layout, &control)
}

fn publish_deb_control(layout: &Path, control: &str) -> anyhow::Result<()> {
    let metadata = layout.join("deb-root/DEBIAN");
    fs::create_dir(&metadata)?;
    publish_file(&metadata, "control", control.as_bytes())
}
