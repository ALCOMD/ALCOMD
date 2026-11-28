use std::fs::{self, OpenOptions};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use alcomd_platform::{open_plain_file, sync_directory};
use alcomd_updater::{distribution::format_digest, hash_exact};
use anyhow::{Context, ensure};

pub(super) const BOOTSTRAPPER_NAME: &str = "MicrosoftEdgeWebview2Setup.exe";
pub(super) const STANDALONE_NAME: &str = "MicrosoftEdgeWebView2RuntimeInstallerX64.exe";
const MAX_INSTALLER_SIZE: u64 = 512 * 1024 * 1024;

pub(super) struct PinnedInstaller {
    pub path: PathBuf,
    pub sha256: String,
}

pub(super) struct Inputs {
    pub bootstrapper: PinnedInstaller,
    pub standalone: Option<PinnedInstaller>,
}

pub(super) struct StagedDigests {
    pub bootstrapper: String,
    pub standalone: Option<String>,
}

impl Inputs {
    pub(super) fn stage(&self, layout: &Path) -> anyhow::Result<StagedDigests> {
        ensure!(cfg!(windows), "export WebView2 packaging inputs on Windows");
        let directory = layout.join("webview2");
        fs::create_dir(&directory)?;
        self.bootstrapper
            .stage(&directory.join(BOOTSTRAPPER_NAME))
            .context("WebView2 Evergreen Bootstrapper")?;
        if let Some(standalone) = &self.standalone {
            ensure!(
                standalone.sha256 != self.bootstrapper.sha256,
                "WebView2 Bootstrapper and Standalone installer require distinct artifacts"
            );
            standalone
                .stage(&directory.join(STANDALONE_NAME))
                .context("WebView2 Evergreen Standalone installer")?;
        }
        sync_directory(&directory)?;
        Ok(StagedDigests {
            bootstrapper: self.bootstrapper.sha256.clone(),
            standalone: self.standalone.as_ref().map(|file| file.sha256.clone()),
        })
    }
}

impl PinnedInstaller {
    fn stage(&self, destination: &Path) -> anyhow::Result<()> {
        self.copy_pinned(destination)?;
        #[cfg(windows)]
        alcomd_platform::verify_windows_program_publisher(destination, "Microsoft Corporation")
            .context("expected a valid Microsoft Authenticode signature")?;
        Ok(())
    }

    fn copy_pinned(&self, destination: &Path) -> anyhow::Result<()> {
        ensure!(
            self.sha256.len() == 64
                && self
                    .sha256
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
            "invalid WebView2 installer SHA-256 pin"
        );
        let source = std::path::absolute(&self.path)?;
        let (source, _) = open_plain_file(&source)?;
        let length = source.metadata()?.len();
        ensure!(
            (1..=MAX_INSTALLER_SIZE).contains(&length),
            "WebView2 installer exceeds its size bounds"
        );
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)?;
        ensure!(
            io::copy(&mut source.take(length + 1), &mut output)? == length,
            "WebView2 installer changed while copying"
        );
        output.sync_all()?;
        drop(output);
        let (staged, _) = open_plain_file(destination)?;
        ensure!(
            format_digest(&hash_exact(staged, length)?) == self.sha256,
            "WebView2 installer does not match its SHA-256 pin"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_bad_installer_pin_and_checks_the_copied_bytes() -> anyhow::Result<()> {
        let temporary = tempfile::tempdir()?;
        let source = temporary.path().join("installer.exe");
        fs::write(&source, b"abc")?;
        let mut input = PinnedInstaller {
            path: source,
            sha256: "A".repeat(64),
        };
        let invalid = temporary.path().join("invalid.exe");
        assert!(input.copy_pinned(&invalid).is_err());
        assert!(!invalid.exists());
        input.sha256 = "0".repeat(64);
        assert!(input.copy_pinned(&invalid).is_err());
        input.sha256 = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".into();
        let pinned = temporary.path().join("pinned.exe");
        input.copy_pinned(&pinned)?;
        assert_eq!(fs::read(&pinned)?, b"abc");
        #[cfg(windows)]
        assert!(input.stage(&temporary.path().join("unsigned.exe")).is_err());
        Ok(())
    }
}
