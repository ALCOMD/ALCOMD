//! Coordinates migration against a verified product and a fresh per-user data directory.

mod journal;
#[cfg(target_os = "linux")]
mod linux_portable;
#[cfg(target_os = "linux")]
pub use linux_portable::start_portable;
#[cfg(windows)]
mod windows_installation;
#[cfg(windows)]
pub use windows_installation::{
    register_installed_core, stage_installed_update, start_installed_core,
    unregister_installed_core,
};

/// Verifies and starts the installed Core in the calling Linux account.
#[cfg(target_os = "linux")]
pub fn start_installed_core() -> anyhow::Result<()> {
    use std::os::unix::process::CommandExt;

    let data = alcomd_platform::DataConfig::default();
    let runtime = alcomd_platform::linux_installation_runtime(&std::env::current_exe()?, &data)?;
    let version = env!("CARGO_PKG_VERSION").parse()?;
    if runtime == alcomd_platform::linux_portable_runtime(&data)? {
        alcomd_updater::distribution::verify_portable_runtime(&runtime, &version)
    } else {
        alcomd_updater::distribution::verify_installed_release(
            &runtime,
            alcomd_updater::distribution::Target::Linux,
            &version,
        )
    }
    .context("Linux installed product payload did not verify")?;
    Err(
        Command::new(runtime.join(alcomd_platform::CORE_EXECUTABLE_NAME))
            .args(["register-installation", "--serve"])
            .current_dir(runtime)
            .env_remove("ALCOMD_MCP_CLIENT_CREDENTIAL")
            .exec(),
    )
    .context("installed Core could not start")
}

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fs;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use alcomd_import::{NativeState, migration_credential_reference};
use alcomd_updater::{Channel, VerifiedPayload, decode_hex, hash_exact, read_limited};
use anyhow::{Context, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use journal::{Journal, isolated_child};

const SOURCE_VERSION: &str = "3.4.0";
const EXE_LIMIT: u64 = 1024 * 1024 * 1024;
const BUNDLE_MANIFEST_LIMIT: u64 = 1024 * 1024;
const BUNDLE_FILE_LIMIT: u64 = 8 * 1024 * 1024 * 1024;
const OUTPUT_LIMIT: u64 = 64 * 1024;
const NATIVE_INPUT_LIMIT: u64 = 64 * 1024 * 1024;
const PHASE_TIMEOUT: Duration = Duration::from_secs(300);
const PLAN_RECORD: &str = "plan.json";
const EXPORT_DIR: &str = "export-stage";
const STATE_DIR: &str = "native-stage";
const BUNDLE_DIR: &str = "bundle";
const DATA_DIR: &str = "data";
const DISCORD_PACKAGE: &str = "extensions/discord.alcomdextension";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum InstallScope {
    WindowsCurrentUser,
    WindowsAllUsers,
    Macos,
    Appimage,
    Deb,
}

impl InstallScope {
    fn supported(self) -> bool {
        matches!(
            (std::env::consts::OS, self),
            ("windows", Self::WindowsCurrentUser | Self::WindowsAllUsers)
                | ("macos", Self::Macos)
                | ("linux", Self::Appimage | Self::Deb)
        )
    }
}

pub struct MigrationRequest {
    pub source_root: PathBuf,
    pub recovery_executable: PathBuf,
    pub work_directory: PathBuf,
    pub destination: PathBuf,
    pub staged_payload: PathBuf,
    pub channel: Channel,
    pub install_scope: InstallScope,
    pub health_report: Option<HealthReport>,
    pub transfer_credentials: bool,
    pub discord_consent_digest: Option<String>,
}

pub struct HealthReport {
    pub report: PathBuf,
    pub signature: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Plan {
    workspace: PathBuf,
    source_root: PathBuf,
    source_identity: Vec<u8>,
    recovery_executable: PathBuf,
    recovery_file: FileEvidence,
    destination: PathBuf,
    destination_parent_identity: Vec<u8>,
    payload_root: PathBuf,
    payload_identity: Vec<u8>,
    payload_manifest_sha256: [u8; 32],
    target_version: String,
    channel: Channel,
    install_scope: InstallScope,
    credential_transfer_id: Option<Uuid>,
    discord_consent_digest: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FileEvidence {
    identity: Vec<u8>,
    size: u64,
    sha256: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExportEvidence {
    manifest_sha256: [u8; 32],
    records: BTreeMap<String, u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StateEvidence {
    directory_identity: Vec<u8>,
    state: FileEvidence,
    settings: FileEvidence,
    content_sha256: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
enum Phase {
    Frozen,
    ExportValidated(ExportEvidence),
    NativeStaged,
    NativeChecked(StateEvidence),
    Healthy(StateEvidence),
    CommitIntent(StateEvidence),
    Committed,
    RolledBack,
}

impl Phase {
    const fn index(&self) -> u8 {
        match self {
            Self::Frozen => 1,
            Self::ExportValidated(_) => 2,
            Self::NativeStaged => 3,
            Self::NativeChecked(_) => 4,
            Self::Healthy(_) => 5,
            Self::CommitIntent(_) => 6,
            Self::Committed => 7,
            Self::RolledBack => 8,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum MigrationOutcome {
    AwaitingHealth(HealthChallenge),
    Committed,
    AlreadyCommitted,
}

/// Resume a verified migration or stage a fresh one. Program deployment and old-instance
/// cleanup are owned by the platform installer and require their own authorization.
pub fn migrate_v3(request: &MigrationRequest) -> anyhow::Result<MigrationOutcome> {
    ensure!(
        request.install_scope.supported(),
        "migration scope does not match the current platform"
    );
    let payload = alcomd_updater::verify_staged_payload(&request.staged_payload, request.channel)?;
    let migration_reader = payload
        .binary("migration_v3")
        .context("signed migration reader missing")?;
    let daemon = payload.binary("daemon").context("signed Core missing")?;
    let mut plan = plan(request, &payload)?;
    let journal = Journal::open(&plan.workspace)?;
    let mut phase = latest_phase(&journal)?;
    if let Some(previous) = journal.read::<Plan>(PLAN_RECORD)? {
        ensure!(
            previous.credential_transfer_id.is_some() == request.transfer_credentials,
            "credential transfer consent differs from the existing migration"
        );
        plan.credential_transfer_id = previous.credential_transfer_id;
        ensure!(
            previous == plan,
            "migration source, signed staging or destination changed"
        );
    } else {
        ensure!(phase.is_none(), "migration journal is missing its plan");
        ensure!(
            fs::read_dir(journal.root())?.all(|entry| entry.is_ok_and(|entry| {
                matches!(
                    entry.file_name().to_str(),
                    Some("transaction.lock" | "plan.json.partial")
                )
            })),
            "workspace contains unrecognized files"
        );
        ensure_absent(&plan.destination)?;
        plan.credential_transfer_id = request.transfer_credentials.then(Uuid::new_v4);
        journal.record(PLAN_RECORD, &plan)?;
    }
    ensure_quiescent_stage(&plan)?;
    let mut steps = Steps {
        journal,
        plan,
        migration_reader,
        daemon,
    };
    match phase {
        Some(Phase::Committed) => {
            steps.cleanup_stage()?;
            return Ok(MigrationOutcome::AlreadyCommitted);
        }
        Some(Phase::RolledBack) => {
            anyhow::bail!("migration rolled back; use a fresh migration workspace")
        }
        _ => {}
    }
    let outcome = steps.run(&mut phase, request.health_report.as_ref());
    if outcome.is_err()
        && !matches!(
            phase,
            Some(Phase::NativeChecked(_) | Phase::CommitIntent(_) | Phase::Committed)
        )
        && latest_phase(&steps.journal).is_ok_and(|durable| {
            !matches!(
                durable,
                Some(Phase::CommitIntent(_) | Phase::Committed | Phase::RolledBack)
            )
        })
    {
        // Only disposable outputs beneath the verified private workspace are removed.
        rollback(&steps.journal, &steps.plan)?;
    }
    outcome
}

/// Discard a known uncommitted migration and its own temporary credential references.
pub fn rollback_v3(workspace: &Path) -> anyhow::Result<()> {
    let (workspace, _) = alcomd_platform::resolve_plain_storage_directory(workspace)?;
    let journal = Journal::open(&workspace)?;
    let plan = journal
        .read::<Plan>(PLAN_RECORD)?
        .context("migration plan is missing")?;
    ensure!(
        journal.root() == plan.workspace,
        "migration workspace moved"
    );
    let phase = latest_phase(&journal)?;
    if matches!(phase, Some(Phase::RolledBack)) {
        return Ok(());
    }
    ensure!(
        !matches!(phase, Some(Phase::CommitIntent(_) | Phase::Committed)),
        "migration reached its commit point and must be recovered forward"
    );
    ensure_absent(&plan.destination)?;
    rollback(&journal, &plan)
}

fn latest_phase(journal: &Journal) -> anyhow::Result<Option<Phase>> {
    let mut latest = None;
    for index in 1..=8 {
        if let Some(phase) = journal.read::<Phase>(&phase_file(index))? {
            ensure!(phase.index() == index, "invalid migration journal phase");
            if index <= 7 {
                ensure!(
                    latest.as_ref().map_or(0, Phase::index) + 1 == index,
                    "migration journal phase is missing"
                );
            } else {
                ensure!(
                    !matches!(latest, Some(Phase::CommitIntent(_) | Phase::Committed)),
                    "invalid rollback after the migration commit point"
                );
            }
            latest = Some(phase);
        }
    }
    Ok(latest)
}

fn phase_file(index: u8) -> String {
    format!("phase-{index}.json")
}

fn plan(request: &MigrationRequest, payload: &VerifiedPayload) -> anyhow::Result<Plan> {
    let (source_root, source_identity) =
        alcomd_platform::resolve_plain_storage_directory(&request.source_root)?;
    let (recovery_executable, _) = isolated_child(&request.recovery_executable)?;
    let (workspace, workspace_parent_identity) = isolated_child(&request.work_directory)?;
    let (destination, destination_parent_identity) = isolated_child(&request.destination)?;
    ensure!(
        workspace_parent_identity.get(..8) == destination_parent_identity.get(..8),
        "migration workspace and destination must be on the same volume"
    );
    let roots: [&Path; 4] = [&source_root, &workspace, &destination, payload.root()];
    for (index, root) in roots.iter().enumerate() {
        for other in &roots[index + 1..] {
            ensure!(
                !root.starts_with(other) && !other.starts_with(root),
                "migration source, workspace, destination and payload must be separate"
            );
        }
    }
    ensure!(
        [&workspace, &destination, payload.root()]
            .iter()
            .all(|path| !recovery_executable.starts_with(path)),
        "migration recovery executable must stay outside v4 state and staging"
    );

    Ok(Plan {
        workspace,
        source_root,
        source_identity,
        recovery_file: file_evidence(&recovery_executable, EXE_LIMIT)?,
        recovery_executable,
        destination,
        destination_parent_identity,
        payload_root: payload.root().to_owned(),
        payload_identity: payload.identity().to_vec(),
        payload_manifest_sha256: payload.manifest_sha256(),
        target_version: payload.version().to_string(),
        channel: request.channel,
        install_scope: request.install_scope,
        credential_transfer_id: None,
        discord_consent_digest: request
            .discord_consent_digest
            .as_ref()
            .map(|digest| decode_hex::<32>(digest).map(|bytes| encode_digest(&bytes)))
            .transpose()?,
    })
}

fn ensure_quiescent_stage(plan: &Plan) -> anyhow::Result<()> {
    let running = alcomd_platform::observe_processes();
    let current = std::process::id();
    let active = running.processes().iter().any(|process| {
        process.pid() != current
            && process.executable().is_some_and(|path| {
                path.starts_with(&plan.payload_root)
                    || fs::canonicalize(path).is_ok_and(|path| path.starts_with(&plan.payload_root))
            })
            && process.arguments().is_none_or(|arguments| {
                arguments
                    .iter()
                    .any(|argument| Path::new(argument).starts_with(&plan.workspace))
            })
    });
    ensure!(
        !active,
        "a verified subprocess still holds this migration staging"
    );
    Ok(())
}

struct Steps {
    journal: Journal,
    plan: Plan,
    migration_reader: PathBuf,
    daemon: PathBuf,
}

impl Steps {
    fn run(
        &mut self,
        phase: &mut Option<Phase>,
        report: Option<&HealthReport>,
    ) -> anyhow::Result<MigrationOutcome> {
        loop {
            self.journal.check()?;
            if matches!(phase, Some(Phase::Committed)) {
                self.cleanup_stage()?;
                return Ok(MigrationOutcome::Committed);
            }
            self.verify_recovery_source()?;
            let next = match phase.as_ref() {
                None => self.freeze()?,
                Some(Phase::Frozen) => self.export()?,
                Some(Phase::ExportValidated(export)) => self.import(export)?,
                Some(Phase::NativeStaged) => self.check_native_state()?,
                Some(Phase::NativeChecked(evidence)) => {
                    let challenge = self.health_challenge(evidence)?;
                    let Some(report) = report else {
                        return Ok(MigrationOutcome::AwaitingHealth(challenge));
                    };
                    check_platform_health(&challenge, report)?;
                    Phase::Healthy(evidence.clone())
                }
                Some(Phase::Healthy(evidence)) => {
                    ensure_absent(&self.plan.destination)?;
                    ensure!(
                        state_evidence(&self.staged_data())? == *evidence,
                        "healthy v4 data changed"
                    );
                    Phase::CommitIntent(evidence.clone())
                }
                Some(Phase::CommitIntent(evidence)) => self.commit(evidence)?,
                Some(Phase::Committed) => {
                    unreachable!("commit is returned before source validation")
                }
                Some(Phase::RolledBack) => anyhow::bail!("migration has already rolled back"),
            };
            self.journal.record(&phase_file(next.index()), &next)?;
            *phase = Some(next);
        }
    }

    fn verify_recovery_source(&self) -> anyhow::Result<()> {
        ensure!(
            alcomd_platform::resolve_plain_storage_directory(&self.plan.source_root)?.1
                == self.plan.source_identity
                && file_evidence(&self.plan.recovery_executable, EXE_LIMIT)?
                    == self.plan.recovery_file,
            "v3 recovery source changed"
        );
        let processes = alcomd_platform::observe_processes();
        let running = processes.processes().iter().any(|process| {
            process.executable().is_some_and(|path| {
                path.file_name() == self.plan.recovery_executable.file_name()
                    && alcomd_platform::file_identity_key(path)
                        .is_ok_and(|identity| identity == self.plan.recovery_file.identity)
            }) || process
                .arguments()
                .and_then(|args| args.first())
                .is_some_and(|arg0| Path::new(arg0) == self.plan.recovery_executable)
        });
        ensure!(
            !running,
            "close the selected v3 installation before migrating"
        );
        Ok(())
    }

    fn check_payload(&self) -> anyhow::Result<VerifiedPayload> {
        let verified =
            alcomd_updater::verify_staged_payload(&self.plan.payload_root, self.plan.channel)?;
        ensure!(
            verified.manifest_sha256() == self.plan.payload_manifest_sha256
                && verified.identity() == self.plan.payload_identity,
            "signed migration staging changed"
        );
        Ok(verified)
    }

    fn freeze(&self) -> anyhow::Result<Phase> {
        self.check_payload()?;
        run_command(
            &self.migration_reader,
            &self.plan.payload_root,
            &[
                OsStr::new("inventory"),
                OsStr::new("--source-root"),
                self.plan.source_root.as_os_str(),
            ],
        )?;
        Ok(Phase::Frozen)
    }

    fn export(&self) -> anyhow::Result<Phase> {
        self.check_payload()?;
        // On restart, clean up only the previous attempt in this transaction's vault namespace.
        remove_transferred_credentials(&self.journal, &self.plan)?;
        let destination = self.reset_stage(EXPORT_DIR)?.join(BUNDLE_DIR);
        let mut args = vec![
            OsStr::new("export"),
            OsStr::new("--source-root"),
            self.plan.source_root.as_os_str(),
            OsStr::new("--output"),
            destination.as_os_str(),
        ];
        let transfer_id = self.plan.credential_transfer_id.map(|id| id.to_string());
        if let Some(id) = &transfer_id {
            args.extend([
                OsStr::new("--transfer-credentials"),
                OsStr::new("--credential-transfer-id"),
                OsStr::new(id),
            ]);
        }
        run_command(&self.migration_reader, &self.plan.payload_root, &args)?;
        let exported = validate_bundle(&destination)?;
        if let Some(id) = self.plan.credential_transfer_id {
            staged_credential_references(&destination, id)?;
        }
        Ok(Phase::ExportValidated(exported))
    }

    fn native_command(
        &self,
        action: &str,
        bundle: &Path,
        staged_data: &Path,
    ) -> anyhow::Result<Vec<u8>> {
        let payload = self.check_payload()?;
        let mut args = vec![
            OsStr::new(action),
            OsStr::new("--bundle"),
            bundle.as_os_str(),
            OsStr::new("--data-dir"),
            staged_data.as_os_str(),
        ];
        let discord = payload.file(DISCORD_PACKAGE);
        let digest = payload
            .file_sha256(DISCORD_PACKAGE)
            .map(|hash| encode_digest(&hash));
        if let Some((discord, digest)) = discord.as_ref().zip(digest.as_ref()) {
            args.extend([
                OsStr::new("--discord-package"),
                discord.as_os_str(),
                OsStr::new("--discord-sha256"),
                OsStr::new(digest),
                OsStr::new("--publish-data-dir"),
                self.plan.destination.as_os_str(),
            ]);
        }
        if let Some(consent) = &self.plan.discord_consent_digest {
            ensure!(
                discord.is_some(),
                "staged payload lacks the consented Discord package"
            );
            args.extend([OsStr::new("--discord-consent-digest"), OsStr::new(consent)]);
        }
        run_command(&self.daemon, &self.plan.payload_root, &args)
    }

    fn import(&self, evidence: &ExportEvidence) -> anyhow::Result<Phase> {
        let bundle = self.bundle();
        ensure!(
            validate_bundle(&bundle)? == *evidence,
            "validated migration export changed"
        );
        let destination = self.reset_stage(STATE_DIR)?.join(DATA_DIR);
        self.native_command("import-native-state", &bundle, &destination)
            .context("could not build native migration state")?;
        state_evidence(&destination)?;
        Ok(Phase::NativeStaged)
    }

    fn exported_evidence(&self) -> anyhow::Result<ExportEvidence> {
        let Some(Phase::ExportValidated(export)) = self.journal.read::<Phase>(&phase_file(2))?
        else {
            anyhow::bail!("validated export missing from migration journal");
        };
        ensure!(
            validate_bundle(&self.bundle())? == export,
            "validated migration export changed"
        );
        Ok(export)
    }

    fn check_native_state(&self) -> anyhow::Result<Phase> {
        let export = self.exported_evidence()?;
        let bundle = self.bundle();
        let destination = self.staged_data();
        let output = self
            .native_command("check-native-state", &bundle, &destination)
            .context("could not reopen and reconcile native migration state")?;
        let health: NativeHealth = serde_json::from_slice(&output)
            .context("Core did not report a valid native migration check")?;
        ensure!(
            health.schema == 1
                && health.data_schema == 1
                && health.config_schema == 1
                && health.records == export.records,
            "native state check or reconciliation is incomplete"
        );
        Ok(Phase::NativeChecked(state_evidence(&destination)?))
    }

    fn health_challenge(&self, evidence: &StateEvidence) -> anyhow::Result<HealthChallenge> {
        ensure!(
            state_evidence(&self.staged_data())? == *evidence,
            "verified native state changed"
        );
        let expected = self.exported_evidence()?;
        Ok(HealthChallenge {
            schema: 1,
            product_version: self.plan.target_version.clone(),
            install_scope: self.plan.install_scope,
            payload_manifest_sha256: self.plan.payload_manifest_sha256,
            export_manifest_sha256: expected.manifest_sha256,
            native_content_sha256: evidence.content_sha256,
            records: expected.records,
        })
    }

    fn commit(&self, evidence: &StateEvidence) -> anyhow::Result<Phase> {
        let parent = self
            .plan
            .destination
            .parent()
            .context("migration destination parent")?;
        ensure!(
            alcomd_platform::resolve_plain_storage_directory(parent)?.1
                == self.plan.destination_parent_identity,
            "migration destination parent changed"
        );
        let stage = self.staged_data();
        if let Err(error) = fs::symlink_metadata(&self.plan.destination) {
            ensure!(
                error.kind() == io::ErrorKind::NotFound,
                "cannot check migration destination"
            );
            ensure!(
                state_evidence(&stage)? == *evidence,
                "healthy v4 data changed"
            );
            alcomd_platform::publish_path_no_replace(&stage, &self.plan.destination)?;
            alcomd_platform::sync_directory(stage.parent().context("staging parent")?)?;
        }
        ensure!(
            state_evidence(&self.plan.destination)? == *evidence && !stage.exists(),
            "cannot verify publication of migrated v4 data"
        );
        alcomd_platform::sync_directory(parent)?;
        Ok(Phase::Committed)
    }

    fn bundle(&self) -> PathBuf {
        self.journal.root().join(EXPORT_DIR).join(BUNDLE_DIR)
    }

    fn staged_data(&self) -> PathBuf {
        self.journal.root().join(STATE_DIR).join(DATA_DIR)
    }

    fn reset_stage(&self, name: &str) -> anyhow::Result<PathBuf> {
        remove_stage(&self.journal, name)?;
        let stage = self.journal.root().join(name);
        fs::create_dir(&stage)?;
        let (_, identity) = alcomd_platform::resolve_plain_storage_directory(&stage)?;
        self.journal
            .record(&format!("{name}.owner.json"), &identity)?;
        Ok(stage)
    }

    fn cleanup_stage(&self) -> anyhow::Result<()> {
        remove_stage(&self.journal, EXPORT_DIR)?;
        remove_stage(&self.journal, STATE_DIR)
    }
}

fn remove_stage(journal: &Journal, name: &str) -> anyhow::Result<()> {
    journal.check()?;
    let stage = journal.root().join(name);
    let owner = format!("{name}.owner.json");
    let identity = journal.read::<Vec<u8>>(&owner)?;
    match fs::symlink_metadata(&stage) {
        Ok(_) => {
            let (canonical, observed) = alcomd_platform::resolve_plain_storage_directory(&stage)?;
            ensure!(
                canonical == stage && canonical.parent() == Some(journal.root()),
                "unexpected migration staging directory"
            );
            if let Some(identity) = &identity {
                ensure!(
                    *identity == observed,
                    "migration staging directory was replaced"
                );
                fs::remove_dir_all(canonical)?;
            } else {
                // After a crash before the ownership record, only an empty directory can be removed.
                fs::remove_dir(canonical)?;
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).context("cannot inspect migration staging"),
    }
    if identity.is_some() {
        fs::remove_file(journal.root().join(owner))?;
    }
    alcomd_platform::sync_directory(journal.root())?;
    Ok(())
}

fn rollback(journal: &Journal, plan: &Plan) -> anyhow::Result<()> {
    ensure_quiescent_stage(plan)?;
    remove_transferred_credentials(journal, plan)?;
    remove_stage(journal, EXPORT_DIR)?;
    remove_stage(journal, STATE_DIR)?;
    journal.record(&phase_file(Phase::RolledBack.index()), &Phase::RolledBack)
}

fn remove_transferred_credentials(journal: &Journal, plan: &Plan) -> anyhow::Result<()> {
    let Some(id) = plan.credential_transfer_id else {
        return Ok(());
    };
    let Some(identity) = journal.read::<Vec<u8>>(&format!("{EXPORT_DIR}.owner.json"))? else {
        return Ok(());
    };
    let export = journal.root().join(EXPORT_DIR);
    let bundle = export.join(BUNDLE_DIR);
    if !bundle.try_exists()? {
        return Ok(());
    }
    ensure!(
        alcomd_platform::resolve_plain_storage_directory(&export)?.1 == identity,
        "migration export staging changed"
    );
    validate_bundle(&bundle)?;
    for reference in staged_credential_references(&bundle, id)? {
        alcomd_platform::delete_credential(&reference.to_string())
            .context("cannot discard a staged migration credential")?;
    }
    Ok(())
}

fn staged_credential_references(bundle: &Path, id: Uuid) -> anyhow::Result<Vec<Uuid>> {
    let bytes = read_limited(&bundle.join("state.json"), NATIVE_INPUT_LIMIT)?;
    let state: NativeState =
        serde_json::from_slice(&bytes).context("invalid native migration input")?;
    state
        .repositories
        .iter()
        .filter_map(|repository| repository.credential_ref.as_deref())
        .enumerate()
        .map(|(index, reference)| {
            let reference =
                Uuid::parse_str(reference).context("invalid migrated credential reference")?;
            ensure!(
                reference == migration_credential_reference(id, u32::try_from(index)?),
                "migrated credential reference does not belong to this transaction"
            );
            Ok(reference)
        })
        .collect()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BundleManifest {
    schema: u32,
    source_version: String,
    created_at: String,
    records: BTreeMap<String, u64>,
    files: Vec<BundleFile>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BundleFile {
    path: String,
    size: u64,
    sha256: String,
}

fn validate_bundle(path: &Path) -> anyhow::Result<ExportEvidence> {
    let (root, identity) = alcomd_platform::resolve_plain_storage_directory(path)?;
    let bytes = read_limited(&root.join("manifest.json"), BUNDLE_MANIFEST_LIMIT)?;
    let manifest: BundleManifest =
        serde_json::from_slice(&bytes).context("invalid v3 export manifest")?;
    ensure!(
        manifest.schema == 1
            && manifest.source_version == SOURCE_VERSION
            && !manifest.created_at.is_empty(),
        "migration supports the published 3.4.0 source only"
    );
    ensure!(
        manifest.records.len() <= 32 && manifest.files.len() <= 4096,
        "v3 export manifest exceeds its limits"
    );
    let mut files = BTreeSet::new();
    for entry in manifest.files {
        ensure!(
            !entry.path.contains(['\\', ':'])
                && !entry.path.is_empty()
                && entry.path.len() <= 1024
                && entry
                    .path
                    .split('/')
                    .all(|name| !name.is_empty() && !matches!(name, "." | ".."))
                && files.insert(entry.path.clone())
                && entry.size <= BUNDLE_FILE_LIMIT,
            "invalid or duplicate v3 export member"
        );
        let relative = Path::new(&entry.path);
        ensure!(
            relative
                .components()
                .all(|part| matches!(part, Component::Normal(_))),
            "unsafe v3 export member"
        );
        let file_path = root.join(relative);
        let parent = file_path.parent().context("export member has no parent")?;
        let canonical = alcomd_platform::resolve_plain_storage_directory(parent)?.0;
        ensure!(
            canonical.starts_with(&root),
            "v3 export member escapes staging"
        );
        let (file, _) = alcomd_platform::open_plain_file(&file_path)?;
        ensure!(
            hash_exact(file, entry.size)? == decode_hex(&entry.sha256)?,
            "v3 export member digest mismatch"
        );
    }
    ensure!(
        files.contains("state.json"),
        "v3 export does not include normalized native state"
    );
    ensure!(
        alcomd_platform::resolve_plain_storage_directory(&root)?.1 == identity,
        "v3 export directory changed"
    );
    Ok(ExportEvidence {
        manifest_sha256: Sha256::digest(&bytes).into(),
        records: manifest.records,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NativeHealth {
    schema: u32,
    data_schema: u32,
    config_schema: u32,
    records: BTreeMap<String, u64>,
}

/// Safe evidence that the platform's real acceptance report must identify.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HealthChallenge {
    schema: u32,
    product_version: String,
    install_scope: InstallScope,
    payload_manifest_sha256: [u8; 32],
    export_manifest_sha256: [u8; 32],
    native_content_sha256: [u8; 32],
    records: BTreeMap<String, u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HealthProof {
    schema: u32,
    product_version: String,
    install_scope: InstallScope,
    payload_manifest_sha256: [u8; 32],
    test_evidence_sha256: [u8; 32],
    core_rpc: bool,
    cli: bool,
    gui: bool,
    mcp: bool,
    first_party_extensions: bool,
}

impl HealthProof {
    fn matches(&self, expected: &HealthChallenge) -> bool {
        self.schema == expected.schema
            && self.product_version == expected.product_version
            && self.install_scope == expected.install_scope
            && self.payload_manifest_sha256 == expected.payload_manifest_sha256
            && self.test_evidence_sha256 != [0; 32]
            && self.core_rpc
            && self.cli
            && self.gui
            && self.mcp
            && self.first_party_extensions
    }
}

fn check_platform_health(challenge: &HealthChallenge, report: &HealthReport) -> anyhow::Result<()> {
    let (report_path, _) = isolated_child(&report.report)?;
    let (signature_path, _) = isolated_child(&report.signature)?;
    let report_bytes = read_limited(&report_path, OUTPUT_LIMIT)?;
    let signature_bytes = read_limited(&signature_path, 130)?;
    alcomd_updater::verify_health_report(&report_bytes, &signature_bytes)?;
    let proof: HealthProof =
        serde_json::from_slice(&report_bytes).context("invalid platform health report")?;
    ensure!(
        proof.matches(challenge),
        "platform health or reconciliation evidence is incomplete"
    );
    Ok(())
}

fn state_evidence(path: &Path) -> anyhow::Result<StateEvidence> {
    let (root, directory_identity) = alcomd_platform::resolve_plain_storage_directory(path)?;
    let state = file_evidence(&root.join("state.db"), BUNDLE_FILE_LIMIT)?;
    let settings = file_evidence(&root.join("config/settings.toml"), BUNDLE_MANIFEST_LIMIT)?;
    ensure!(
        state.size > 0 && settings.size > 0,
        "native staged state is incomplete"
    );
    for companion in ["state.db-wal", "state.db-shm", "state.db-journal"] {
        ensure_absent(&root.join(companion))
            .context("native health check left the database open or uncheckpointed")?;
    }
    let mut content = Sha256::new();
    hash_state_tree(&root, &root, 0, &mut 0, &mut content)?;
    Ok(StateEvidence {
        directory_identity,
        state,
        settings,
        content_sha256: content.finalize().into(),
    })
}

fn hash_state_tree(
    root: &Path,
    directory: &Path,
    depth: usize,
    entries: &mut usize,
    digest: &mut Sha256,
) -> anyhow::Result<()> {
    ensure!(
        depth <= 32,
        "native staged state exceeds the verification depth limit"
    );
    alcomd_platform::resolve_plain_storage_directory(directory)?;
    let mut children = fs::read_dir(directory)?
        .take(65_537)
        .collect::<io::Result<Vec<_>>>()?;
    children.sort_by_key(fs::DirEntry::file_name);
    for child in children {
        *entries += 1;
        ensure!(
            *entries <= 65_536,
            "native staged state exceeds the verification entry limit"
        );
        let path = child.path();
        let relative = path
            .strip_prefix(root)?
            .to_str()
            .context("invalid native state path")?;
        digest.update(u64::try_from(relative.len())?.to_le_bytes());
        digest.update(relative.as_bytes());
        if child.file_type()?.is_dir() {
            digest.update([0]);
            hash_state_tree(root, &path, depth + 1, entries, digest)?;
        } else {
            let file = file_evidence(&path, BUNDLE_FILE_LIMIT)?;
            digest.update([1]);
            digest.update(file.size.to_le_bytes());
            digest.update(file.sha256);
        }
    }
    Ok(())
}

fn encode_digest(digest: &[u8; 32]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn file_evidence(path: &Path, limit: u64) -> anyhow::Result<FileEvidence> {
    let parent = path.parent().context("migration file has no parent")?;
    alcomd_platform::resolve_plain_storage_directory(parent)?;
    let (file, identity) = alcomd_platform::open_plain_file(path)?;
    let size = file.metadata()?.len();
    ensure!(size <= limit, "migration file exceeds its limit");
    let sha256 = hash_exact(file, size)?;
    ensure!(
        alcomd_platform::file_identity_key(path)? == identity,
        "migration file changed"
    );
    Ok(FileEvidence {
        identity,
        size,
        sha256,
    })
}

fn ensure_absent(path: &Path) -> anyhow::Result<()> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).context("cannot inspect migration destination"),
        Ok(_) => anyhow::bail!("migration destination already exists"),
    }
}

fn run_command(executable: &Path, cwd: &Path, args: &[&OsStr]) -> anyhow::Result<Vec<u8>> {
    run_command_with_exit_code(executable, cwd, args, 0)
}

fn run_command_with_exit_code(
    executable: &Path,
    cwd: &Path,
    args: &[&OsStr],
    expected_exit_code: i32,
) -> anyhow::Result<Vec<u8>> {
    let mut process = Command::new(executable)
        .current_dir(cwd)
        .args(args)
        .env_remove("ALCOMD_MCP_CLIENT_CREDENTIAL")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("cannot start migration subprocess")?;
    let output = process
        .stdout
        .take()
        .context("migration subprocess has no output pipe")?;
    let (sender, receiver) = mpsc::sync_channel(1);
    let _reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = output
            .take(OUTPUT_LIMIT + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes);
        let _ = sender.send(result);
    });
    let started = Instant::now();
    let status = loop {
        match process.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < PHASE_TIMEOUT => {
                std::thread::sleep(Duration::from_millis(25))
            }
            result => {
                let _ = process.kill();
                let _ = process.wait();
                result.context("migration subprocess failed")?;
                anyhow::bail!("migration subprocess timed out");
            }
        }
    };
    ensure!(
        status.code() == Some(expected_exit_code),
        "migration subprocess did not complete successfully"
    );
    let output = receiver
        .recv_timeout(Duration::from_secs(1))
        .context("migration subprocess output did not close")??;
    ensure!(
        u64::try_from(output.len())? <= OUTPUT_LIMIT,
        "migration subprocess produced too much output"
    );
    Ok(output)
}

#[cfg(test)]
mod tests;
