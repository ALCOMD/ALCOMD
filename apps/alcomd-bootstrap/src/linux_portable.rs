use std::ffi::{OsStr, OsString};
use std::fs::{self, DirBuilder, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use alcomd_platform::{
    CORE_EXECUTABLE_NAME, DataConfig, LINUX_BOOTSTRAP_EXECUTABLE_NAME, LINUX_GUI_EXECUTABLE_NAME,
    PORTABLE_ORIGIN_FILE, PortableImage, PortableOrigin, check_linux_core_registration,
    linux_portable_runtime, open_plain_file, publish_path_no_replace,
    resolve_plain_storage_directory, state_database_path, sync_directory,
};
use alcomd_updater::{
    distribution::{
        MANIFEST_NAME, SIGNATURE_NAME, Target, VerifiedRuntime, format_digest,
        verify_installed_release, verify_portable_runtime,
    },
    hash_exact,
};
use anyhow::{Context, ensure};
use uuid::Uuid;

// The frozen release limits a complete desktop asset to 512 MiB.
const IMAGE_LIMIT: u64 = 512 * 1024 * 1024;

/// Opens a product AppImage, installs its verified runtime and starts the desktop.
pub fn start_portable(arguments: &[OsString]) -> anyhow::Result<()> {
    let mount = std::env::var_os("APPDIR").context("AppImage mount is unavailable")?;
    let image_path = std::env::var_os("APPIMAGE").context("AppImage origin is unavailable")?;
    let image = PortableImage::open(Path::new(&image_path))?;
    ensure!(
        image.size() <= IMAGE_LIMIT,
        "AppImage exceeds its size limit"
    );
    let version = env!("CARGO_PKG_VERSION").parse()?;
    let runtime = embedded_runtime(Path::new(&mount))?;
    let verified = verify_installed_release(&runtime, Target::Linux, &version)?;
    let image_digest = format_digest(&hash_exact(image.file().try_clone()?, image.size())?);
    let origin = PortableOrigin::bind(
        &image,
        &image_digest,
        &format_digest(&verified.manifest_sha256),
    )?;

    let data = DataConfig::default();
    let destination = linux_portable_runtime(&data)?;
    check_linux_core_registration(&data, &destination)
        .context("a different installed Core owns this user account")?;
    super::run_command(
        &runtime.join(CORE_EXECUTABLE_NAME),
        &runtime,
        &[OsStr::new("check-installation-data")],
    )?;
    let state = state_database_path(&data)?;
    let data_root = state
        .parent()
        .context("installed data root is unavailable")?;
    let portable_root = destination
        .parent()
        .context("portable runtime needs a parent")?;
    ensure!(
        portable_root.parent() == Some(data_root),
        "portable runtime is outside the data directory"
    );
    if !portable_root.try_exists()? {
        let parent = data_root
            .parent()
            .context("portable data directory needs a parent")?;
        let stage = RuntimeStage::new(parent)?;
        let install = stage.0.join(version.to_string());
        private_directory(&install)?;
        copy_runtime(&runtime, &install, &verified)?;
        write_origin(&install.join(PORTABLE_ORIGIN_FILE), &origin)?;
        verify_portable_runtime(&install, &version)?;
        sync_directory(&install)?;
        sync_directory(&stage.0)?;
        origin.revalidate()?;
        match publish_path_no_replace(&stage.0, portable_root) {
            Ok(()) => {
                sync_directory(data_root)?;
                sync_directory(parent)?;
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
    }

    let installed = verify_portable_runtime(&destination, &version)?;
    ensure!(
        installed.manifest_sha256 == verified.manifest_sha256,
        "this version already has a different portable payload"
    );
    let previous_origin = PortableOrigin::read(&destination)?;
    if previous_origin != origin {
        rebind_origin(portable_root, &destination, &origin)?;
    }
    origin.revalidate()?;
    let gui = destination.join(LINUX_GUI_EXECUTABLE_NAME);
    Err(Command::new(gui)
        .args(arguments)
        .env_remove("ALCOMD_MCP_CLIENT_CREDENTIAL")
        .exec())
    .context("installed portable desktop could not start")
}

fn embedded_runtime(mount: &Path) -> anyhow::Result<PathBuf> {
    let (mount, _) = resolve_plain_storage_directory(mount)?;
    let runtime = mount
        .join("usr/lib")
        .join(CORE_EXECUTABLE_NAME)
        .join("runtime")
        .join(env!("CARGO_PKG_VERSION"));
    ensure!(
        std::env::current_exe()? == runtime.join(LINUX_BOOTSTRAP_EXECUTABLE_NAME),
        "Bootstrap is outside the mounted product runtime"
    );
    Ok(runtime)
}

struct RuntimeStage(PathBuf);

impl RuntimeStage {
    fn new(parent: &Path) -> io::Result<Self> {
        let path = parent.join(format!(".alcomd-portable-{}", Uuid::new_v4()));
        private_directory(&path)?;
        Ok(Self(path))
    }
}

impl Drop for RuntimeStage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn private_directory(path: &Path) -> io::Result<()> {
    DirBuilder::new().mode(0o700).create(path)
}

fn copy_runtime(
    source: &Path,
    destination: &Path,
    verified: &VerifiedRuntime,
) -> anyhow::Result<()> {
    let extensions = destination.join("extensions");
    private_directory(&extensions)?;
    for name in verified.files().chain([MANIFEST_NAME, SIGNATURE_NAME]) {
        let (source, _) = open_plain_file(&source.join(name))?;
        let metadata = source.metadata()?;
        let size = metadata.len();
        let executable = metadata.permissions().mode() & 0o111 != 0;
        let path = destination.join(name);
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(if executable { 0o700 } else { 0o600 })
            .open(path)?;
        let limit = size
            .checked_add(1)
            .context("AppImage component exceeds its size limit")?;
        ensure!(
            io::copy(&mut source.take(limit), &mut output)? == size,
            "AppImage component changed during installation"
        );
        output.sync_all()?;
    }
    sync_directory(&extensions)?;
    Ok(())
}

fn write_origin(path: &Path, origin: &PortableOrigin) -> io::Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(&origin.json_bytes()?)?;
    file.sync_all()
}

fn rebind_origin(parent: &Path, runtime: &Path, origin: &PortableOrigin) -> io::Result<()> {
    let stage = RuntimeStage::new(parent)?;
    let record = stage.0.join(PORTABLE_ORIGIN_FILE);
    write_origin(&record, origin)?;
    origin.revalidate()?;
    fs::rename(record, runtime.join(PORTABLE_ORIGIN_FILE))?;
    sync_directory(runtime)
}
