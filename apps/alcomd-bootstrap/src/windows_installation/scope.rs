use std::path::Path;

use alcomd_platform::{
    WindowsScopeTransitionSource, resolve_plain_storage_directory, windows_installation_runtime,
    windows_process_is_elevated,
};
use alcomd_updater::distribution::{Target, format_digest, verify_installed_release};
use anyhow::{Context, ensure};
use clap::ValueEnum;
use serde::Serialize;

use super::update::{check_machine_runtime, verify_stable_cli};

/// The two mutually exclusive Windows product installation scopes.
#[derive(Clone, Copy, Debug, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum WindowsInstallScope {
    CurrentUser,
    AllUsers,
}

impl WindowsInstallScope {
    fn opposite(self) -> Self {
        match self {
            Self::CurrentUser => Self::AllUsers,
            Self::AllUsers => Self::CurrentUser,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ScopePlan {
    source: ScopePayload,
    destination: ScopePayload,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ScopePayload {
    scope: WindowsInstallScope,
    version: String,
    manifest_sha256: String,
}

/// Identifies the exact installed source and staged destination for a scope conversion.
pub fn check_windows_scope_transition(
    destination: WindowsInstallScope,
    destination_version: &str,
    staged_runtime: &Path,
) -> anyhow::Result<String> {
    let source = destination.opposite();
    let machine = matches!(source, WindowsInstallScope::AllUsers);
    if !machine {
        ensure!(
            !windows_process_is_elevated()?,
            "current-user scope conversion must check the initiating account without elevation"
        );
    }
    let executable = std::env::current_exe()?;
    let _lease = WindowsScopeTransitionSource::acquire(&executable, machine)
        .context("the installed Windows source is unavailable for scope conversion")?;
    let installed = windows_installation_runtime(&executable)?;
    let source_version = env!("CARGO_PKG_VERSION").parse()?;
    let destination_version = destination_version.parse()?;
    let previous = verify_installed_release(&installed, Target::Windows, &source_version)
        .context("the installed Windows scope did not verify")?;
    if machine {
        check_machine_runtime(&installed, &previous)?;
    }
    let root = installed
        .parent()
        .and_then(Path::parent)
        .context("the installed Windows scope has no product root")?;
    verify_stable_cli(root, &previous)?;

    let (staged, _) = resolve_plain_storage_directory(staged_runtime)?;
    ensure!(
        staged != installed,
        "the destination must be staged separately"
    );
    let candidate = verify_installed_release(&staged, Target::Windows, &destination_version)
        .context("the proposed Windows scope payload did not verify")?;
    ensure!(
        !destination_version.cmp_precedence(&source_version).is_lt(),
        "Windows scope conversion cannot install an older product version"
    );
    ensure!(
        destination_version != source_version
            || candidate.manifest_sha256 == previous.manifest_sha256,
        "the two Windows payloads disagree for the same version"
    );
    Ok(serde_json::to_string(&ScopePlan {
        source: ScopePayload {
            scope: source,
            version: source_version.to_string(),
            manifest_sha256: format_digest(&previous.manifest_sha256),
        },
        destination: ScopePayload {
            scope: destination,
            version: destination_version.to_string(),
            manifest_sha256: format_digest(&candidate.manifest_sha256),
        },
    })?)
}
