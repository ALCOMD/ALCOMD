use std::ffi::{OsStr, OsString};
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Read};
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::{Duration, Instant};

use alcomd_platform::{
    DaemonInstance, DataConfig, IpcConfig, WINDOWS_ACTIVE_RUNTIME_RECORD,
    approve_windows_empty_bridge_profile, core_installation_registration, external_filesystem_path,
    hold_windows_installation, local_app_data_directory, open_owned_installation_record,
    open_windows_machine_installation_record, resolve_plain_storage_directory,
    state_database_location, windows_current_account_sid, windows_process_is_elevated,
};
use anyhow::{Context, ensure};
use serde::{Deserialize, Serialize};

mod assets;
mod legacy;
mod native;

use assets::Bundle;

const PRODUCT_INSTALLER: &str = "installer.exe";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const WAIT_FOR_V3: Duration = Duration::from_secs(30);
const MAX_RECORD: u64 = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Scope {
    CurrentUser,
    AllUsers,
}

impl Scope {
    fn from_frozen_args(mut arguments: impl Iterator<Item = OsString>) -> anyhow::Result<Self> {
        for flag in ["/SP-", "/SILENT", "/NOICONS"] {
            ensure!(
                arguments.next().as_deref() == Some(OsStr::new(flag)),
                "unsupported frozen updater arguments"
            );
        }
        let scope = match arguments.next().as_deref().and_then(OsStr::to_str) {
            Some("/CURRENTUSER") => Self::CurrentUser,
            Some("/ALLUSERS") => Self::AllUsers,
            _ => anyhow::bail!("frozen updater installation scope is missing"),
        };
        ensure!(
            arguments.next().is_none(),
            "unexpected frozen updater argument"
        );
        Ok(scope)
    }

    fn switch(self) -> &'static str {
        match self {
            Self::CurrentUser => "/CURRENTUSER",
            Self::AllUsers => "/ALLUSERS",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::CurrentUser => "current-user",
            Self::AllUsers => "all-users",
        }
    }

    fn other(self) -> Self {
        match self {
            Self::CurrentUser => Self::AllUsers,
            Self::AllUsers => Self::CurrentUser,
        }
    }

    fn program_root(self, local: &Path, install_directory: &str) -> anyhow::Result<PathBuf> {
        let parent = match self {
            Self::CurrentUser => local.join("Programs"),
            Self::AllUsers => PathBuf::from(
                native::registry_value(
                    Self::AllUsers,
                    r"Software\Microsoft\Windows\CurrentVersion",
                    "ProgramFilesDir",
                )?
                .context("native Program Files is unavailable")?,
            ),
        };
        Ok(parent.join(install_directory))
    }
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Plan {
    requester: String,
    scope: Scope,
    source: PathBuf,
    source_sha256: String,
    installation: legacy::Installation,
    product_manifest: String,
    bridge_manifest: String,
    journal_id: uuid::Uuid,
    transfer_credentials: bool,
    retain_discord: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Approval {
    plan: Plan,
    challenge: serde_json::Value,
}

impl Plan {
    fn journal(&self, parent: &Path) -> PathBuf {
        parent.join(format!(".v3-bridge-data-{}", self.journal_id))
    }
}

pub(super) fn run() -> ExitCode {
    match migrate() {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => {
            native::notify(
                "ALCOMD could not complete the Windows migration. Reopen the same verified bridge to continue, or use your existing ALCOMD3 installation.",
            );
            ExitCode::FAILURE
        }
    }
}

fn migrate() -> anyhow::Result<()> {
    let scope = Scope::from_frozen_args(std::env::args_os().skip(1))?;
    ensure!(
        !windows_process_is_elevated()?,
        "start migration under the original unelevated Windows account"
    );
    let bundle = assets::embedded().context("the verified Windows release bundle is missing")?;
    let local = local_app_data_directory()?;
    let (local, _) = resolve_plain_storage_directory(&local)?;
    let old = legacy::detect(scope)?;
    let deadline = Instant::now() + WAIT_FOR_V3;
    while old.running() {
        ensure!(
            Instant::now() < deadline,
            "close the selected v3 installation"
        );
        std::thread::sleep(Duration::from_millis(150));
    }
    let source = legacy::source(&local)?;
    let data = DataConfig::default();
    let database = state_database_location(&data)?;
    let destination = database
        .parent()
        .context("product data path has no parent")?;
    let data_root = destination
        .parent()
        .context("product data parent is unavailable")?;
    let program_root = scope.program_root(&local, bundle.install_directory)?;
    let program_active = verify_active_program(&bundle, scope, &program_root)?;
    if source.is_none() {
        ensure!(
            scope == Scope::CurrentUser || program_active,
            "an existing v3 machine installation requires a verified Windows bridge commitment"
        );
        let action = if program_active {
            "Set up the verified ALCOMD installation for this account without importing data?"
        } else {
            "Install ALCOMD for this account without importing data?"
        };
        if !native::confirm(&format!(
            "No ALCOMD3 data was found for this Windows account. {action}"
        )) {
            return Ok(());
        }
    }
    let workspace = data_root.join(format!(
        ".v3-bridge-{}-{}",
        scope.label(),
        bundle.bridge_manifest
    ));
    prepare_dir(&workspace)?;
    let lock_path = workspace.join("transaction.lock");
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .share_mode(0)
        .custom_flags(0x0020_0000)
        .open(lock_path)?;
    let lock_metadata = lock.metadata()?;
    ensure!(
        lock_metadata.is_file() && lock_metadata.file_attributes() & 0x400 == 0,
        "invalid bridge transaction lock"
    );
    let extracted = workspace.join("assets");
    prepare_dir(&extracted)?;
    bundle.populate(&extracted)?;
    let _files = bundle.lock_files(&extracted)?;

    let Some(source) = source else {
        if program_active && scope == Scope::AllUsers {
            let runtime = program_root.join("runtime").join(bundle.version);
            approve_windows_empty_bridge_profile(&runtime)?;
        } else if !program_active {
            let status = start_installer(&extracted, scope, &[])?;
            ensure!(
                status == 0 && verify_active_program(&bundle, scope, &program_root)?,
                "the v4 installer did not complete"
            );
        }
        register_account(&bundle, &program_root)?;
        return Ok(());
    };
    let requester = windows_current_account_sid()?;
    let Some((plan, number)) = load_or_confirm_plan(
        &bundle, scope, &old, &source, &requester, &workspace, &extracted, data_root,
    )?
    else {
        return Ok(());
    };
    let journal = plan.journal(data_root);
    let agreed = workspace.join(format!("commit-{number}.json"));
    let program_stage = program_root
        .parent()
        .context("missing Windows program parent")?
        .join(format!(
            ".{}-bridge-{}",
            bundle.install_directory, bundle.product_manifest
        ));
    let committed = record_present(&journal.join("phase-7.json"))?;
    let approved = read_optional::<Approval>(&agreed)?;
    if let Some(approval) = &approved {
        ensure!(approval.plan == plan, "approved bridge plan changed");
        check_challenge(&bundle, scope, &serde_json::to_vec(&approval.challenge)?)?;
    }
    if program_active && committed && approved.is_some() {
        register_account(&bundle, &program_root)?;
        return Ok(());
    }
    let mut instance = None;
    let mut program_lease = None;
    if program_active {
        ensure!(
            scope == Scope::AllUsers,
            "a different v4 installation is already active"
        );
        let runtime = program_root.join("runtime").join(bundle.version);
        program_lease = hold_windows_installation(&runtime.join(bundle.daemon))?;
        ensure!(
            program_lease.is_some(),
            "unable to hold the machine installation"
        );
        instance = Some(
            DaemonInstance::acquire(&IpcConfig::default())
                .context("close this account's Core before migrating")?,
        );
    } else {
        if !program_stage.try_exists()? {
            let status = start_installer(
                &extracted,
                scope,
                &[format!("/ALCOMDStageFresh={}", bundle.product_manifest).into()],
            )?;
            ensure!(
                status == 1,
                "private Windows bridge staging did not complete"
            );
        }
        bundle.verify_program(&program_stage, scope == Scope::AllUsers)?;
    }

    let migration = extracted.join("migration");
    let launcher = format!("migration/{}", bundle.bootstrap);
    let mut arguments = vec![
        OsString::from("migrate-v3"),
        "--source-root".into(),
        source.clone().into_os_string(),
        "--recovery-executable".into(),
        old.recovery.clone().into_os_string(),
        "--work-directory".into(),
        journal.clone().into_os_string(),
        "--destination".into(),
        destination.as_os_str().to_owned(),
        "--staged-payload".into(),
        migration.clone().into_os_string(),
        "--channel".into(),
        bundle.channel.into(),
        "--install-scope".into(),
        format!("windows-{}", scope.label()).into(),
    ];
    if plan.transfer_credentials {
        arguments.push("--transfer-credentials".into());
    }
    if plan.retain_discord {
        arguments.extend([
            "--discord-consent-digest".into(),
            bundle.discord_sha256.into(),
        ]);
    }
    let recovering = record_present(&journal.join("phase-5.json"))?
        || record_present(&journal.join("phase-6.json"))?
        || committed;
    ensure!(
        !recovering || approved.is_some(),
        "native commit has no original-account approval"
    );
    if !recovering {
        ensure!(
            !destination.try_exists()? && !core_installation_registration(&data)?.try_exists()?,
            "the v4 account was already initialized"
        );
        let challenge = run_bridge_tool(&extracted.join(&launcher), &migration, &arguments)?;
        let challenge = check_challenge(&bundle, scope, &challenge)?;
        if let Some(approval) = &approved {
            ensure!(
                approval.challenge == challenge,
                "the approved native data changed"
            );
        } else if !native::confirm(&format!(
            "ALCOMD {} verified this account's staged data, including {} project records. Commit this data for the {} Windows installation?",
            bundle.version,
            challenge["records"]["projects"],
            scope.label()
        )) {
            run_bridge_tool(
                &extracted.join(&launcher),
                &migration,
                &[
                    "rollback-v3".into(),
                    "--work-directory".into(),
                    journal.clone().into_os_string(),
                ],
            )?;
            return Ok(());
        }
        ensure!(
            legacy::detect(scope)? == old
                && !old.running()
                && legacy::snapshot(&source)? == plan.source_sha256,
            "the selected v3 installation or account data changed during approval"
        );
        if approved.is_none() {
            assets::publish_bytes(&agreed, &serde_json::to_vec(&Approval { plan, challenge })?)?;
        }
    }
    let report = format!("health/{}.json", scope.label());
    let signature = format!("health/{}.sig", scope.label());
    arguments.extend([
        "--health-report".into(),
        extracted.join(&report).into_os_string(),
        "--health-signature".into(),
        extracted.join(&signature).into_os_string(),
    ]);
    run_bridge_tool(&extracted.join(&launcher), &migration, &arguments)?;
    ensure!(
        record_present(&journal.join("phase-7.json"))? && destination.is_dir(),
        "native migration did not commit"
    );
    drop(instance);
    if !program_active {
        let mut journal_argument = OsString::from("/ALCOMDBridgeJournal=");
        journal_argument.push(external_filesystem_path(&journal).as_os_str());
        let status = start_installer(
            &extracted,
            scope,
            &[
                format!("/ALCOMDFinishFresh={}", bundle.product_manifest).into(),
                journal_argument,
            ],
        )?;
        ensure!(status == 0, "Windows bridge activation did not complete");
    }
    ensure!(
        verify_active_program(&bundle, scope, &program_root)?,
        "Windows bridge did not select the exact runtime"
    );
    register_account(&bundle, &program_root)?;
    drop(program_lease);
    native::notify(
        "ALCOMD is ready. This account's data was migrated and the old installation was preserved.",
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn load_or_confirm_plan(
    bundle: &Bundle,
    scope: Scope,
    old: &legacy::Installation,
    source: &Path,
    requester: &str,
    workspace: &Path,
    extracted: &Path,
    data_root: &Path,
) -> anyhow::Result<Option<(Plan, u32)>> {
    for number in 0..32 {
        let path = workspace.join(format!("plan-{number}.json"));
        if let Some(plan) = read_optional::<Plan>(&path)? {
            if record_present(&plan.journal(data_root).join("phase-8.json"))? {
                continue;
            }
            ensure!(
                plan.requester == requester
                    && plan.scope == scope
                    && plan.installation == *old
                    && plan.source == source
                    && plan.source_sha256 == legacy::snapshot(source)?
                    && plan.product_manifest == bundle.product_manifest
                    && plan.bridge_manifest == bundle.bridge_manifest,
                "the source or candidate changed since bridge preparation"
            );
            return Ok(Some((plan, number)));
        }
        if !native::confirm(&format!(
            "Prepare ALCOMD3 3.4.0 data for ALCOMD {} in the {} Windows scope? The selected old installation and this account's original data will be preserved.",
            bundle.version,
            scope.label()
        )) {
            return Ok(None);
        }
        let reader = format!("migration/{}", bundle.migration_reader);
        let output = run_bridge_tool(
            &extracted.join(reader),
            &extracted.join("migration"),
            &[
                "inventory".into(),
                "--source-root".into(),
                source.as_os_str().into(),
            ],
        )?;
        let inventory: serde_json::Value = serde_json::from_slice(&output)?;
        let credentials = inventory["repositoryCredentials"]
            .as_u64()
            .context("missing credential inventory count")?;
        if credentials > 0
            && !native::confirm(
                "Move the v3 repository authentication into this Windows account's Credential Manager?",
            )
        {
            return Ok(None);
        }
        let retain_discord = native::confirm(
            "Allow ALCOMD to continue Discord Rich Presence if it was enabled in v3? Its existing privacy choices are preserved.",
        );
        let plan = Plan {
            requester: requester.to_owned(),
            scope,
            source: source.to_owned(),
            source_sha256: legacy::snapshot(source)?,
            installation: old.clone(),
            product_manifest: bundle.product_manifest.to_owned(),
            bridge_manifest: bundle.bridge_manifest.to_owned(),
            journal_id: uuid::Uuid::new_v4(),
            transfer_credentials: credentials > 0,
            retain_discord,
        };
        assets::publish_bytes(&path, &serde_json::to_vec(&plan)?)?;
        return Ok(Some((plan, number)));
    }
    anyhow::bail!("too many abandoned Windows bridge attempts")
}

fn read_optional<T: serde::de::DeserializeOwned>(path: &Path) -> anyhow::Result<Option<T>> {
    let file = match open_owned_installation_record(path) {
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
        Ok(file) => file,
    };
    let mut bytes = Vec::new();
    file.take(MAX_RECORD + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() as u64 <= MAX_RECORD, "oversized bridge record");
    Ok(Some(serde_json::from_slice(&bytes)?))
}

fn record_present(path: &Path) -> anyhow::Result<bool> {
    match path.symlink_metadata() {
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
        Ok(_) => {
            let _record = open_owned_installation_record(path)?;
            Ok(true)
        }
    }
}

fn check_challenge(
    bundle: &Bundle,
    scope: Scope,
    bytes: &[u8],
) -> anyhow::Result<serde_json::Value> {
    let value: serde_json::Value = serde_json::from_slice(bytes)?;
    let digest = |key: &str| -> anyhow::Result<String> {
        let bytes: [u8; 32] = serde_json::from_value(value[key].clone())?;
        ensure!(bytes != [0; 32], "missing bridge health digest");
        Ok(assets::format_digest(&bytes))
    };
    ensure!(
        value["schema"] == 1
            && value["productVersion"] == bundle.version
            && value["installScope"] == format!("windows-{}", scope.label())
            && digest("payloadManifestSha256")? == bundle.bridge_manifest
            && value["records"]["projects"].as_u64().is_some(),
        "staged health challenge does not match this migration"
    );
    digest("exportManifestSha256")?;
    digest("nativeContentSha256")?;
    Ok(value)
}

fn verify_active_program(bundle: &Bundle, scope: Scope, root: &Path) -> anyhow::Result<bool> {
    let selector = root.join(WINDOWS_ACTIVE_RUNTIME_RECORD);
    match selector.symlink_metadata() {
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
        Ok(_) => {}
    }
    let file = match scope {
        Scope::CurrentUser => open_owned_installation_record(&selector)?,
        Scope::AllUsers => open_windows_machine_installation_record(&selector)?,
    };
    let mut bytes = Vec::new();
    file.take(82).read_to_end(&mut bytes)?;
    ensure!(
        bytes == format!("{}\n", bundle.version).as_bytes(),
        "the active Windows selector does not select this candidate"
    );
    bundle.verify_program(
        &root.join("runtime").join(bundle.version),
        scope == Scope::AllUsers,
    )?;
    Ok(true)
}

fn register_account(bundle: &Bundle, program_root: &Path) -> anyhow::Result<()> {
    let runtime = program_root.join("runtime").join(bundle.version);
    let _lease = hold_windows_installation(&runtime.join(bundle.daemon))?
        .context("unable to hold the selected Windows runtime for registration")?;
    run_bridge_tool(
        &runtime.join(bundle.bootstrap),
        &runtime,
        &[
            "register-installed-core".into(),
            "--source-sha".into(),
            bundle.source_commit.into(),
        ],
    )?;
    Ok(())
}

fn prepare_dir(path: &Path) -> anyhow::Result<()> {
    match path.symlink_metadata() {
        Ok(_) => {
            resolve_plain_storage_directory(path)?;
            return Ok(());
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let parent = path.parent().context("bridge directory has no parent")?;
    prepare_dir(parent)?;
    match fs::create_dir(path) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.into()),
    }
    resolve_plain_storage_directory(path)?;
    Ok(())
}

fn start_installer(root: &Path, scope: Scope, private: &[OsString]) -> anyhow::Result<i32> {
    let installer = root.join(PRODUCT_INSTALLER);
    let mut command = Command::new(external_filesystem_path(&installer).as_os_str());
    command.current_dir(external_filesystem_path(root)).args([
        "/SP-",
        "/SILENT",
        "/NOICONS",
        scope.switch(),
        "/NORESTART",
    ]);
    if !private.is_empty() && scope == Scope::AllUsers {
        command.arg("/ALCOMDScope=all-users");
    }
    command
        .args(private)
        .status()?
        .code()
        .context("Windows setup was interrupted")
}

fn run_bridge_tool(
    program: &Path,
    directory: &Path,
    arguments: &[OsString],
) -> anyhow::Result<Vec<u8>> {
    let output = Command::new(external_filesystem_path(program).as_os_str())
        .current_dir(external_filesystem_path(directory))
        .args(arguments)
        .creation_flags(CREATE_NO_WINDOW)
        .output()?;
    ensure!(
        output.status.success() && output.stdout.len() as u64 <= MAX_RECORD,
        "verified bridge subprocess did not complete"
    );
    Ok(output.stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frozen_updater_arguments_do_not_accept_extra_or_unverified_install_scope()
    -> anyhow::Result<()> {
        let parse =
            |values: &[&str]| Scope::from_frozen_args(values.iter().copied().map(OsString::from));
        assert_eq!(
            parse(&["/SP-", "/SILENT", "/NOICONS", "/CURRENTUSER"])?,
            Scope::CurrentUser
        );
        assert_eq!(
            parse(&["/SP-", "/SILENT", "/NOICONS", "/ALLUSERS"])?,
            Scope::AllUsers
        );
        for values in [
            vec![],
            vec!["/SP-", "/SILENT", "/NOICONS"],
            vec!["/SP-", "/SILENT", "/NOICONS", "/ALLUSERS", "/DIR=C:\\Other"],
            vec!["/SP-", "/SILENT", "/NOICONS", "/CURRENTUSER", "/ALLUSERS"],
            vec!["/SP-", "/NOICONS", "/SILENT", "/CURRENTUSER"],
        ] {
            assert!(parse(&values).is_err());
        }
        Ok(())
    }

    #[test]
    fn health_challenge_is_bound_to_scope_and_candidate() -> anyhow::Result<()> {
        use serde_json::json;

        let bundle = Bundle {
            version: "4.0.0-alpha.0",
            channel: "beta",
            install_directory: "",
            source_commit: "",
            product_manifest: "",
            bridge_manifest: "3535353535353535353535353535353535353535353535353535353535353535",
            bootstrap: "",
            daemon: "",
            migration_reader: "",
            discord_sha256: "",
            files: &[],
            runtime: &[],
        };
        let mut challenge = json!({
            "schema": 1,
            "productVersion": bundle.version,
            "installScope": "windows-all-users",
            "payloadManifestSha256": ([0x35_u8; 32]),
            "exportManifestSha256": ([0x81_u8; 32]),
            "nativeContentSha256": ([0xa1_u8; 32]),
            "records": {"projects": 7}
        });
        let bytes = serde_json::to_vec(&challenge)?;
        assert_eq!(
            check_challenge(&bundle, Scope::AllUsers, &bytes)?,
            challenge
        );
        assert!(check_challenge(&bundle, Scope::CurrentUser, &bytes).is_err());
        for field in [
            "payloadManifestSha256",
            "exportManifestSha256",
            "nativeContentSha256",
        ] {
            let valid = std::mem::replace(&mut challenge[field], json!([0_u8; 32].to_vec()));
            assert!(
                check_challenge(&bundle, Scope::AllUsers, &serde_json::to_vec(&challenge)?)
                    .is_err()
            );
            challenge[field] = valid;
        }
        Ok(())
    }
}
