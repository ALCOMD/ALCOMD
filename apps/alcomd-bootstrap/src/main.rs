use std::path::PathBuf;

use alcomd_bootstrap::{
    HealthReport, InstallScope, MigrationOutcome, MigrationRequest, migrate_v3, rollback_v3,
};
use alcomd_updater::Channel;
use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(version, about = "ALCOMD product migration coordinator")]
struct Arguments {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Migrate published v3 state or continue an interrupted migration.
    MigrateV3(Box<MigrateArguments>),
    /// Discard uncommitted outputs of an existing migration.
    RollbackV3 {
        #[arg(long)]
        work_directory: PathBuf,
    },
    #[cfg(target_os = "linux")]
    /// Migrate this account using a separate AppImage or a package-manager installed DEB.
    BridgeLinuxV3(Box<LinuxBridgeArguments>),
    #[cfg(windows)]
    /// Initialize the current user's Core and publish its installed runtime registration.
    RegisterInstalledCore(InstallCoreArguments),
    #[cfg(windows)]
    /// Remove the current user's registration when it belongs to this installed runtime.
    UnregisterInstalledCore(InstallCoreArguments),
    #[cfg(windows)]
    #[command(hide = true)]
    WindowsInstallRoot {
        #[arg(long, value_enum)]
        action: WindowsInstallRootAction,
        #[arg(long)]
        root: PathBuf,
        #[arg(long)]
        machine: bool,
    },
    #[cfg(windows)]
    #[command(hide = true)]
    VerifyInstalledUninstall {
        #[arg(long)]
        source_sha: String,
    },
    #[cfg(windows)]
    #[command(hide = true)]
    VerifyWindowsBridgeCommit {
        #[arg(long)]
        work_directory: PathBuf,
        #[arg(long, value_enum)]
        scope: alcomd_bootstrap::WindowsInstallScope,
        #[arg(long)]
        manifest_sha256: String,
    },
    #[cfg(windows)]
    #[command(hide = true)]
    CoordinateWindowsScopeSource {
        #[arg(long, value_enum)]
        action: alcomd_bootstrap::ScopeSourceAction,
        #[arg(long, value_enum)]
        destination: alcomd_bootstrap::WindowsInstallScope,
        #[arg(long)]
        requester_sid: String,
        #[arg(long)]
        source_version: String,
        #[arg(long)]
        destination_version: String,
        #[arg(long)]
        staged_runtime: PathBuf,
        #[arg(long)]
        approved_plan: Option<String>,
    },
    #[cfg(windows)]
    #[command(hide = true)]
    WindowsScopePlanFingerprint(alcomd_bootstrap::WindowsScopePlanArguments),
    #[cfg(windows)]
    #[command(hide = true)]
    CoordinateWindowsScopeDestination {
        #[arg(long, value_enum)]
        action: alcomd_bootstrap::ScopeDestinationAction,
        #[command(flatten)]
        plan: alcomd_bootstrap::WindowsScopePlanArguments,
        #[arg(long)]
        source_root: Option<PathBuf>,
    },
    #[cfg(windows)]
    #[command(hide = true)]
    WindowsScopeAccountSid {
        #[arg(long)]
        expected: Option<String>,
    },
    #[cfg(windows)]
    #[command(hide = true)]
    RequestWindowsMachineScopeSource {
        #[arg(long, value_enum)]
        action: alcomd_bootstrap::ScopeSourceAction,
        #[arg(long)]
        source_version: String,
        #[arg(long)]
        destination_version: String,
        #[arg(long)]
        staged_runtime: PathBuf,
        #[arg(long)]
        approved_plan: String,
    },
    #[cfg(windows)]
    #[command(hide = true)]
    CompleteWindowsScopeAccount {
        #[arg(long)]
        requester_sid: String,
        #[arg(long)]
        source_sha: String,
    },
    #[cfg(windows)]
    #[command(hide = true)]
    RemoveRetiredWindowsScope {
        #[arg(long, value_enum)]
        source: alcomd_bootstrap::WindowsInstallScope,
        #[arg(long)]
        requester_sid: String,
        #[arg(long)]
        uninstaller: PathBuf,
        #[arg(long)]
        approved_plan: String,
    },
    #[cfg(windows)]
    #[command(hide = true)]
    VerifyRetiredWindowsScopeUninstall {
        #[arg(long)]
        source_sha: String,
        #[arg(long)]
        approved_plan: String,
        #[arg(long)]
        machine: bool,
    },
    #[cfg(windows)]
    /// Prepare a verified update for a current-user Windows installation.
    StageInstalledUpdate(UpdateArguments),
    #[cfg(windows)]
    /// Install a verified update in the original Windows installation scope.
    ApplyInstalledUpdate {
        #[command(flatten)]
        update: UpdateArguments,
        #[arg(long, hide = true)]
        desktop: bool,
    },
    #[cfg(windows)]
    /// Recover an interrupted installed Windows update.
    ResumeInstalledUpdate {
        #[arg(long, hide = true)]
        desktop: bool,
    },
    #[cfg(windows)]
    #[command(hide = true)]
    ApplyMachineUpdateWorker {
        #[arg(long)]
        transfer: String,
        #[arg(long)]
        requester_pid: u32,
        #[arg(long, value_enum)]
        channel: Channel,
    },
    #[cfg(windows)]
    #[command(hide = true)]
    ResumeMachineUpdateWorker,
    #[cfg(any(windows, target_os = "linux"))]
    #[command(hide = true)]
    StartInstalledCore,
    #[cfg(target_os = "linux")]
    #[command(hide = true)]
    StartPortable {
        #[arg(last = true)]
        arguments: Vec<std::ffi::OsString>,
    },
    #[cfg(target_os = "linux")]
    #[command(hide = true)]
    PortableCli {
        #[arg(value_enum)]
        action: alcomd_bootstrap::PortableCliAction,
    },
    #[cfg(target_os = "linux")]
    #[command(hide = true)]
    ApplyPortableUpdate {
        #[command(flatten)]
        update: UpdateArguments,
        #[arg(long)]
        desktop: bool,
    },
    #[cfg(target_os = "linux")]
    #[command(hide = true)]
    ResumePortableUpdate {
        #[arg(long)]
        desktop: bool,
    },
}

#[cfg(windows)]
#[derive(Clone, Debug, clap::ValueEnum)]
enum WindowsInstallRootAction {
    Check,
    Reserve,
    Release,
}

#[cfg(windows)]
#[derive(Debug, Args)]
struct InstallCoreArguments {
    #[arg(long)]
    source_sha: String,
    #[arg(long, hide = true)]
    data_dir: Option<PathBuf>,
}

#[cfg(any(windows, target_os = "linux"))]
#[derive(Debug, Args)]
struct UpdateArguments {
    #[arg(long)]
    envelope: PathBuf,
    #[arg(long)]
    asset: PathBuf,
    #[arg(long)]
    replay_directory: PathBuf,
    #[arg(long, value_enum)]
    channel: Channel,
}

#[cfg(any(windows, target_os = "linux"))]
impl UpdateArguments {
    fn absolute_paths(&self) -> std::io::Result<[PathBuf; 3]> {
        Ok([
            std::path::absolute(&self.envelope)?,
            std::path::absolute(&self.asset)?,
            std::path::absolute(&self.replay_directory)?,
        ])
    }
}

#[derive(Debug, Args)]
struct MigrateArguments {
    #[arg(long)]
    source_root: PathBuf,
    #[arg(long)]
    recovery_executable: PathBuf,
    #[arg(long)]
    work_directory: PathBuf,
    #[arg(long)]
    destination: PathBuf,
    #[arg(long)]
    staged_payload: PathBuf,
    #[arg(long, value_enum)]
    channel: Channel,
    #[arg(long, value_enum)]
    install_scope: InstallScope,
    #[arg(long, requires = "health_signature")]
    health_report: Option<PathBuf>,
    #[arg(long, requires = "health_report")]
    health_signature: Option<PathBuf>,
    /// Store this user's repository authentication in this user's credential vault.
    #[arg(long)]
    transfer_credentials: bool,
    /// Consent to migrate the enabled Discord integration for this exact package digest.
    #[arg(long)]
    discord_consent_digest: Option<String>,
}

#[cfg(target_os = "linux")]
#[derive(Debug, Args)]
struct LinuxBridgeArguments {
    #[command(flatten)]
    migration: MigrateArguments,
    /// The retained original v3.4.0 DEB used to restore the old application through apt or dpkg.
    #[arg(long)]
    recovery_package: Option<PathBuf>,
}

impl From<MigrateArguments> for MigrationRequest {
    fn from(args: MigrateArguments) -> Self {
        Self {
            source_root: args.source_root,
            recovery_executable: args.recovery_executable,
            work_directory: args.work_directory,
            destination: args.destination,
            staged_payload: args.staged_payload,
            channel: args.channel,
            install_scope: args.install_scope,
            transfer_credentials: args.transfer_credentials,
            discord_consent_digest: args.discord_consent_digest,
            health_report: args
                .health_report
                .zip(args.health_signature)
                .map(|(report, signature)| HealthReport { report, signature }),
        }
    }
}

fn main() -> anyhow::Result<()> {
    #[cfg(target_os = "linux")]
    alcomd_bootstrap::start_portable_cli_if_selected()?;
    let arguments = match Arguments::parse().command {
        Command::MigrateV3(arguments) => arguments,
        Command::RollbackV3 { work_directory } => {
            rollback_v3(&work_directory)?;
            println!("Uncommitted migration staging discarded");
            return Ok(());
        }
        #[cfg(target_os = "linux")]
        Command::BridgeLinuxV3(arguments) => {
            let request = MigrationRequest::from(arguments.migration);
            let outcome =
                alcomd_bootstrap::bridge_linux_v3(&request, arguments.recovery_package.as_deref())?;
            let committed = !matches!(outcome, MigrationOutcome::AwaitingHealth(_));
            report_migration_outcome(outcome)?;
            if committed {
                return alcomd_bootstrap::start_linux_bridge_desktop(request.install_scope);
            }
            return Ok(());
        }
        #[cfg(windows)]
        Command::RegisterInstalledCore(arguments) => {
            return alcomd_bootstrap::register_installed_core(
                arguments.source_sha,
                arguments.data_dir.as_deref(),
            );
        }
        #[cfg(windows)]
        Command::UnregisterInstalledCore(arguments) => {
            return alcomd_bootstrap::unregister_installed_core(
                arguments.source_sha,
                arguments.data_dir.as_deref(),
            );
        }
        #[cfg(windows)]
        Command::WindowsInstallRoot {
            action,
            root,
            machine,
        } => {
            use alcomd_platform::{
                release_windows_installation_root, reserve_windows_installation_root,
                validate_windows_installation_root,
            };
            match action {
                WindowsInstallRootAction::Check => {
                    validate_windows_installation_root(machine, &root)?
                }
                WindowsInstallRootAction::Reserve => {
                    reserve_windows_installation_root(machine, &root)?
                }
                WindowsInstallRootAction::Release => {
                    release_windows_installation_root(machine, &root)?
                }
            }
            return Ok(());
        }
        #[cfg(windows)]
        Command::VerifyInstalledUninstall { source_sha } => {
            return alcomd_bootstrap::verify_installed_machine_uninstall(source_sha);
        }
        #[cfg(windows)]
        Command::CoordinateWindowsScopeSource {
            action,
            destination,
            requester_sid,
            source_version,
            destination_version,
            staged_runtime,
            approved_plan,
        } => {
            let result = alcomd_bootstrap::coordinate_windows_scope_source(
                action,
                destination,
                &requester_sid,
                &source_version,
                &destination_version,
                &std::path::absolute(staged_runtime)?,
                approved_plan.as_deref(),
            )?;
            println!("{result}");
            return Ok(());
        }
        #[cfg(windows)]
        Command::VerifyWindowsBridgeCommit {
            work_directory,
            scope,
            manifest_sha256,
        } => {
            return alcomd_bootstrap::verify_windows_bridge_commit(
                &work_directory,
                scope,
                &manifest_sha256,
            );
        }
        #[cfg(windows)]
        Command::WindowsScopePlanFingerprint(plan) => {
            let digest = alcomd_bootstrap::windows_scope_plan_fingerprint(plan)?;
            println!("{digest}");
            return Ok(());
        }
        #[cfg(windows)]
        Command::CoordinateWindowsScopeDestination {
            action,
            plan,
            source_root,
        } => {
            return alcomd_bootstrap::coordinate_windows_scope_destination(
                action,
                plan,
                source_root.as_deref(),
            );
        }
        #[cfg(windows)]
        Command::WindowsScopeAccountSid { expected } => {
            let sid = alcomd_platform::windows_current_account_sid()?;
            anyhow::ensure!(
                !alcomd_platform::windows_process_is_elevated()?
                    && expected.as_ref().is_none_or(|expected| expected == &sid),
                "Windows scope conversion requires the original non-elevated account"
            );
            println!("{sid}");
            return Ok(());
        }
        #[cfg(windows)]
        Command::RequestWindowsMachineScopeSource {
            action,
            source_version,
            destination_version,
            staged_runtime,
            approved_plan,
        } => {
            return alcomd_bootstrap::request_windows_machine_scope_source(
                action,
                &source_version,
                &destination_version,
                &std::path::absolute(staged_runtime)?,
                &approved_plan,
            );
        }
        #[cfg(windows)]
        Command::CompleteWindowsScopeAccount {
            requester_sid,
            source_sha,
        } => {
            return alcomd_bootstrap::complete_windows_scope_account(&requester_sid, source_sha);
        }
        #[cfg(windows)]
        Command::RemoveRetiredWindowsScope {
            source,
            requester_sid,
            uninstaller,
            approved_plan,
        } => {
            return alcomd_bootstrap::remove_retired_windows_scope(
                source,
                &requester_sid,
                &std::path::absolute(uninstaller)?,
                &approved_plan,
            );
        }
        #[cfg(windows)]
        Command::VerifyRetiredWindowsScopeUninstall {
            source_sha,
            approved_plan,
            machine,
        } => {
            return alcomd_bootstrap::verify_retired_windows_scope_uninstall(
                source_sha,
                &approved_plan,
                machine,
            );
        }
        #[cfg(windows)]
        Command::ApplyMachineUpdateWorker {
            transfer,
            requester_pid,
            channel,
        } => {
            let outcome =
                alcomd_bootstrap::apply_machine_update_worker(&transfer, requester_pid, channel);
            if let Err(error) = &outcome
                && error.chain().any(|cause| {
                    cause
                        .downcast_ref::<std::io::Error>()
                        .is_some_and(|io| io.kind() == std::io::ErrorKind::StorageFull)
                })
            {
                eprintln!("Error: {error:#}");
                std::process::exit(
                    alcomd_platform::WindowsElevatedProcess::MACHINE_UPDATE_DISK_FULL_EXIT_CODE,
                );
            }
            return outcome;
        }
        #[cfg(windows)]
        Command::ResumeMachineUpdateWorker => {
            return alcomd_bootstrap::resume_machine_update_worker();
        }
        #[cfg(windows)]
        Command::StageInstalledUpdate(arguments) => {
            let [envelope, asset, replay_directory] = arguments.absolute_paths()?;
            let staged = alcomd_bootstrap::stage_installed_update(
                &envelope,
                &asset,
                &replay_directory,
                arguments.channel,
            )?;
            println!("Verified update staged at {}", staged.display());
            return Ok(());
        }
        #[cfg(windows)]
        Command::ApplyInstalledUpdate { update, desktop } => {
            let [envelope, asset, replay_directory] = update.absolute_paths()?;
            let apply = || {
                alcomd_bootstrap::apply_installed_update(
                    &envelope,
                    &asset,
                    &replay_directory,
                    update.channel,
                )
            };
            if desktop {
                alcomd_bootstrap::with_windows_desktop_restart(apply)?;
            } else {
                apply()?;
            }
            println!("Windows product update applied");
            return Ok(());
        }
        #[cfg(windows)]
        Command::ResumeInstalledUpdate { desktop } => {
            if desktop {
                alcomd_bootstrap::with_windows_desktop_restart(
                    alcomd_bootstrap::resume_installed_update,
                )?;
            } else {
                alcomd_bootstrap::resume_installed_update()?;
            }
            println!("Windows product update resumed");
            return Ok(());
        }
        #[cfg(any(windows, target_os = "linux"))]
        Command::StartInstalledCore => return alcomd_bootstrap::start_installed_core(),
        #[cfg(target_os = "linux")]
        Command::StartPortable { arguments } => {
            return alcomd_bootstrap::start_portable(&arguments);
        }
        #[cfg(target_os = "linux")]
        Command::PortableCli { action } => {
            println!(
                "{}",
                serde_json::to_string(&alcomd_bootstrap::portable_cli(action)?)?
            );
            return Ok(());
        }
        #[cfg(target_os = "linux")]
        Command::ApplyPortableUpdate { update, desktop } => {
            let [envelope, asset, replay_directory] = update.absolute_paths()?;
            return alcomd_bootstrap::apply_portable_update(
                &envelope,
                &asset,
                &replay_directory,
                update.channel,
                desktop,
            );
        }
        #[cfg(target_os = "linux")]
        Command::ResumePortableUpdate { desktop } => {
            return alcomd_bootstrap::resume_portable_update(desktop);
        }
    };
    report_migration_outcome(migrate_v3(&(*arguments).into())?)
}

fn report_migration_outcome(outcome: MigrationOutcome) -> anyhow::Result<()> {
    match outcome {
        MigrationOutcome::AwaitingHealth(challenge) => {
            println!("{}", serde_json::to_string(&challenge)?)
        }
        MigrationOutcome::Committed => println!("Native v4 state committed"),
        MigrationOutcome::AlreadyCommitted => println!("Native v4 state was already committed"),
    }
    Ok(())
}
