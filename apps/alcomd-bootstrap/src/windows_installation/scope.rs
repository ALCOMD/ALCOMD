use std::ffi::{OsStr, OsString};
use std::io::{self, Read};
use std::path::Path;

use alcomd_platform::{
    CORE_EXECUTABLE_NAME, DataConfig, WINDOWS_ACTIVE_RUNTIME_RECORD,
    WINDOWS_BOOTSTRAP_EXECUTABLE_NAME, WINDOWS_SCOPE_TRANSITION_RECORD,
    WindowsScopeRegistrationTransfer, WindowsScopeTransitionDestination,
    WindowsScopeTransitionSource, open_owned_installation_record, open_plain_file,
    open_windows_machine_installation_record, resolve_plain_storage_directory,
    start_elevated_windows_program, windows_current_account_sid, windows_machine_installation_root,
    windows_machine_runtime_bootstrap, windows_process_is_elevated,
};
use alcomd_updater::distribution::{
    MANIFEST_NAME, SIGNATURE_NAME, Target, VerifiedRuntime, format_digest, verify_installed_release,
};
use anyhow::{Context, ensure};
use clap::{Args, ValueEnum};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use super::update::{check_machine_runtime, verify_stable_cli};

/// The two mutually exclusive Windows product installation scopes.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ValueEnum)]
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

/// A source-side action in the installer-controlled conversion transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum ScopeSourceAction {
    Inspect,
    Fence,
    Retire,
    Restore,
}

/// A destination-side action in the installer-controlled conversion transaction.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ScopeDestinationAction {
    Prepare,
    Activate,
    Abort,
}

/// The version and identity fields reviewed by the installer before switching scope.
#[derive(Debug, Args)]
pub struct WindowsScopePlanArguments {
    #[arg(long)]
    requester_sid: String,
    #[arg(long, value_enum)]
    destination: WindowsInstallScope,
    #[arg(long)]
    source_version: String,
    #[arg(long)]
    source_manifest_sha256: String,
    #[arg(long)]
    destination_version: String,
    #[arg(long)]
    destination_manifest_sha256: String,
}

impl WindowsScopePlanArguments {
    fn plan(self) -> ScopePlan {
        ScopePlan {
            requester_sid: self.requester_sid,
            source: ScopePayload {
                scope: self.destination.opposite(),
                version: self.source_version,
                manifest_sha256: self.source_manifest_sha256,
            },
            destination: ScopePayload {
                scope: self.destination,
                version: self.destination_version,
                manifest_sha256: self.destination_manifest_sha256,
            },
        }
    }
}

/// Routes an ordinary user's request only to the verified machine Bootstrap.
pub fn request_windows_machine_scope_source(
    action: ScopeSourceAction,
    source_version: &str,
    destination_version: &str,
    staged_runtime: &Path,
    approved_plan: &str,
) -> anyhow::Result<()> {
    ensure!(
        !windows_process_is_elevated()? && action != ScopeSourceAction::Inspect,
        "machine scope maintenance must be requested by the original account"
    );
    let (staged, _) = resolve_plain_storage_directory(staged_runtime)?;
    verify_installed_release(&staged, Target::Windows, &destination_version.parse()?)
        .context("the proposed Windows scope payload did not verify")?;

    let root = windows_machine_installation_root()?;
    let (versions, _) = resolve_plain_storage_directory(&root.join("runtime"))?;
    let version = source_version.parse()?;
    let runtime = versions.join(source_version);
    let verified = verify_installed_release(&runtime, Target::Windows, &version)
        .context("the installed machine scope did not verify")?;
    check_machine_runtime(&runtime, &verified)?;
    let bootstrap = windows_machine_runtime_bootstrap(&runtime)?;
    let requester = windows_current_account_sid()?;
    let action = action
        .to_possible_value()
        .context("the Windows scope source action is unavailable")?;
    let mut arguments: Vec<OsString> = [
        "coordinate-windows-scope-source",
        "--action",
        action.get_name(),
        "--destination",
        "current-user",
        "--requester-sid",
        &requester,
        "--source-version",
        source_version,
        "--destination-version",
        destination_version,
        "--approved-plan",
        approved_plan,
        "--staged-runtime",
    ]
    .into_iter()
    .map(OsString::from)
    .collect();
    arguments.push(staged.into_os_string());
    let arguments: Vec<&OsStr> = arguments.iter().map(OsString::as_os_str).collect();
    start_elevated_windows_program(&bootstrap, &runtime, &arguments)?
        .wait()
        .context("the protected machine scope did not finish the requested action")
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ScopePlan {
    requester_sid: String,
    source: ScopePayload,
    destination: ScopePayload,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ScopePayload {
    scope: WindowsInstallScope,
    version: String,
    manifest_sha256: String,
}

impl ScopePlan {
    fn fingerprint(&self) -> anyhow::Result<(Vec<u8>, String)> {
        let intent = serde_json::to_vec(self)?;
        let digest: [u8; 32] = Sha256::digest(&intent).into();
        Ok((intent, format_digest(&digest)))
    }
}

/// Encodes the exact plan shown by Setup so the source worker can independently recompute it.
pub fn windows_scope_plan_fingerprint(
    arguments: WindowsScopePlanArguments,
) -> anyhow::Result<String> {
    arguments.plan().fingerprint().map(|(_, digest)| digest)
}

/// Checks the installed candidate and publishes it only after exact source retirement.
pub fn coordinate_windows_scope_destination(
    action: ScopeDestinationAction,
    arguments: WindowsScopePlanArguments,
    source_root: Option<&Path>,
) -> anyhow::Result<()> {
    let plan = arguments.plan();
    let machine = plan.destination.scope == WindowsInstallScope::AllUsers;
    ensure!(
        windows_process_is_elevated()? == machine
            && (machine || windows_current_account_sid()? == plan.requester_sid),
        "the Windows conversion destination must use the selected account and elevation"
    );
    let destination =
        WindowsScopeTransitionDestination::acquire(machine, &plan.destination.version)?;
    let runtime = destination.runtime();
    let version = plan.destination.version.parse()?;
    let verified = verify_installed_release(runtime, Target::Windows, &version)?;
    let root = runtime
        .parent()
        .and_then(Path::parent)
        .context("the Windows destination has no installation root")?;
    if machine {
        check_machine_runtime(runtime, &verified)?;
    }
    ensure!(
        plan.destination.manifest_sha256 == format_digest(&verified.manifest_sha256),
        "the Windows destination does not match the accepted plan"
    );
    let (intent, _) = plan.fingerprint()?;
    match action {
        ScopeDestinationAction::Prepare => {
            if !machine {
                // Installed Core refuses all commands before the active selector
                // exists. Use the separate signed payload under the original SID.
                let worker = std::env::current_exe()?;
                let (staged, _) = resolve_plain_storage_directory(
                    worker
                        .parent()
                        .context("the scope worker has no directory")?,
                )?;
                ensure!(
                    staged.parent() == root.parent(),
                    "the scope health probe must use a sibling payload"
                );
                let probe = verify_installed_release(&staged, Target::Windows, &version)?;
                ensure!(
                    probe.manifest_sha256 == verified.manifest_sha256,
                    "the Windows scope health payload differs from the destination"
                );
                check_scope_candidate_data(&staged, &probe, false)?;
            }
            destination.prepare(&intent)?;
        }
        ScopeDestinationAction::Abort => destination.abort(&intent)?,
        ScopeDestinationAction::Activate => {
            let source =
                source_root.context("conversion activation requires the retired source")?;
            verify_retired_source(source, &plan, &intent)?;
            verify_stable_cli(root, &verified)?;
            destination.activate(&intent)?;
        }
    }
    Ok(())
}

fn verify_retired_source(root: &Path, plan: &ScopePlan, expected: &[u8]) -> anyhow::Result<()> {
    let (root, _) = resolve_plain_storage_directory(root)?;
    let machine = plan.source.scope == WindowsInstallScope::AllUsers;
    if machine {
        ensure!(
            root == windows_machine_installation_root()?,
            "wrong machine source root"
        );
    }
    let version = plan.source.version.parse()?;
    let runtime = root.join("runtime").join(&plan.source.version);
    let verified = verify_installed_release(&runtime, Target::Windows, &version)?;
    ensure!(
        plan.source.manifest_sha256 == format_digest(&verified.manifest_sha256),
        "the retired Windows source does not match the accepted plan"
    );
    let marker = root.join(WINDOWS_SCOPE_TRANSITION_RECORD);
    let intent = if machine {
        open_windows_machine_installation_record(&marker)?
    } else {
        open_plain_file(&marker)?.0
    };
    let mut bytes = Vec::new();
    intent
        .take((expected.len() + 1) as u64)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes == expected,
        "the Windows source retirement plan changed"
    );
    match std::fs::symlink_metadata(root.join(WINDOWS_ACTIVE_RUNTIME_RECORD)) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
        Ok(_) => anyhow::bail!("the previous Windows scope is still active"),
    }
}

/// Completes registration in the original account before the old program is removed.
pub fn complete_windows_scope_account(
    requester_sid: &str,
    source_sha: String,
) -> anyhow::Result<()> {
    check_requester(requester_sid)?;
    super::register_installed_core(source_sha, None)
}

fn check_requester(requester_sid: &str) -> anyhow::Result<()> {
    ensure!(
        !windows_process_is_elevated()? && windows_current_account_sid()? == requester_sid,
        "Windows scope conversion requires the original non-elevated account"
    );
    Ok(())
}

/// Runs only the matching Inno uninstaller after the source selector has been retired.
pub fn remove_retired_windows_scope(
    source: WindowsInstallScope,
    requester_sid: &str,
    uninstaller: &Path,
    approved_plan: &str,
) -> anyhow::Result<()> {
    check_requester(requester_sid)?;
    let machine = source == WindowsInstallScope::AllUsers;
    let root = if machine {
        windows_machine_installation_root()?
    } else {
        alcomd_platform::windows_current_user_installation_root()?
    };
    let name = uninstaller
        .file_name()
        .and_then(|name| name.to_str())
        .context("the Windows scope uninstaller name is missing")?;
    ensure!(
        name.len() == 12
            && name.starts_with("unins")
            && name.ends_with(".exe")
            && name.as_bytes()[5..8].iter().all(u8::is_ascii_digit),
        "the Windows scope uninstaller has an unexpected name"
    );
    let expected = root.join(name);
    let (parent, _) = resolve_plain_storage_directory(
        uninstaller
            .parent()
            .context("the Windows uninstaller parent is missing")?,
    )?;
    ensure!(
        parent == root,
        "the Windows uninstaller is outside its source scope"
    );
    let file = if machine {
        open_windows_machine_installation_record(&expected)?
    } else {
        open_owned_installation_record(&expected)?
    };
    drop(file);
    let plan = check_retirement_digest(&root, machine, approved_plan)?;
    ensure!(
        plan.requester_sid == requester_sid && plan.source.scope == source,
        "the retired source belongs to another scope conversion"
    );
    let destination_root = if machine {
        alcomd_platform::windows_current_user_installation_root()?
    } else {
        windows_machine_installation_root()?
    };
    let destination_cli = destination_root
        .join("bin")
        .join(alcomd_platform::WINDOWS_CLI_EXECUTABLE_NAME);
    let destination_bootstrap = alcomd_platform::windows_installation_bootstrap(&destination_cli)?
        .context("the Windows conversion destination is not active")?;
    let destination = destination_bootstrap
        .parent()
        .context("the Windows destination runtime is missing")?;
    let installed = verify_installed_release(
        destination,
        Target::Windows,
        &plan.destination.version.parse()?,
    )?;
    ensure!(
        format_digest(&installed.manifest_sha256) == plan.destination.manifest_sha256
            && alcomd_platform::check_windows_core_registration(
                &DataConfig::default(),
                destination
            )?,
        "the initiating account has not verified the destination registration"
    );
    let argument = format!("/ALCOMDRetireScope={approved_plan}");
    let arguments = [
        OsStr::new(&argument),
        OsStr::new("/VERYSILENT"),
        OsStr::new("/NORESTART"),
    ];
    if machine {
        start_elevated_windows_program(&expected, &root, &arguments)?.wait()?;
    } else {
        super::super::run_command(&expected, &root, &arguments)?;
    }
    match std::fs::symlink_metadata(root.join(WINDOWS_SCOPE_TRANSITION_RECORD)) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
        Ok(_) => anyhow::bail!("the previous Windows scope cleanup did not complete"),
    }
}

/// Verifies an already retired source before Inno cleans its exact installed files.
pub fn verify_retired_windows_scope_uninstall(
    source_sha: String,
    approved_plan: &str,
    machine: bool,
) -> anyhow::Result<()> {
    ensure!(
        windows_process_is_elevated()? == machine,
        "wrong Windows uninstaller elevation"
    );
    let (runtime, verified) = super::verified_runtime(source_sha)?;
    let expected = if machine {
        windows_machine_installation_root()?
    } else {
        alcomd_platform::windows_current_user_installation_root()?
    };
    ensure!(
        runtime.parent().and_then(Path::parent) == Some(expected.as_path()),
        "wrong Windows source root"
    );
    let plan = check_retirement_digest(&expected, machine, approved_plan)?;
    ensure!(
        plan.source.version == env!("CARGO_PKG_VERSION")
            && plan.source.manifest_sha256 == format_digest(&verified.manifest_sha256),
        "the retired program files do not match the accepted source"
    );
    super::verify_uninstall_program_files(&runtime, &verified, machine)
}

fn check_retirement_digest(
    root: &Path,
    machine: bool,
    approved_plan: &str,
) -> anyhow::Result<ScopePlan> {
    let marker = root.join(WINDOWS_SCOPE_TRANSITION_RECORD);
    let file = if machine {
        open_windows_machine_installation_record(&marker)?
    } else {
        open_owned_installation_record(&marker)?
    };
    let mut bytes = Vec::new();
    file.take(8193).read_to_end(&mut bytes)?;
    ensure!(
        !bytes.is_empty() && bytes.len() <= 8192,
        "invalid Windows scope retirement record"
    );
    let digest: [u8; 32] = Sha256::digest(&bytes).into();
    ensure!(
        format_digest(&digest) == approved_plan,
        "the Windows scope retirement proof changed"
    );
    match std::fs::symlink_metadata(root.join(WINDOWS_ACTIVE_RUNTIME_RECORD)) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            Ok(serde_json::from_slice(&bytes)?)
        }
        Err(error) => Err(error.into()),
        Ok(_) => anyhow::bail!("the previous Windows scope is still active"),
    }
}

pub(super) fn rebind_scope_registration(
    destination: &Path,
    verified: &VerifiedRuntime,
) -> anyhow::Result<()> {
    let data = DataConfig::default();
    let Some(transfer) = WindowsScopeRegistrationTransfer::acquire(&data, destination)? else {
        return Ok(());
    };
    let plan: ScopePlan = serde_json::from_slice(transfer.intent())
        .context("the retired Windows scope has an invalid conversion plan")?;
    let source_scope = if transfer.source_is_machine() {
        WindowsInstallScope::AllUsers
    } else {
        WindowsInstallScope::CurrentUser
    };
    let source = transfer.source_runtime();
    ensure!(
        plan.requester_sid == windows_current_account_sid()?
            && plan.source.scope == source_scope
            && plan.destination.scope == source_scope.opposite()
            && source.file_name().and_then(|name| name.to_str())
                == Some(plan.source.version.as_str())
            && plan.destination.version == env!("CARGO_PKG_VERSION")
            && plan.destination.manifest_sha256 == format_digest(&verified.manifest_sha256),
        "the installed Windows scope does not match the conversion plan"
    );
    let source_version = plan.source.version.parse()?;
    let destination_version = plan.destination.version.parse()?;
    let previous = verify_installed_release(source, Target::Windows, &source_version)
        .context("the retired Windows scope no longer matches its signed payload")?;
    ensure!(
        source_version.cmp_precedence(&destination_version).is_le()
            && plan.source.manifest_sha256 == format_digest(&previous.manifest_sha256)
            && (destination_version != source_version
                || verified.manifest_sha256 == previous.manifest_sha256),
        "the retired Windows payload does not belong to the accepted conversion"
    );
    transfer.publish()?;
    Ok(())
}

/// Checks both signed payloads and performs the selected source action for the approved plan.
pub fn coordinate_windows_scope_source(
    action: ScopeSourceAction,
    destination: WindowsInstallScope,
    requester_sid: &str,
    source_version: &str,
    destination_version: &str,
    staged_runtime: &Path,
    approved_plan: Option<&str>,
) -> anyhow::Result<String> {
    let source = destination.opposite();
    let machine = matches!(source, WindowsInstallScope::AllUsers);
    if !machine {
        ensure!(
            !windows_process_is_elevated()? && windows_current_account_sid()? == requester_sid,
            "current-user scope conversion must check the initiating account without elevation"
        );
    }
    ensure!(
        (5..=184).contains(&requester_sid.len())
            && requester_sid.strip_prefix("S-1-").is_some_and(|parts| {
                parts
                    .split('-')
                    .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
            }),
        "the Windows conversion requester SID is invalid"
    );
    let lease = WindowsScopeTransitionSource::acquire(machine, source_version)
        .context("the installed Windows source is unavailable for scope conversion")?;
    let installed = lease.runtime();
    let source_version = source_version.parse()?;
    let destination_version = destination_version.parse()?;
    let previous = verify_installed_release(installed, Target::Windows, &source_version)
        .context("the installed Windows scope did not verify")?;
    if machine {
        ensure!(
            std::fs::canonicalize(std::env::current_exe()?)?
                == installed.join(WINDOWS_BOOTSTRAP_EXECUTABLE_NAME),
            "machine scope maintenance must use the protected installed Bootstrap"
        );
        check_machine_runtime(installed, &previous)?;
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
    let plan = ScopePlan {
        requester_sid: requester_sid.to_owned(),
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
    };
    let (intent, confirmation) = plan.fingerprint()?;
    if action == ScopeSourceAction::Inspect {
        ensure!(
            approved_plan.is_none(),
            "inspection does not accept approval"
        );
    } else {
        ensure!(
            approved_plan == Some(confirmation.as_str()),
            "the Windows conversion plan changed or was not approved"
        );
    }
    match action {
        ScopeSourceAction::Inspect => {}
        ScopeSourceAction::Fence => {
            if !machine && lease.phase(&intent)? == "active" {
                check_scope_candidate_data(&staged, &candidate, true)?;
            }
            lease.fence(&intent)?;
        }
        ScopeSourceAction::Retire => lease.retire(&intent)?,
        ScopeSourceAction::Restore => lease.restore(&intent)?,
    }
    Ok(serde_json::to_string(&serde_json::json!({
        "plan": plan,
        "planSha256": confirmation,
        "sourcePhase": lease.phase(&intent)?,
    }))?)
}

fn check_scope_candidate_data(
    runtime: &Path,
    verified: &VerifiedRuntime,
    machine: bool,
) -> anyhow::Result<()> {
    let open = if machine {
        open_windows_machine_installation_record
    } else {
        open_owned_installation_record
    };
    let _files = verified
        .files()
        .chain([MANIFEST_NAME, SIGNATURE_NAME])
        .map(|name| open(&runtime.join(name)))
        .collect::<io::Result<Vec<_>>>()?;
    super::super::run_command(
        &runtime.join(CORE_EXECUTABLE_NAME),
        runtime,
        &[OsStr::new("check-installation-data")],
    )
    .map(drop)
    .context("the proposed Windows scope could not read the initiating account's data")
}
