mod client_requests;
use clap::{Parser, Subcommand, ValueEnum};
use std::io::{self, IsTerminal, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;
mod extension_commands;
mod m8_oauth;
mod package_catalog;
mod unity_migration;

#[derive(Debug)]
enum CliError {
    Client(alcomd_client::ClientError),
    ConfirmationRequired,
    ExtensionCommandFailed {
        code: String,
        core_effect: Option<serde_json::Value>,
        replayed: bool,
    },
    OperationFailed(Box<alcomd_protocol::Operation>),
    OperationDetached(String),
    LocalIo(std::io::Error),
    Usage(&'static str),
    CredentialOutcomeUnknown,
    OAuthCredentialOutcomeUnknown,
}

impl From<alcomd_client::ClientError> for CliError {
    fn from(error: alcomd_client::ClientError) -> Self {
        Self::Client(error)
    }
}

impl std::fmt::Display for CliError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Client(error) => error.fmt(formatter),
            Self::ConfirmationRequired => formatter.write_str("confirmation is required"),
            Self::ExtensionCommandFailed { code, .. } => write!(formatter, "extension command failed: {code}"),
            Self::OperationFailed(_) => formatter.write_str("operation did not succeed"),
            Self::OperationDetached(_) => formatter.write_str("operation follow was interrupted"),
            Self::LocalIo(_) => formatter.write_str("local CLI I/O failed"),
            Self::Usage(message) => formatter.write_str(message),
            Self::CredentialOutcomeUnknown => formatter.write_str("credential issuance outcome is unknown; check the client's revision before explicitly rotating again; the plaintext cannot be recovered; no retry was made"),
            Self::OAuthCredentialOutcomeUnknown => formatter.write_str("OAuth registration update outcome is unknown; list registrations and inspect its revision before explicit rotation; the one-time client secret cannot be recovered; no retry was made"),
        }
    }
}

impl std::error::Error for CliError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Client(error) => Some(error),
            Self::LocalIo(error) => Some(error),
            Self::ConfirmationRequired
            | Self::ExtensionCommandFailed { .. }
            | Self::OperationFailed(_)
            | Self::OperationDetached(_)
            | Self::Usage(_)
            | Self::CredentialOutcomeUnknown
            | Self::OAuthCredentialOutcomeUnknown => None,
        }
    }
}

/// Command-line client for ALCOMD.
#[derive(Debug, Parser)]
#[command(name = "alcomd-cli", version, about)]
struct Arguments {
    /// Emit machine-readable JSON.
    #[arg(long, global = true, conflicts_with = "ndjson")]
    json: bool,

    /// Emit newline-delimited machine-readable JSON.
    #[arg(long, global = true, conflicts_with = "json")]
    ndjson: bool,

    /// Suppress non-error human diagnostics and progress.
    #[arg(long, global = true)]
    quiet: bool,

    /// Confirm a high-impact operation without prompting.
    #[arg(long, global = true)]
    yes: bool,

    /// Create and return an immutable Plan without applying it.
    #[arg(long, global = true)]
    dry_run: bool,

    /// Return after an Operation is accepted instead of following it.
    #[arg(long, global = true)]
    no_wait: bool,

    /// Do not start the daemon when its endpoint is absent.
    #[arg(long, global = true)]
    no_start_daemon: bool,

    /// Override the private Unix runtime directory for isolated testing.
    #[arg(long, global = true, hide = true)]
    runtime_dir: Option<PathBuf>,

    /// Override the daemon data directory for isolated testing.
    #[arg(long, global = true, hide = true)]
    data_dir: Option<PathBuf>,

    /// Override the daemon executable for isolated testing.
    #[arg(long, global = true, hide = true)]
    daemon_path: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, clap::Args)]
#[group(multiple = false)]
struct PackageSourceOptions {
    /// Restrict the selected version to this repository.
    #[arg(long)]
    repository_id: Option<String>,
    /// Restrict the selected version to this enrolled user package.
    #[arg(long)]
    user_package_id: Option<String>,
}

impl PackageSourceOptions {
    fn selector(self) -> Option<alcomd_protocol::PackageSourceSelector> {
        use alcomd_protocol::PackageSourceSelector;
        self.repository_id
            .map(|repository_id| PackageSourceSelector::Repository { repository_id })
            .or_else(|| {
                self.user_package_id
                    .map(|user_package_id| PackageSourceSelector::UserPackage { user_package_id })
            })
    }
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Request an action on the Core machine's interactive desktop.
    Desktop {
        #[command(subcommand)]
        command: DesktopCommand,
    },
    /// Inspect installed programs and their current Core connections.
    Client {
        #[command(subcommand)]
        command: NativeClientCommand,
    },
    /// Discover and run commands declared by installed extensions.
    Extension {
        #[command(subcommand)]
        command: extension_commands::Command,
    },
    /// Read the exact plan retained by Core before applying it.
    Plan {
        #[command(subcommand)]
        command: PlanCommand,
    },
    /// Manage authenticated MCP connections.
    Mcp {
        #[command(subcommand)]
        command: McpCommand,
    },
    /// Inspect or control core components.
    System {
        #[command(subcommand)]
        command: SystemCommand,
    },
    /// Read or resume Core business events.
    Events {
        #[command(subcommand)]
        command: EventsCommand,
    },
    /// Read Core operation logs with the selected filters.
    Logs {
        #[command(subcommand)]
        command: LogsCommand,
    },
    /// Check the integrity of the Core state.
    State {
        #[command(subcommand)]
        command: StateCommand,
    },
    /// Read Core settings or update the selected fields.
    Settings {
        #[command(subcommand)]
        command: SettingsCommand,
    },
    /// Inspect, follow, or cooperatively cancel daemon Operations.
    #[command(visible_alias = "operations")]
    Operation {
        #[command(subcommand)]
        command: OperationCommand,
    },
    /// Inspect and manage only the ALCOMD project registry.
    #[command(visible_alias = "projects")]
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
    /// List, inspect, save, edit, export or remove project templates.
    Template {
        #[command(subcommand)]
        command: TemplateCommand,
    },
    /// Inspect and manage normalized VPM repository metadata.
    #[command(visible_alias = "repo")]
    Repository {
        #[command(subcommand)]
        command: RepositoryCommand,
    },
    /// Plan and apply the minimal M4 VPM package transaction slice.
    #[command(visible_alias = "packages")]
    Package {
        #[command(subcommand)]
        command: PackageCommand,
    },
    /// Inspect and manage Unity Editor installations and project launches.
    Unity {
        #[command(subcommand)]
        command: UnityCommand,
    },
    /// List, inspect, and create managed native Backup archives.
    Backup {
        #[command(subcommand)]
        command: BackupCommand,
    },
    /// Generate static shell completion without connecting to the daemon.
    Completion { shell: CompletionShell },
}

#[derive(Debug, Subcommand)]
enum PlanCommand {
    Get {
        #[arg(value_parser = ["package", "project_copy", "project_delete_directory", "project_unity_migration", "extension_install", "extension_uninstall", "repositoryRegistration"])]
        kind: String,
        plan_id: String,
    },
}

#[derive(Debug, Subcommand)]
enum EventsCommand {
    List {
        #[arg(long, default_value_t = 0)]
        after_sequence: u64,
        #[arg(long)]
        limit: Option<u32>,
    },
    /// Wait for the next batch and return its full continuation cursor.
    Subscribe {
        #[arg(long,value_parser=parse_cursor::<alcomd_protocol::EventsCursor>)]
        cursor: Option<alcomd_protocol::EventsCursor>,
        #[arg(long)]
        limit: Option<u32>,
    },
}

#[derive(Debug, Subcommand)]
enum LogsCommand {
    Read {
        #[arg(long)]
        before_sequence: Option<u64>,
        #[arg(long)]
        after_sequence: Option<u64>,
        #[arg(long)]
        limit: Option<u32>,
        #[arg(long,value_parser=parse_cursor::<alcomd_protocol::LogFilter>)]
        filter: Option<alcomd_protocol::LogFilter>,
    },
}

#[derive(Debug, Subcommand)]
enum StateCommand {
    Check {
        #[arg(long)]
        idempotency_key: String,
    },
}

#[derive(Debug, Subcommand)]
enum SystemCommand {
    /// Query the running per-user daemon.
    Status,
}

#[derive(Debug, Subcommand)]
enum DesktopCommand {
    FocusProjectUnity {
        project_id: String,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long)]
        idempotency_key: String,
    },
    OpenProjectDirectory {
        project_id: String,
        #[arg(long)]
        idempotency_key: String,
    },
    OpenUnityHubDownload {
        #[arg(long)]
        idempotency_key: String,
    },
}

#[derive(Debug, Subcommand)]
enum NativeClientCommand {
    /// List installed programs including those that have never connected.
    List {
        #[arg(long)]
        cursor: Option<String>,
        #[arg(long)]
        limit: Option<u32>,
    },
    /// Show the live instances and connections of one program.
    Instances {
        program_key: String,
        #[arg(long)]
        cursor: Option<String>,
        #[arg(long)]
        limit: Option<u32>,
    },
    /// Disconnect the indicated connection or the current connections of an instance.
    Disconnect {
        #[arg(value_enum)]
        kind: DisconnectKind,
        id: String,
        #[arg(long)]
        core_instance_id: String,
        #[arg(long)]
        idempotency_key: String,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum DisconnectKind {
    Connection,
    Instance,
}

#[derive(Debug, Subcommand)]
enum SettingsCommand {
    /// Read the current settings and their revision.
    Get,
    /// Apply a partial settings update at the revision returned by settings get.
    Update {
        #[arg(long)]
        idempotency_key: String,
        #[arg(long)]
        expected_revision: u64,
        /// JSON object containing the fields to change.
        #[arg(long, value_parser = parse_settings_update)]
        update: Box<alcomd_protocol::SettingsUpdate>,
    },
}

fn parse_settings_update(value: &str) -> Result<Box<alcomd_protocol::SettingsUpdate>, String> {
    serde_json::from_str(value).map_err(|error| format!("invalid settings update: {error}"))
}

#[derive(Debug, Subcommand)]
enum McpCommand {
    Transport {
        #[arg(long)]
        client_id: Option<String>,
        #[arg(long)]
        limit: Option<u32>,
    },
    Runtime {
        #[arg(long)]
        client_id: Option<String>,
        #[arg(long)]
        limit: Option<u32>,
    },
    Oauth {
        #[command(subcommand)]
        command: m8_oauth::OAuthCommand,
    },
    #[command(visible_alias = "clients")]
    Client {
        #[command(subcommand)]
        command: McpClientCommand,
    },
}

#[derive(Debug, Subcommand)]
enum McpClientCommand {
    /// Register an AI client with Core.
    Create {
        display_name: String,
        #[arg(long)]
        idempotency_key: String,
    },
    /// Revoke an AI client's access.
    Revoke {
        client_id: String,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long)]
        idempotency_key: String,
    },
    /// Submit, inspect or decide a client management request.
    Request {
        #[command(subcommand)]
        command: client_requests::Command,
    },
    /// List non-secret client records.
    List {
        #[arg(long)]
        cursor: Option<String>,
        #[arg(long)]
        limit: Option<u32>,
    },
    /// Issue once or rotate a credential. The old credential becomes invalid immediately.
    IssueCredential {
        client_id: String,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long)]
        idempotency_key: String,
        /// Explicitly permit one-time secret output to a pipe or JSON/NDJSON.
        #[arg(long)]
        secret_stdout: bool,
    },
}

#[derive(Debug, Subcommand)]
enum OperationCommand {
    List {
        #[arg(long)]
        active_only: bool,
        /// JSON nextCursor from the preceding operation list.
        #[arg(long, value_parser = parse_cursor::<alcomd_protocol::OperationsListCursor>)]
        cursor: Option<alcomd_protocol::OperationsListCursor>,
        #[arg(long)]
        limit: Option<u32>,
    },
    #[command(visible_alias = "info")]
    Get {
        operation_id: String,
    },
    Follow {
        operation_id: String,
    },
    Cancel {
        operation_id: String,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long)]
        idempotency_key: String,
    },
}

#[derive(Debug, Subcommand)]
enum TemplateCommand {
    List {
        #[arg(long)]
        cursor: Option<String>,
        #[arg(long)]
        limit: Option<u32>,
    },
    Get {
        id: String,
        #[arg(long)]
        cursor: Option<String>,
        #[arg(long)]
        limit: Option<usize>,
    },
    Edit {
        id: String,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long)]
        changes: PathBuf,
        #[arg(long)]
        idempotency_key: String,
    },
    Save {
        #[arg(long)]
        source: PathBuf,
        #[arg(long)]
        expected_revision: Option<u64>,
        #[arg(long)]
        idempotency_key: String,
    },
    Export {
        id: String,
        destination: PathBuf,
        #[arg(long)]
        revision: u64,
        #[arg(long)]
        idempotency_key: String,
    },
    Remove {
        id: String,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long)]
        idempotency_key: String,
    },
}

fn read_template_json(path: PathBuf) -> Result<serde_json::Value, CliError> {
    use std::io::Read as _;
    const LIMIT: usize = 48 * 1024;
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .and_then(|file| file.take((LIMIT + 1) as u64).read_to_end(&mut bytes))
        .map_err(alcomd_client::ClientError::Transport)?;
    if bytes.len() > LIMIT {
        return Err(CliError::Usage("Template input exceeds 48 KiB"));
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| CliError::Usage("Template input must be a valid JSON object"))
}

#[derive(Debug, Subcommand)]
enum ProjectCommand {
    /// Apply the previously reviewed copy plan using its original revision.
    ApplyCopy {
        plan_id: String,
        #[arg(long)]
        expected_revision: Option<u64>,
        #[arg(long)]
        idempotency_key: String,
    },
    /// Apply the previously reviewed directory deletion plan.
    ApplyDeleteDirectory {
        plan_id: String,
        #[arg(long)]
        expected_revision: Option<u64>,
        #[arg(long)]
        idempotency_key: String,
    },
    /// Apply the previously reviewed Unity migration plan.
    ApplyUnityMigration {
        plan_id: String,
        /// Explicitly approve a Unity 6 migration at the reviewed project's original path.
        #[arg(long, conflicts_with = "copy_operation_id")]
        in_place: bool,
        /// ID of the completed copy operation that created the reviewed target.
        #[arg(long)]
        copy_operation_id: Option<String>,
        #[arg(long)]
        idempotency_key: String,
    },
    Copy {
        project_id: String,
        /// Project name under the configured project directory, or an explicit parent followed by NAME.
        #[arg(value_name = "NAME_OR_PARENT")]
        target_parent: PathBuf,
        #[arg(value_name = "NAME")]
        target_leaf: Option<String>,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long)]
        plan_idempotency_key: String,
        #[arg(long, required_unless_present = "dry_run")]
        idempotency_key: Option<String>,
    },
    Create {
        /// Project name under the configured project directory, or an explicit parent followed by NAME.
        #[arg(value_name = "NAME_OR_PARENT")]
        target_parent: PathBuf,
        #[arg(value_name = "NAME")]
        name: Option<String>,
        #[arg(long)]
        unity_version: String,
        #[arg(long, requires = "template_revision")]
        template_id: Option<String>,
        #[arg(long, requires = "template_id")]
        template_revision: Option<u64>,
        #[arg(long, conflicts_with = "dry_run")]
        idempotency_key: String,
    },
    DeleteDirectory {
        #[arg(long)]
        project_id: String,
        #[arg(long)]
        plan_idempotency_key: String,
        #[arg(long, required_unless_present = "dry_run")]
        idempotency_key: Option<String>,
    },
    MigrateUnity {
        /// Upgrade the original project. Unity 6 migrations otherwise use a sibling copy.
        #[arg(long, conflicts_with = "copy_name")]
        in_place: bool,
        /// Folder name for the Unity 6 migration copy.
        #[arg(long)]
        copy_name: Option<String>,
        project_id: String,
        target_installation_id: String,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long)]
        plan_idempotency_key: String,
        #[arg(long, required_unless_present = "dry_run")]
        idempotency_key: Option<String>,
    },
    Inspect {
        path: PathBuf,
        #[arg(long)]
        search_parents: bool,
    },
    List {
        /// JSON nextCursor from the preceding page.
        #[arg(long, value_parser = parse_cursor::<alcomd_protocol::ProjectRegistryCursor>)]
        cursor: Option<alcomd_protocol::ProjectRegistryCursor>,
        #[arg(long)]
        limit: Option<u32>,
        /// JSON list view with sort, descending and optional search; reuse it for each page.
        #[arg(long, value_parser = parse_project_view)]
        view: Option<alcomd_protocol::ProjectListView>,
        /// viewSequence from the first page of the same view.
        #[arg(long, requires = "view")]
        expected_view_sequence: Option<u64>,
    },
    #[command(visible_alias = "info")]
    Get { project_id: String },
    #[command(visible_alias = "add")]
    Register {
        path: PathBuf,
        /// Resolve the supplied relative path beneath Core's configured project directory.
        #[arg(long)]
        relative_to_default: bool,
        #[arg(long)]
        idempotency_key: String,
    },
    Refresh {
        project_id: String,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long)]
        idempotency_key: String,
    },
    SetFavorite {
        project_id: String,
        #[arg(action = clap::ArgAction::Set)]
        favorite: bool,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long)]
        idempotency_key: String,
    },
    #[command(visible_alias = "remove")]
    Unregister {
        project_id: String,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long)]
        idempotency_key: String,
    },
}

#[derive(Debug, Subcommand)]
enum RepositoryCommand {
    /// Move a repository before another, or to the end when no predecessor is supplied.
    Move {
        repository_id: String,
        #[arg(long)]
        before_repository_id: Option<String>,
        #[arg(long)]
        expected_order_fingerprint: String,
        #[arg(long)]
        idempotency_key: String,
    },
    /// Set a repository label, or clear its override with --clear.
    SetDisplayName {
        repository_id: String,
        #[arg(long, required_unless_present = "clear", conflicts_with = "clear")]
        display_name: Option<String>,
        #[arg(long)]
        clear: bool,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long)]
        idempotency_key: String,
    },
    RefreshAll {
        #[arg(long)]
        idempotency_key: String,
    },
    InspectListFile {
        path: String,
    },
    ImportListFile {
        path: String,
        #[arg(long)]
        expected_fingerprint: String,
        #[arg(long)]
        idempotency_key: String,
        #[arg(long,num_args=0..=1,default_missing_value="true",require_equals=true)]
        overwrite_existing: Option<bool>,
    },
    /// Stop admitting new registrations for the selected import.
    CancelListRegistration {
        #[arg(long)]
        idempotency_key: String,
    },
    PreviewListExport {
        #[arg(long, value_enum, default_value = "native")]
        format: RepositoryExportFormat,
    },
    /// Write the reviewed repository list to a new file on the Core machine.
    ExportListFile {
        target_path: String,
        #[arg(long)]
        expected_fingerprint: String,
        #[arg(long, value_enum, default_value = "native")]
        format: RepositoryExportFormat,
        #[arg(long)]
        idempotency_key: String,
    },
    Inspect {
        source: String,
        #[arg(long)]
        credentials_stdin: bool,
    },
    List {
        /// JSON nextCursor from the preceding page.
        #[arg(long, value_parser = parse_cursor::<alcomd_protocol::RegistryCursor>)]
        cursor: Option<alcomd_protocol::RegistryCursor>,
        #[arg(long)]
        limit: Option<u32>,
    },
    #[command(visible_alias = "info")]
    Get {
        repository_id: String,
    },
    Packages {
        #[arg(long, value_parser = parse_cursor::<alcomd_protocol::PackageCursor>)]
        cursor: Option<alcomd_protocol::PackageCursor>,
        repository_id: String,
        #[arg(long)]
        limit: Option<u32>,
    },
    #[command(visible_alias = "add")]
    Register {
        /// ID returned by repository inspect.
        preview_id: String,
        #[arg(long)]
        idempotency_key: String,
    },
    SetCredentials {
        repository_id: String,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long)]
        idempotency_key: String,
        #[arg(
            long,
            required_unless_present = "credentials_stdin",
            conflicts_with = "credentials_stdin"
        )]
        clear: bool,
        #[arg(long)]
        credentials_stdin: bool,
    },
    Refresh {
        repository_id: String,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long)]
        idempotency_key: String,
    },
    #[command(visible_alias = "remove")]
    Unregister {
        repository_id: String,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long)]
        idempotency_key: String,
    },
}

#[derive(Debug, Subcommand)]
enum PackageCommand {
    /// Read candidates and Core recommendations for a project.
    Candidates(package_catalog::Candidates),
    /// Preview or apply a mixed set of package intents as a single plan.
    Bulk(package_catalog::Bulk),
    /// Reinstall selected or all packages at their locked versions.
    Reinstall(package_catalog::Reinstall),
    /// Manage local packages available to project plans.
    UserPackage {
        #[command(subcommand)]
        command: package_catalog::UserPackage,
    },
    /// Read declared, locked, and locally observed packages in a project.
    Inventory {
        project_id: String,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long)]
        cursor: Option<String>,
        #[arg(long)]
        limit: Option<usize>,
    },
    #[command(name = "install", visible_alias = "i")]
    Install {
        project_id: String,
        package_id: String,
        #[arg(long)]
        expected_revision: u64,
        /// Omit to let Core select the recommended compatible version.
        #[arg(long)]
        version_range: Option<String>,
        /// Allow this exact package version despite its Unity requirement (requires --version-range "=X.Y.Z").
        #[arg(long, requires = "version_range")]
        allow_unity_incompatible: bool,
        #[command(flatten)]
        source: PackageSourceOptions,
        #[arg(long, action = clap::ArgAction::Set, num_args = 0..=1, require_equals = true, default_missing_value = "true")]
        include_prerelease: Option<bool>,
        #[arg(long, required_unless_present = "dry_run")]
        idempotency_key: Option<String>,
    },
    #[command(name = "remove", visible_alias = "rm")]
    Remove {
        project_id: String,
        package_id: String,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long, required_unless_present = "dry_run")]
        idempotency_key: Option<String>,
    },
    #[command(name = "upgrade")]
    Upgrade {
        project_id: String,
        package_id: String,
        #[arg(long)]
        expected_revision: u64,
        /// Omit to let Core select the recommended compatible version.
        #[arg(long)]
        version_range: Option<String>,
        /// Allow this exact package version despite its Unity requirement (requires --version-range "=X.Y.Z").
        #[arg(long, requires = "version_range")]
        allow_unity_incompatible: bool,
        #[command(flatten)]
        source: PackageSourceOptions,
        #[arg(long, action = clap::ArgAction::Set, num_args = 0..=1, require_equals = true, default_missing_value = "true")]
        include_prerelease: Option<bool>,
        #[arg(long, required_unless_present = "dry_run")]
        idempotency_key: Option<String>,
    },
    #[command(name = "downgrade")]
    Downgrade {
        project_id: String,
        package_id: String,
        version: String,
        /// Allow this exact package version despite its Unity requirement.
        #[arg(long)]
        allow_unity_incompatible: bool,
        #[arg(long)]
        expected_revision: u64,
        #[command(flatten)]
        source: PackageSourceOptions,
        #[arg(long, required_unless_present = "dry_run")]
        idempotency_key: Option<String>,
    },
    #[command(name = "resolve")]
    Resolve {
        project_id: String,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long, action = clap::ArgAction::Set, num_args = 0..=1, require_equals = true, default_missing_value = "true")]
        include_prerelease: Option<bool>,
        #[arg(long, required_unless_present = "dry_run")]
        idempotency_key: Option<String>,
    },
    ApplyPlan {
        plan_id: String,
        #[arg(long)]
        expected_revision: Option<u64>,
        #[arg(long)]
        idempotency_key: String,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum RepositoryExportFormat {
    Native,
    VccTxt,
}
impl From<RepositoryExportFormat> for alcomd_protocol::RepositoryListFormat {
    fn from(value: RepositoryExportFormat) -> Self {
        match value {
            RepositoryExportFormat::Native => Self::Native,
            RepositoryExportFormat::VccTxt => Self::Legacy,
        }
    }
}

#[derive(Debug, Subcommand)]
enum UnityCommand {
    /// Open the Unity Hub observed at the specified settings revision.
    HubOpen {
        #[arg(long)]
        expected_revision: u64,
        /// Pass the target token from settings get. Omit for the observed missing target.
        #[arg(long)]
        expected_target_token: Option<String>,
        #[arg(long)]
        idempotency_key: String,
    },
    List {
        #[arg(long,num_args=0..=1,default_missing_value="true",require_equals=true)]
        include_version_options: Option<bool>,
        #[arg(long)]
        cursor: Option<String>,
        #[arg(long)]
        limit: Option<u32>,
    },
    #[command(visible_alias = "info")]
    Get {
        installation_id: String,
    },
    Refresh {
        #[arg(long)]
        idempotency_key: String,
    },
    #[command(visible_alias = "add")]
    Register {
        executable_path: PathBuf,
        #[arg(long)]
        idempotency_key: String,
    },
    Remove {
        installation_id: String,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long)]
        idempotency_key: String,
    },
    ProjectLaunchConfigGet {
        project_id: String,
    },
    ProjectLaunchConfigSet {
        project_id: String,
        #[arg(long = "argument", allow_hyphen_values = true)]
        arguments: Vec<String>,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long)]
        idempotency_key: String,
    },
    ProjectLaunchConfigClear {
        project_id: String,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long)]
        idempotency_key: String,
    },
    WriterState {
        project_id: String,
    },
    Launch {
        project_id: String,
        installation_id: String,
        #[arg(long)]
        expected_project_revision: u64,
        #[arg(long)]
        idempotency_key: String,
    },
    LaunchOptions {
        #[arg(long,num_args=0..=1,default_missing_value="true",require_equals=true)]
        include_migration_version_options: Option<bool>,
        project_id: String,
        #[arg(long)]
        expected_project_revision: u64,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum BackupCompression {
    Store,
    Fast,
    #[value(name = "level3")]
    Level3,
    #[value(name = "level5")]
    Level5,
    #[value(name = "level7")]
    Level7,
    Maximum,
}

#[derive(Debug, Subcommand)]
enum BackupCommand {
    Create {
        project_id: String,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long, value_enum)]
        compression: Option<BackupCompression>,
        #[arg(long, num_args = 0..=1, default_missing_value = "true", require_equals = true)]
        exclude_vpm_packages: Option<bool>,
        #[arg(long, num_args = 0..=1, default_missing_value = "true", require_equals = true)]
        include_library: Option<bool>,
        #[arg(long)]
        destination: Option<PathBuf>,
        #[arg(long)]
        idempotency_key: String,
    },
    Restore {
        archive: PathBuf,
        #[arg(long, conflicts_with = "destination")]
        name: Option<String>,
        #[arg(long)]
        destination: Option<PathBuf>,
        #[arg(long)]
        idempotency_key: String,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum CompletionShell {
    Bash,
    Zsh,
    #[value(name = "powershell")]
    PowerShell,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OutputMode {
    Human,
    Json,
    Ndjson,
}

#[derive(Clone, Copy, Debug)]
struct ExecutionOptions {
    output_mode: OutputMode,
    yes: bool,
    dry_run: bool,
    no_wait: bool,
}

#[tokio::main]
async fn main() -> ExitCode {
    let arguments = Arguments::parse();
    let output_mode = if arguments.json {
        OutputMode::Json
    } else if arguments.ndjson {
        OutputMode::Ndjson
    } else {
        OutputMode::Human
    };
    let options = ExecutionOptions {
        output_mode,
        yes: arguments.yes,
        dry_run: arguments.dry_run,
        no_wait: arguments.no_wait,
    };
    let command_name = arguments.command.name();
    let credential_output = matches!(&arguments.command,Command::Mcp{command:McpCommand::Oauth{command}} if m8_oauth::secret_output(command))
        || matches!(
            &arguments.command,
            Command::Mcp {
                command: McpCommand::Client {
                    command: McpClientCommand::IssueCredential { .. }
                        | McpClientCommand::Request {
                            command: client_requests::Command::Approve { .. }
                        }
                }
            }
        );

    if let Command::Completion { shell } = &arguments.command {
        return write_completion(*shell);
    }

    let mut config =
        alcomd_client::ClientConfig::default().with_client_name(env!("CARGO_PKG_NAME"));
    if arguments.no_start_daemon {
        config = config.without_daemon_start();
    }
    if let Some(path) = arguments.runtime_dir {
        config = config.with_runtime_directory(path);
    }
    if let Some(path) = arguments.data_dir {
        config = config.with_data_directory(path);
    }
    if let Some(path) = arguments.daemon_path {
        config = config.with_daemon_path(path);
    }

    match execute(config, arguments.command, options).await {
        Ok(value) => {
            let result = if credential_output {
                print_credential_result(output_mode, command_name, &value)
            } else {
                print_result(output_mode, command_name, &value)
            };
            if credential_output {
                credential_output_exit(result)
            } else {
                output_exit(result)
            }
        }
        Err(CliError::OperationDetached(operation_id)) => {
            let result = serde_json::json!({
                "operationId": operation_id,
                "detached": true,
            });
            let _ = print_result(output_mode, command_name, &result);
            ExitCode::from(130)
        }
        Err(error) => output_error_exit(print_error(output_mode, &error)),
    }
}

fn validate_mcp_output(
    command: &Command,
    options: ExecutionOptions,
    stdout_is_terminal: bool,
) -> Result<(), CliError> {
    if let Command::Mcp {
        command: McpCommand::Oauth { command },
    } = command
    {
        return m8_oauth::validate(command, options, stdout_is_terminal);
    }
    if let Command::Mcp {
        command: McpCommand::Client { command },
    } = command
    {
        if options.dry_run || options.no_wait {
            return Err(CliError::Usage(
                "MCP client commands do not support --dry-run or --no-wait",
            ));
        }
        if let McpClientCommand::IssueCredential { secret_stdout, .. } = command
            && (!stdout_is_terminal || options.output_mode != OutputMode::Human)
            && !secret_stdout
        {
            return Err(CliError::Usage(
                "credential output to a pipe or JSON/NDJSON requires explicit --secret-stdout; no request was sent",
            ));
        }
    }
    Ok(())
}

fn parse_project_view(value: &str) -> Result<alcomd_protocol::ProjectListView, String> {
    serde_json::from_str(value).map_err(|error| format!("invalid project list view: {error}"))
}

fn parse_cursor<T: serde::de::DeserializeOwned>(value: &str) -> Result<T, String> {
    serde_json::from_str(value).map_err(|err| format!("invalid nextCursor: {err}"))
}

fn supports_preview(command: &Command) -> bool {
    if let Command::Extension { command } = command {
        return command.supports_preview();
    }
    matches!(
        command,
        Command::Project {
            command: ProjectCommand::Copy { .. }
                | ProjectCommand::DeleteDirectory { .. }
                | ProjectCommand::MigrateUnity { .. }
        } | Command::Package {
            command: PackageCommand::Install { .. }
                | PackageCommand::Bulk(_)
                | PackageCommand::Reinstall(_)
                | PackageCommand::Upgrade { .. }
                | PackageCommand::Downgrade { .. }
                | PackageCommand::Remove { .. }
                | PackageCommand::Resolve { .. }
        }
    )
}

async fn execute(
    config: alcomd_client::ClientConfig,
    command: Command,
    options: ExecutionOptions,
) -> Result<serde_json::Value, CliError> {
    if options.dry_run && !supports_preview(&command) {
        return Err(CliError::Usage("this command does not support --dry-run"));
    }
    validate_mcp_output(&command, options, io::stdout().is_terminal())?;
    if let Command::Events {
        command: EventsCommand::Subscribe { cursor, limit },
    } = command
    {
        let mut subscription = alcomd_client::EventSubscription::connect(
            config,
            alcomd_protocol::EventsSubscribeParams { cursor, limit },
        )
        .await?;
        let ready = subscription.ready().clone();
        if ready.resync_required {
            return Ok(serde_json::json!({"ready":ready}));
        }
        return Ok(
            serde_json::json!({"ready":ready,"notification":subscription.next_notification().await?}),
        );
    }
    let mut client = alcomd_client::AlcomdClient::connect(config.clone()).await?;
    let value = match command {
        Command::Desktop {command} => {
            match command {
                DesktopCommand::FocusProjectUnity{project_id,expected_revision,idempotency_key} =>
                    serde_json::to_value(client.desktop_focus_project_unity(alcomd_protocol::DesktopFocusProjectUnityParams{project_id,expected_revision,idempotency_key}).await?),
                DesktopCommand::OpenProjectDirectory{project_id,idempotency_key} =>
                    serde_json::to_value(client.desktop_open_project_directory(alcomd_protocol::DesktopOpenProjectDirectoryParams{project_id,idempotency_key}).await?),
                DesktopCommand::OpenUnityHubDownload{idempotency_key} =>
                    serde_json::to_value(client.desktop_open_unity_hub_download(alcomd_protocol::DesktopOpenUnityHubDownloadParams{idempotency_key}).await?),
            }
        }
        Command::Client { command } => match command {
            NativeClientCommand::List {cursor,limit} => serde_json::to_value(client.clients_list(alcomd_protocol::ClientsListParams {cursor,limit}).await?),
            NativeClientCommand::Instances {program_key,cursor,limit} => serde_json::to_value(client.client_instances_list(alcomd_protocol::ClientInstancesListParams {program_key,cursor,limit}).await?),
            NativeClientCommand::Disconnect {kind,id,core_instance_id,idempotency_key} => {
                if !options.yes && matches!(options.output_mode, OutputMode::Human) {
                    eprintln!("Disconnect {kind:?} {} on Core {}.", id.escape_debug(), core_instance_id.escape_debug());
                }
                confirm_high_impact(options)?;
                let target = match kind {
                    DisconnectKind::Connection => alcomd_protocol::ClientDisconnectTarget::Connection { id },
                    DisconnectKind::Instance => alcomd_protocol::ClientDisconnectTarget::Instance { id },
                };
                serde_json::to_value(client.clients_disconnect(alcomd_protocol::ClientsDisconnectParams {expected_core_instance_id:core_instance_id,target,idempotency_key}).await?)
            }
        },
        Command::Mcp {command:McpCommand::Runtime {client_id,limit}} => serde_json::to_value(client.mcp_runtime_snapshot(alcomd_protocol::McpRuntimeSnapshotParams {client_id,limit}).await?),
        Command::Mcp {command:McpCommand::Transport {client_id,limit}} => serde_json::to_value(client.mcp_transport_snapshot(alcomd_protocol::McpTransportQuery {client_id,limit}).await?),
        Command::Extension {command} => return extension_commands::execute(&mut client, command, options).await,
        Command::Plan { command: PlanCommand::Get { kind, plan_id } } => {
            let kind = serde_json::from_value(serde_json::Value::String(kind))
                .map_err(|_| CliError::Usage("unknown plan kind"))?;
            serde_json::to_value(client.get_plan(alcomd_protocol::PlansGetParams { kind, plan_id }).await?)
        }
        Command::Events {command:EventsCommand::Subscribe{..}} => unreachable!("subscription owns its dedicated connection"),
        Command::Events {command:EventsCommand::List{after_sequence,limit}} => serde_json::to_value(client.events_list(after_sequence,limit).await?),
        Command::State {command:StateCommand::Check{idempotency_key}} => serde_json::to_value(client.state_check(idempotency_key).await?),
        Command::Logs {command:LogsCommand::Read{before_sequence,after_sequence,limit,filter}} => serde_json::to_value(client.logs_read(alcomd_protocol::LogsReadParams{before_sequence,after_sequence,limit,filter}).await?),
        Command::Mcp{command:McpCommand::Oauth{command}}=>return m8_oauth::execute(&mut client,command,options).await,
        Command::Mcp { command: McpCommand::Client { command } } => match command {
            McpClientCommand::Request { command } => return client_requests::execute(&mut client,command,options).await,
            McpClientCommand::Create {display_name,idempotency_key} => {
                eprintln!("Register AI client {}.", display_name.escape_debug());
                confirm_high_impact(options)?;
                serde_json::to_value(client.mcp_client_create(alcomd_protocol::McpClientsCreateParams { display_name,idempotency_key }).await?)
            }
            McpClientCommand::Revoke {client_id,expected_revision,idempotency_key} => {
                if !options.yes {
                    let record = client_requests::lookup(&mut client,&client_id).await?;
                    eprintln!("Permanently revoke {} ({}) at revision {expected_revision}.",record.display_name.escape_debug(),record.client_id);
                }
                confirm_high_impact(options)?;
                serde_json::to_value(client.mcp_client_revoke(alcomd_protocol::McpClientMutationParams { client_id,expected_revision,idempotency_key }).await?)
            }
            McpClientCommand::List { cursor, limit } => serde_json::to_value(client.mcp_clients_list(alcomd_protocol::McpClientsListParams { cursor, limit }).await?),
            McpClientCommand::IssueCredential { client_id, expected_revision, idempotency_key, .. } => {
                if !options.yes {
                    let record = client_requests::lookup(&mut client, &client_id).await?;
                    eprintln!("Issue credential for {} ({}) at revision {expected_revision}. The previous credential becomes invalid immediately.", record.display_name.escape_debug(), record.client_id);
                }
                confirm_high_impact(options)?;
                let issued = client.mcp_client_issue_credential(alcomd_protocol::McpClientMutationParams { client_id, expected_revision, idempotency_key }).await.map_err(|error| match error {
                    alcomd_client::ClientError::Remote(ref remote) if matches!(remote.code.as_str(), "store_unavailable" | "internal_error" | "daemon_unavailable") => CliError::CredentialOutcomeUnknown,
                    alcomd_client::ClientError::Remote(_) => CliError::Client(error),
                    _ => CliError::CredentialOutcomeUnknown,
                })?;
                serde_json::to_value(issued)
            }
        },
        Command::System {
            command: SystemCommand::Status,
        } => serde_json::to_value(client.system_status().await?),
        Command::Settings { command } => match command {
            SettingsCommand::Get => serde_json::to_value(client.settings_get().await?),
            SettingsCommand::Update { expected_revision, update, idempotency_key } => {
                let result = client.settings_update(alcomd_protocol::SettingsUpdateParams {
                    idempotency_key,
                    expected_revision,
                    update: *update,
                }).await?;
                serde_json::to_value(result)
            }
        },
        Command::Operation { command } => match command {
            OperationCommand::List {
                active_only,
                cursor,
                limit,
            } => {
                serde_json::to_value(client.operations_list(cursor, limit, active_only).await?)
            }
            OperationCommand::Get { operation_id } => {
                serde_json::to_value(client.operation_get(operation_id).await?)
            }
            OperationCommand::Follow { operation_id } => serde_json::to_value(
                wait_for_operation(&mut client, operation_id, options.output_mode).await?,
            ),
            OperationCommand::Cancel {
                operation_id,
                expected_revision,
                idempotency_key,
            } => serde_json::to_value(
                client
                    .operation_cancel(operation_id, expected_revision, idempotency_key)
                    .await?,
            ),
        },
        Command::Template { command } => match command {
            TemplateCommand::List { cursor, limit } => serde_json::to_value(
                client.project_templates_list(alcomd_protocol::ProjectTemplatesListParams { cursor, limit }).await?,
            ),
            TemplateCommand::Get { id, cursor, limit } => serde_json::to_value(
                client.template_get(alcomd_protocol::ProjectTemplateGetParams { id, cursor, limit }).await?,
            ),
            TemplateCommand::Edit { id, expected_revision, changes, idempotency_key } => {
                let changes = serde_json::from_value(read_template_json(changes)?)
                    .map_err(|_| CliError::Usage("Invalid template changes"))?;
                eprintln!("Apply the selected changes to template {} at revision {expected_revision}.", id.escape_debug());
                confirm_high_impact(options)?;
                serde_json::to_value(client.template_edit(alcomd_protocol::ProjectTemplateEditParams {
                    id, expected_revision, changes, confirm: true, idempotency_key,
                }).await?)
            }
            TemplateCommand::Save { source, expected_revision, idempotency_key } => {
                let source = serde_json::from_value(read_template_json(source)?)
                    .map_err(|_| CliError::Usage("Invalid template source"))?;
                eprintln!("Save a template with the supplied definition or source.");
                confirm_high_impact(options)?;
                serde_json::to_value(client.project_templates_save(alcomd_protocol::ProjectTemplatesSaveParams {
                    source, expected_revision, idempotency_key,
                }).await?)
            }
            TemplateCommand::Export { id, destination, revision, idempotency_key } => {
                let destination = absolute_path(destination).map_err(alcomd_client::ClientError::Transport)?;
                eprintln!("Export template {} at revision {revision} to {}.", id.escape_debug(), destination.as_str().escape_debug());
                confirm_high_impact(options)?;
                serde_json::to_value(client.project_templates_export(alcomd_protocol::ProjectTemplatesExportParams {
                    id, revision, destination_file: destination, idempotency_key,
                }).await?)
            }
            TemplateCommand::Remove { id, expected_revision, idempotency_key } => {
                eprintln!("Remove template {} at revision {expected_revision}.", id.escape_debug());
                confirm_high_impact(options)?;
                serde_json::to_value(client.project_templates_remove(alcomd_protocol::ProjectTemplatesRemoveParams {
                    id, expected_revision, idempotency_key,
                }).await?)
            }
        },
        Command::Project { command } => match command {
            ProjectCommand::ApplyCopy { plan_id, expected_revision, idempotency_key } => {
                let expected_revision = confirm_frozen_plan(&mut client, alcomd_protocol::PlanKind::ProjectCopy, &plan_id, expected_revision, options).await?.ok_or(alcomd_client::ClientError::InvalidResponse)?;
                let accepted = client.project_apply_copy(alcomd_protocol::ProjectsApplyCopyParams {
                    plan_id, expected_revision, idempotency_key,
                }).await?;
                if options.no_wait { serde_json::to_value(accepted) } else {
                    serde_json::to_value(wait_for_operation(&mut client, accepted.operation_id, options.output_mode).await?)
                }
            }
            ProjectCommand::ApplyDeleteDirectory { plan_id, expected_revision, idempotency_key } => {
                let expected_revision = confirm_frozen_plan(&mut client, alcomd_protocol::PlanKind::ProjectDeleteDirectory, &plan_id, expected_revision, options).await?.ok_or(alcomd_client::ClientError::InvalidResponse)?;
                let accepted = client.project_apply_delete_directory(alcomd_protocol::ProjectsApplyDeleteDirectoryParams {
                    plan_id, expected_revision, idempotency_key,
                }).await?;
                if options.no_wait { serde_json::to_value(accepted) } else {
                    serde_json::to_value(wait_for_operation(&mut client, accepted.operation_id, options.output_mode).await?)
                }
            }
            ProjectCommand::ApplyUnityMigration { plan_id, in_place, copy_operation_id, idempotency_key } => {
                return unity_migration::apply_reviewed(&mut client, &config, plan_id, in_place,
                    copy_operation_id, idempotency_key, options).await;
            }
            ProjectCommand::Copy {
                project_id,
                target_parent,
                target_leaf,
                expected_revision,
                plan_idempotency_key,
                idempotency_key,
            } => {
                let target = project_target(target_parent, target_leaf)?;
                let plan = client
                    .project_plan_copy(alcomd_protocol::ProjectsPlanCopyParams {
                        source_project_id: project_id,
                        expected_revision,
                        target,
                        idempotency_key: plan_idempotency_key,
                    })
                    .await?;
                if options.dry_run {
                    serde_json::to_value(plan)
                } else {
                    if !options.yes && options.output_mode == OutputMode::Human {
                        show_copy_plan(&plan.plan);
                    }
                    confirm_high_impact(options)?;
                    let accepted = client
                        .project_apply_copy(alcomd_protocol::ProjectsApplyCopyParams {
                            plan_id: plan.plan.plan_id,
                            expected_revision,
                            idempotency_key: idempotency_key.ok_or(CliError::Usage("Apply requires --idempotency-key"))?,
                        })
                        .await?;
                    if options.no_wait {
                        serde_json::to_value(accepted)
                    } else {
                        serde_json::to_value(
                            wait_for_operation(
                                &mut client,
                                accepted.operation_id,
                                options.output_mode,
                            )
                            .await?,
                        )
                    }
                }
            }
            ProjectCommand::Create {
                name,
                target_parent,
                unity_version,
                template_id,
                template_revision,
                idempotency_key,
            } => {
                let target = project_target(target_parent, name)?;
                let template = template_id
                    .zip(template_revision)
                    .map(|(id, revision)| alcomd_protocol::ProjectTemplateRef { id, revision });
                let accepted = client
                    .projects_create(alcomd_protocol::ProjectsCreateParams {
                        target,
                        unity_version,
                        idempotency_key,
                        template,
                    })
                    .await?;
                if options.no_wait {
                    serde_json::to_value(accepted)
                } else {
                    serde_json::to_value(
                        wait_for_operation(&mut client, accepted.operation_id, options.output_mode)
                            .await?,
                    )
                }
            }
            ProjectCommand::DeleteDirectory { project_id, plan_idempotency_key, idempotency_key } => {
                let project = client.project_get(project_id.clone()).await?.project;
                let expected_revision = project
                    .revision
                    .ok_or(alcomd_client::ClientError::InvalidResponse)?;
                let plan = client
                    .project_plan_delete_directory(
                        alcomd_protocol::ProjectsPlanDeleteDirectoryParams {
                            project_id,
                            expected_revision,
                            idempotency_key: plan_idempotency_key,
                        },
                    )
                    .await?;
                if options.dry_run {
                    serde_json::to_value(plan)
                } else {
                    show_directory_delete(&plan.plan, options);
                    confirm_high_impact(options)?;
                    let accepted = client
                        .project_apply_delete_directory(
                            alcomd_protocol::ProjectsApplyDeleteDirectoryParams {
                                plan_id: plan.plan.plan_id,
                                expected_revision,
                                idempotency_key: idempotency_key.ok_or(CliError::Usage("Apply requires --idempotency-key"))?,
                            },
                        )
                        .await?;
                    if options.no_wait {
                        serde_json::to_value(accepted)
                    } else {
                        serde_json::to_value(
                            wait_for_operation(
                                &mut client,
                                accepted.operation_id,
                                options.output_mode,
                            )
                            .await?,
                        )
                    }
                }
            }
            ProjectCommand::MigrateUnity {
                in_place,
                copy_name,
                project_id,
                target_installation_id,
                expected_revision,
                plan_idempotency_key,
                idempotency_key,
            } => return unity_migration::run(
                &mut client, &config,
                alcomd_protocol::ProjectsPlanUnityMigrationParams {
                    project_id, target_installation_id, expected_project_revision: expected_revision,
                    idempotency_key: plan_idempotency_key,
                },
                in_place, copy_name, idempotency_key, options,
            ).await,
            ProjectCommand::Inspect {
                path,
                search_parents,
            } => {
                let path = absolute_path(path).map_err(alcomd_client::ClientError::Transport)?;
                serde_json::to_value(
                    client
                        .project_inspect(
                            path,
                            if search_parents {
                                alcomd_protocol::ProjectDiscoveryMode::SearchParents
                            } else {
                                alcomd_protocol::ProjectDiscoveryMode::ExactRoot
                            },
                        )
                        .await?,
                )
            }
            ProjectCommand::List { cursor, limit, view, expected_view_sequence } => {
                serde_json::to_value(client.projects_list_with_view(alcomd_protocol::ProjectsListParams {
                    cursor, limit, view, expected_view_sequence,
                }).await?)
            }
            ProjectCommand::SetFavorite { project_id, favorite, expected_revision, idempotency_key } => {
                serde_json::to_value(client.project_set_favorite(alcomd_protocol::ProjectSetFavoriteParams {
                    project_id, favorite, expected_revision, idempotency_key,
                }).await?)
            }
            ProjectCommand::Get { project_id } => {
                serde_json::to_value(client.project_get(project_id).await?)
            }
            ProjectCommand::Register { path, relative_to_default, idempotency_key } => {
                let source = if relative_to_default {
                    alcomd_protocol::ProjectSource::Default { relative_path: path.into_os_string().into_string().map_err(|_| CliError::Usage("project path must be UTF-8"))? }
                } else {
                    alcomd_protocol::ProjectSource::Absolute { path: absolute_path(path).map_err(CliError::LocalIo)? }
                };
                serde_json::to_value(client.project_register(alcomd_protocol::ProjectRegisterParams { source, idempotency_key }).await?)
            }
            ProjectCommand::Refresh {
                project_id,
                expected_revision,
                idempotency_key,
            } => serde_json::to_value(
                client
                    .project_refresh(project_id, expected_revision, idempotency_key)
                    .await?,
            ),
            ProjectCommand::Unregister {
                project_id,
                expected_revision,
                idempotency_key,
            } => serde_json::to_value(
                client
                    .project_unregister(project_id, expected_revision, idempotency_key)
                    .await?,
            ),
        },
        Command::Repository { command } => match command {
            RepositoryCommand::Move {repository_id,before_repository_id,expected_order_fingerprint,idempotency_key} => serde_json::to_value(client.repository_move(alcomd_protocol::RepositoryMoveParams{repository_id,before_repository_id,expected_order_fingerprint,idempotency_key}).await?),
            RepositoryCommand::SetDisplayName {repository_id,display_name,expected_revision,idempotency_key,..} => serde_json::to_value(client.repository_set_display_name(alcomd_protocol::RepositorySetDisplayNameParams{repository_id,display_name,expected_revision,idempotency_key}).await?),
            RepositoryCommand::RefreshAll {idempotency_key} => serde_json::to_value(client.repositories_refresh_all(alcomd_protocol::RepositoryRefreshAllParams{idempotency_key}).await?),
            RepositoryCommand::InspectListFile {path} => serde_json::to_value(client.repository_list_inspect_file(path).await?),
            RepositoryCommand::ImportListFile {path,expected_fingerprint,idempotency_key,overwrite_existing} => {
                eprintln!("Import repository configuration from {} with fingerprint {}.",path.escape_debug(),expected_fingerprint.escape_debug());
                confirm_high_impact(options)?;
                serde_json::to_value(client.repository_list_import_file(alcomd_protocol::RepositoryListImportParams{path,expected_fingerprint,idempotency_key,overwrite_existing}).await?)
            }
            RepositoryCommand::CancelListRegistration {idempotency_key} => serde_json::to_value(client.repository_list_cancel_registration(alcomd_protocol::RepositoryListCancelParams{idempotency_key}).await?),
            RepositoryCommand::PreviewListExport {format} => serde_json::to_value(client.repository_configuration_preview_export(format.into()).await?),
            RepositoryCommand::Inspect {
                source,
                credentials_stdin,
            } => serde_json::to_value(
                client
                    .repository_inspect_with_credentials(
                        repository_source(source)?,
                        repository_credentials_from_stdin(credentials_stdin)?,
                    )
                    .await?,
            ),
            RepositoryCommand::ExportListFile {target_path,expected_fingerprint,format,idempotency_key} =>
                serde_json::to_value(client.repository_configuration_export_file(target_path,expected_fingerprint,format.into(),idempotency_key).await?),
            RepositoryCommand::List { cursor, limit } => {
                serde_json::to_value(client.repositories_list(cursor, limit).await?)
            }
            RepositoryCommand::Get { repository_id } => {
                serde_json::to_value(client.repository_get(repository_id).await?)
            }
            RepositoryCommand::Packages {
                cursor,
                repository_id,
                limit,
            } => serde_json::to_value(
                client
                    .repository_packages(repository_id, cursor, limit)
                    .await?,
            ),
            RepositoryCommand::Register { preview_id, idempotency_key } => {
                confirm_frozen_plan(&mut client, alcomd_protocol::PlanKind::RepositoryRegistration, &preview_id, None, options).await?;
                serde_json::to_value(client.repository_register(preview_id,idempotency_key).await?)
            }
            RepositoryCommand::SetCredentials {
                repository_id,
                expected_revision,
                idempotency_key,
                clear,
                credentials_stdin,
            } => serde_json::to_value(
                client
                    .repository_set_credentials(alcomd_protocol::RepositorySetCredentialsParams {
                        repository_id,
                        expected_revision,
                        idempotency_key,
                        credentials: repository_credentials_from_stdin(
                            !clear && credentials_stdin,
                        )?,
                    })
                    .await?,
            ),
            RepositoryCommand::Refresh {
                repository_id,
                expected_revision,
                idempotency_key,
            } => serde_json::to_value(
                client
                    .repository_refresh(repository_id, expected_revision, idempotency_key)
                    .await?,
            ),
            RepositoryCommand::Unregister {
                repository_id,
                expected_revision,
                idempotency_key,
            } => serde_json::to_value(
                client
                    .repository_unregister(repository_id, expected_revision, idempotency_key)
                    .await?,
            ),
        },
        Command::Package { command } => match command {
            PackageCommand::Candidates(query) => serde_json::to_value(client.package_query_project_candidates(query.into()).await?),
            PackageCommand::Bulk(request) => return request.execute(&mut client, options).await,
            PackageCommand::Reinstall(request) => return request.execute(&mut client, options).await,
            PackageCommand::UserPackage { command } => return command.execute(&mut client, options).await,
            PackageCommand::Inventory { project_id, expected_revision, cursor, limit } => {
                serde_json::to_value(client.project_inventory(alcomd_protocol::ProjectInventoryParams {
                    project_id, expected_revision, cursor, limit,
                }).await?)
            }
            PackageCommand::Install {
                project_id,
                package_id,
                expected_revision,
                version_range,
                source,
                allow_unity_incompatible,
                include_prerelease,
                idempotency_key,
            } => {
                let plan = client
                    .package_plan_install(alcomd_protocol::PackagePlanInstallParams {
                        allow_unity_incompatible,
                        project_id,
                        expected_revision,
                        package_id,
                        version_range,
                        repository_id: None,
                        source: source.selector(),
                        include_prerelease,
                    })
                    .await?;
                Ok(apply_or_return_package_plan(
                    &mut client,
                    plan,
                    expected_revision,
                    idempotency_key,
                    options,
                )
                .await?)
            }
            PackageCommand::Remove {
                project_id,
                package_id,
                expected_revision,
                idempotency_key,
            } => {
                let plan = client
                    .package_plan_remove(alcomd_protocol::PackagePlanRemoveParams {
                        project_id,
                        expected_revision,
                        package_id,
                    })
                    .await?;
                Ok(apply_or_return_package_plan(
                    &mut client,
                    plan,
                    expected_revision,
                    idempotency_key,
                    options,
                )
                .await?)
            }
            PackageCommand::Upgrade {
                project_id,
                package_id,
                expected_revision,
                version_range,
                source,
                allow_unity_incompatible,
                include_prerelease,
                idempotency_key,
            } => {
                let plan = client
                    .package_plan_upgrade(alcomd_protocol::PackagePlanUpgradeParams {
                        allow_unity_incompatible,
                        project_id,
                        expected_revision,
                        package_id,
                        version_range,
                        repository_id: None,
                        source: source.selector(),
                        include_prerelease,
                    })
                    .await?;
                Ok(apply_or_return_package_plan(
                    &mut client,
                    plan,
                    expected_revision,
                    idempotency_key,
                    options,
                )
                .await?)
            }
            PackageCommand::Downgrade {
                project_id,
                package_id,
                version,
                expected_revision,
                source,
                allow_unity_incompatible,
                idempotency_key,
            } => {
                let plan = client
                    .package_plan_downgrade(alcomd_protocol::PackagePlanDowngradeParams {
                        allow_unity_incompatible,
                        project_id,
                        expected_revision,
                        package_id,
                        version,
                        repository_id: None,
                        source: source.selector(),
                    })
                    .await?;
                Ok(apply_or_return_package_plan(
                    &mut client,
                    plan,
                    expected_revision,
                    idempotency_key,
                    options,
                )
                .await?)
            }
            PackageCommand::Resolve {
                project_id,
                expected_revision,
                include_prerelease,
                idempotency_key,
            } => {
                let plan = client
                    .package_plan_resolve(alcomd_protocol::PackagePlanResolveParams {
                        project_id,
                        expected_revision,
                        include_prerelease,
                    })
                    .await?;
                Ok(apply_or_return_package_plan(
                    &mut client,
                    plan,
                    expected_revision,
                    idempotency_key,
                    options,
                )
                .await?)
            }
            PackageCommand::ApplyPlan {
                plan_id,
                expected_revision,
                idempotency_key,
            } => {
                let expected_revision = confirm_frozen_plan(&mut client, alcomd_protocol::PlanKind::Package, &plan_id, expected_revision, options).await?.ok_or(alcomd_client::ClientError::InvalidResponse)?;
                let accepted = client
                    .package_apply_plan(alcomd_protocol::PackageApplyPlanParams {
                        plan_id,
                        expected_revision,
                        idempotency_key,
                    })
                    .await?;
                if options.no_wait {
                    serde_json::to_value(accepted)
                } else {
                    serde_json::to_value(
                        wait_for_operation(&mut client, accepted.operation_id, options.output_mode)
                            .await?,
                    )
                }
            }
        },
        Command::Unity { command } => match command {
            UnityCommand::HubOpen{expected_revision,expected_target_token,idempotency_key} =>
                serde_json::to_value(client.unity_hub_open(alcomd_protocol::UnityHubOpenParams {expected_revision,expected_target_token,idempotency_key}).await?),
            UnityCommand::List { cursor, limit, include_version_options } => serde_json::to_value(
                client
                    .unity_installations_list(alcomd_protocol::UnityInstallationsListParams {
                        cursor,
                        limit,
                        include_version_options,
                    })
                    .await?,
            ),
            UnityCommand::Get { installation_id } => {
                serde_json::to_value(client.unity_installation_get(installation_id).await?)
            }
            UnityCommand::Refresh { idempotency_key } => {
                serde_json::to_value(client.unity_installations_refresh(idempotency_key).await?)
            }
            UnityCommand::Register {
                executable_path,
                idempotency_key,
            } => serde_json::to_value(
                client
                    .unity_installation_register(alcomd_protocol::UnityInstallationRegisterParams {
                        executable_path: absolute_path(executable_path)
                            .map_err(CliError::LocalIo)?,
                        idempotency_key,
                    })
                    .await?,
            ),
            UnityCommand::Remove {
                installation_id,
                expected_revision,
                idempotency_key,
            } => serde_json::to_value(
                client
                    .unity_installation_remove(alcomd_protocol::UnityInstallationRemoveParams {
                        installation_id,
                        expected_revision,
                        idempotency_key,
                    })
                    .await?,
            ),
            UnityCommand::ProjectLaunchConfigGet { project_id } => {
                serde_json::to_value(client.unity_project_launch_config_get(project_id).await?)
            }
            UnityCommand::ProjectLaunchConfigSet {
                project_id,
                arguments,
                expected_revision,
                idempotency_key,
            } => serde_json::to_value(
                client
                    .unity_project_launch_config_set(
                        alcomd_protocol::ProjectUnityLaunchConfigSetParams {
                            project_id,
                            arguments,
                            expected_revision,
                            idempotency_key,
                        },
                    )
                    .await?,
            ),
            UnityCommand::ProjectLaunchConfigClear {
                project_id,
                expected_revision,
                idempotency_key,
            } => serde_json::to_value(
                client
                    .unity_project_launch_config_clear(
                        alcomd_protocol::ProjectUnityLaunchConfigClearParams {
                            project_id,
                            expected_revision,
                            idempotency_key,
                        },
                    )
                    .await?,
            ),
            UnityCommand::WriterState { project_id } => {
                serde_json::to_value(client.unity_writer_state(project_id).await?)
            }
            UnityCommand::Launch {
                project_id,
                installation_id,
                expected_project_revision,
                idempotency_key,
            } => serde_json::to_value(
                client
                    .unity_launch(alcomd_protocol::UnityLaunchParams {
                        project_id,
                        installation_id,
                        expected_project_revision,
                        idempotency_key,
                    })
                    .await?,
            ),
            UnityCommand::LaunchOptions {
                include_migration_version_options,
                project_id,
                expected_project_revision,
            } => serde_json::to_value(
                client
                    .unity_launch_options(alcomd_protocol::UnityLaunchOptionsParams {
                        project_id,
                        expected_project_revision,
                        include_migration_version_options,
                    })
                    .await?,
            ),
        },
        Command::Backup { command } => match command {
            BackupCommand::Create {
                project_id,
                expected_revision,
                compression,
                exclude_vpm_packages,
                include_library,
                destination,
                idempotency_key,
            } => {
                let accepted = client
                    .backup_create(alcomd_protocol::BackupCreateParams {
                        project_id,
                        expected_revision,
                        compression_mode: compression.map(|value| match value {
                            BackupCompression::Store => alcomd_protocol::BackupCompression::Store,
                            BackupCompression::Fast => alcomd_protocol::BackupCompression::Fast,
                            BackupCompression::Level3 => alcomd_protocol::BackupCompression::Level3,
                            BackupCompression::Level5 => alcomd_protocol::BackupCompression::Level5,
                            BackupCompression::Level7 => alcomd_protocol::BackupCompression::Level7,
                            BackupCompression::Maximum => {
                                alcomd_protocol::BackupCompression::Maximum
                            }
                        }),
                        exclude_vpm_packages,
                        include_library,
                        destination_path: destination.map(absolute_path).transpose().map_err(CliError::LocalIo)?,
                        idempotency_key,
                    })
                    .await?;
                if options.no_wait {
                    serde_json::to_value(accepted)
                } else {
                    serde_json::to_value(
                        wait_for_operation(&mut client, accepted.operation_id, options.output_mode)
                            .await?,
                    )
                }
            }
            BackupCommand::Restore { archive, name, destination, idempotency_key } => {
                let archive_path = absolute_path(archive).map_err(CliError::LocalIo)?;
                let destination_path = destination.map(absolute_path).transpose().map_err(CliError::LocalIo)?;
                confirm_high_impact(options)?;
                let accepted = client.backup_restore(alcomd_protocol::BackupRestoreParams {
                    archive_path,
                    project_name: name,
                    destination_path,
                    idempotency_key,
                }).await?;
                if options.no_wait {
                    serde_json::to_value(accepted)
                } else {
                    serde_json::to_value(
                        wait_for_operation(&mut client, accepted.operation_id, options.output_mode).await?,
                    )
                }
            }
        },
        Command::Completion { .. } => unreachable!("completion is handled before RPC setup"),
    }
    .map_err(|_| CliError::Client(alcomd_client::ClientError::InvalidResponse))?;
    Ok(value)
}

impl Command {
    fn name(&self) -> &'static str {
        match self {
            Self::Desktop { command } => match command {
                DesktopCommand::FocusProjectUnity { .. } => "desktop focus-project-unity",
                DesktopCommand::OpenProjectDirectory { .. } => "desktop open-project-directory",
                DesktopCommand::OpenUnityHubDownload { .. } => "desktop open-unity-hub-download",
            },
            Self::Client { command } => match command {
                NativeClientCommand::List { .. } => "client list",
                NativeClientCommand::Instances { .. } => "client instances",
                NativeClientCommand::Disconnect { .. } => "client disconnect",
            },
            Self::Mcp {
                command: McpCommand::Runtime { .. },
            } => "mcp runtime",
            Self::Mcp {
                command: McpCommand::Transport { .. },
            } => "mcp transport",
            Self::Mcp {
                command: McpCommand::Oauth { command },
            } => m8_oauth::name(command),
            Self::Mcp {
                command: McpCommand::Client { command },
            } => match command {
                McpClientCommand::Create { .. } => "mcp client create",
                McpClientCommand::Revoke { .. } => "mcp client revoke",
                McpClientCommand::Request { command } => command.name(),
                McpClientCommand::List { .. } => "mcp client list",
                McpClientCommand::IssueCredential { .. } => "mcp client issue-credential",
            },
            Self::Extension { command } => command.name(),
            Self::System { .. } => "system status",
            Self::Settings { command } => match command {
                SettingsCommand::Get => "settings get",
                SettingsCommand::Update { .. } => "settings update",
            },
            Self::Operation { command } => match command {
                OperationCommand::List { .. } => "operation list",
                OperationCommand::Get { .. } => "operation get",
                OperationCommand::Follow { .. } => "operation follow",
                OperationCommand::Cancel { .. } => "operation cancel",
            },
            Self::Template { command } => match command {
                TemplateCommand::List { .. } => "template list",
                TemplateCommand::Get { .. } => "template get",
                TemplateCommand::Edit { .. } => "template edit",
                TemplateCommand::Save { .. } => "template save",
                TemplateCommand::Export { .. } => "template export",
                TemplateCommand::Remove { .. } => "template remove",
            },
            Self::Project { command } => match command {
                ProjectCommand::ApplyCopy { .. } => "project apply-copy",
                ProjectCommand::ApplyDeleteDirectory { .. } => "project apply-delete-directory",
                ProjectCommand::ApplyUnityMigration { .. } => "project apply-unity-migration",
                ProjectCommand::Copy { .. } => "project copy",
                ProjectCommand::Create { .. } => "project create",
                ProjectCommand::DeleteDirectory { .. } => "project delete-directory",
                ProjectCommand::MigrateUnity { .. } => "project migrate-unity",
                ProjectCommand::Inspect { .. } => "project inspect",
                ProjectCommand::List { .. } => "project list",
                ProjectCommand::Get { .. } => "project get",
                ProjectCommand::Register { .. } => "project register",
                ProjectCommand::Refresh { .. } => "project refresh",
                ProjectCommand::SetFavorite { .. } => "project set-favorite",
                ProjectCommand::Unregister { .. } => "project unregister",
            },
            Self::Events {
                command: EventsCommand::List { .. },
            } => "events list",
            Self::Events {
                command: EventsCommand::Subscribe { .. },
            } => "events subscribe",
            Self::Logs {
                command: LogsCommand::Read { .. },
            } => "logs read",
            Self::State {
                command: StateCommand::Check { .. },
            } => "state check",
            Self::Repository { command } => match command {
                RepositoryCommand::Move { .. } => "repository move",
                RepositoryCommand::SetDisplayName { .. } => "repository set-display-name",
                RepositoryCommand::RefreshAll { .. } => "repository refresh-all",
                RepositoryCommand::InspectListFile { .. } => "repository inspect-list-file",
                RepositoryCommand::ImportListFile { .. } => "repository import-list-file",
                RepositoryCommand::CancelListRegistration { .. } => {
                    "repository cancel-list-registration"
                }
                RepositoryCommand::PreviewListExport { .. } => "repository preview-list-export",
                RepositoryCommand::Inspect { .. } => "repository inspect",
                RepositoryCommand::List { .. } => "repository list",
                RepositoryCommand::ExportListFile { .. } => "repository export-list-file",
                RepositoryCommand::Get { .. } => "repository get",
                RepositoryCommand::Packages { .. } => "repository packages",
                RepositoryCommand::Register { .. } => "repository register",
                RepositoryCommand::SetCredentials { .. } => "repository set-credentials",
                RepositoryCommand::Refresh { .. } => "repository refresh",
                RepositoryCommand::Unregister { .. } => "repository unregister",
            },
            Self::Plan { .. } => "plan get",
            Self::Package { command } => match command {
                PackageCommand::Candidates(_) => "package candidates",
                PackageCommand::Bulk(_) => "package bulk",
                PackageCommand::Reinstall(_) => "package reinstall",
                PackageCommand::UserPackage { command } => command.name(),
                PackageCommand::Install { .. } => "package install",
                PackageCommand::Inventory { .. } => "package inventory",
                PackageCommand::Remove { .. } => "package remove",
                PackageCommand::Upgrade { .. } => "package upgrade",
                PackageCommand::Downgrade { .. } => "package downgrade",
                PackageCommand::Resolve { .. } => "package resolve",
                PackageCommand::ApplyPlan { .. } => "package apply-plan",
            },
            Self::Unity { command } => match command {
                UnityCommand::List { .. } => "unity list",
                UnityCommand::HubOpen { .. } => "unity hub-open",
                UnityCommand::Get { .. } => "unity get",
                UnityCommand::Refresh { .. } => "unity refresh",
                UnityCommand::Register { .. } => "unity register",
                UnityCommand::Remove { .. } => "unity remove",
                UnityCommand::ProjectLaunchConfigGet { .. } => "unity project-launch-config-get",
                UnityCommand::ProjectLaunchConfigSet { .. } => "unity project-launch-config-set",
                UnityCommand::ProjectLaunchConfigClear { .. } => {
                    "unity project-launch-config-clear"
                }
                UnityCommand::WriterState { .. } => "unity writer-state",
                UnityCommand::Launch { .. } => "unity launch",
                UnityCommand::LaunchOptions { .. } => "unity launch-options",
            },
            Self::Backup { command } => match command {
                BackupCommand::Create { .. } => "backup create",
                BackupCommand::Restore { .. } => "backup restore",
            },
            Self::Completion { .. } => "completion",
        }
    }
}

async fn apply_or_return_package_plan(
    client: &mut alcomd_client::AlcomdClient,
    plan: alcomd_protocol::PackagePlan,
    expected_revision: u64,
    idempotency_key: Option<String>,
    options: ExecutionOptions,
) -> Result<serde_json::Value, CliError> {
    if options.dry_run {
        return serde_json::to_value(plan)
            .map_err(|_| CliError::Client(alcomd_client::ClientError::InvalidResponse));
    }
    if options.output_mode == OutputMode::Human && !options.yes {
        show_package_plan(&plan);
    }
    confirm_high_impact(options)?;
    let accepted = client
        .package_apply_plan(alcomd_protocol::PackageApplyPlanParams {
            plan_id: plan.plan_id,
            expected_revision,
            idempotency_key: idempotency_key
                .ok_or(CliError::Usage("Apply requires --idempotency-key"))?,
        })
        .await?;
    if options.no_wait {
        serde_json::to_value(accepted)
    } else {
        serde_json::to_value(
            wait_for_operation(client, accepted.operation_id, options.output_mode).await?,
        )
    }
    .map_err(|_| CliError::Client(alcomd_client::ClientError::InvalidResponse))
}

fn print_result(
    output_mode: OutputMode,
    command: &str,
    value: &serde_json::Value,
) -> io::Result<()> {
    let stdout = io::stdout();
    let mut output = stdout.lock();
    write_result(&mut output, output_mode, command, value)
}

fn print_credential_result(
    output_mode: OutputMode,
    command: &str,
    value: &serde_json::Value,
) -> io::Result<()> {
    let stdout = io::stdout();
    #[cfg(windows)]
    if stdout.is_terminal() {
        // Preserve the standard library's Unicode console handling.
        return write_result(&mut stdout.lock(), output_mode, command, value);
    }
    // Stdout tolerates EBADF on Unix. One-time secrets must observe actual
    // file/pipe failures; duplicate the existing handle, never open a destination.
    #[cfg(unix)]
    let file = {
        use std::os::fd::AsFd;
        std::fs::File::from(stdout.as_fd().try_clone_to_owned()?)
    };
    #[cfg(windows)]
    let file = {
        use std::os::windows::io::AsHandle;
        std::fs::File::from(stdout.as_handle().try_clone_to_owned()?)
    };
    write_result(&mut io::BufWriter::new(file), output_mode, command, value)
}

fn write_result(
    output: &mut impl Write,
    output_mode: OutputMode,
    command: &str,
    value: &serde_json::Value,
) -> io::Result<()> {
    if matches!(output_mode, OutputMode::Json | OutputMode::Ndjson) {
        let document = serde_json::json!({
            "type": "result",
            "command": command,
            "result": value,
        });
        writeln!(output, "{document}")
    } else if command == "mcp oauth request show" {
        m8_oauth::write_review(output, value)
    } else if let (Some(product), Some(version), Some(state), Some(rpc_version)) = (
        value.get("product").and_then(serde_json::Value::as_str),
        value
            .get("daemonVersion")
            .and_then(serde_json::Value::as_str),
        value.get("state").and_then(serde_json::Value::as_str),
        value.get("rpcVersion").and_then(serde_json::Value::as_u64),
    ) {
        writeln!(
            output,
            "{product} daemon {version}: {state} (RPC v{rpc_version})"
        )
    } else {
        writeln!(
            output,
            "{}",
            serde_json::to_string_pretty(value).expect("approved result DTO must serialize")
        )
    }?;
    output.flush()
}

fn project_target(
    name_or_parent: PathBuf,
    name: Option<String>,
) -> Result<alcomd_protocol::ProjectTarget, CliError> {
    match name {
        Some(name) => Ok(alcomd_protocol::ProjectTarget::Absolute {
            parent_path: absolute_path(name_or_parent).map_err(CliError::LocalIo)?,
            name,
        }),
        None => Ok(alcomd_protocol::ProjectTarget::Default {
            name: name_or_parent
                .into_os_string()
                .into_string()
                .map_err(|_| CliError::Usage("project name must be UTF-8"))?,
        }),
    }
}

fn absolute_path(path: PathBuf) -> std::io::Result<String> {
    let path = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()?.join(path)
    };
    path.to_str().map(str::to_owned).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "path encoding is unsupported",
        )
    })
}

fn show_package_plan(plan: &alcomd_protocol::PackagePlan) {
    eprintln!(
        "Package plan {:?} for project {:?}",
        plan.plan_id, plan.project_id
    );
    for change in &plan.change_set.mutations {
        eprintln!(
            "  {:?} {:?}  {:?} -> {:?}",
            change.kind, change.package_id, change.from_version, change.to_version
        );
    }
    for removal in &plan.change_set.legacy_removals {
        eprintln!("  Remove {:?} {:?}", removal.kind, removal.path);
    }
    for conflict in &plan.change_set.removal_conflicts {
        eprintln!(
            "  {:?} required by {:?}",
            conflict.package_id, conflict.required_by
        );
    }
    for exception in &plan.change_set.unity_compatibility_overrides {
        eprintln!(
            "  Unity compatibility exception for {:?} {:?}",
            exception.package_id, exception.version
        );
    }
}

fn show_copy_plan(plan: &alcomd_protocol::ProjectCopyPlan) {
    eprintln!(
        "Copy {:?} to {:?} / {:?} using Unity {:?}",
        plan.source_canonical_root_path,
        plan.target_parent_canonical_path,
        plan.normalized_target_leaf,
        plan.expected_unity_version
    );
}

fn show_unity_migration_plan(plan: &alcomd_protocol::ProjectUnityMigrationPlan) {
    if let Some(sdk) = &plan.target_sdk_version {
        eprintln!(
            "VRChat SDK {sdk}. After uploading this avatar or world with Unity 6, Unity 2022 cannot update that content. Complete the VRChat SDK Upgrade Wizard after migration."
        );
    }
    eprintln!(
        "Migrate project {:?} from Unity {:?} to {:?} using installation {:?}",
        plan.project_id,
        plan.source_unity_version,
        plan.target_unity_version,
        plan.target_installation_id
    );
}

async fn confirm_frozen_plan(
    client: &mut alcomd_client::AlcomdClient,
    kind: alcomd_protocol::PlanKind,
    plan_id: &str,
    expected_revision: Option<u64>,
    options: ExecutionOptions,
) -> Result<Option<u64>, CliError> {
    use alcomd_protocol::{PlanKind, PlansGetResult};
    if options.yes
        && (expected_revision.is_some() || matches!(kind, PlanKind::RepositoryRegistration))
    {
        return Ok(expected_revision);
    }
    if !options.yes
        && (options.output_mode != OutputMode::Human
            || !io::stdin().is_terminal()
            || !io::stderr().is_terminal())
    {
        return Err(CliError::ConfirmationRequired);
    }
    let frozen = client
        .get_plan(alcomd_protocol::PlansGetParams {
            kind,
            plan_id: plan_id.to_owned(),
        })
        .await?;
    let revision = match frozen {
        PlansGetResult::RepositoryRegistration { plan }
            if kind == PlanKind::RepositoryRegistration =>
        {
            eprintln!(
                "{}",
                serde_json::to_string_pretty(&plan)
                    .map_err(|_| alcomd_client::ClientError::InvalidResponse)?
            );
            None
        }
        PlansGetResult::Package { plan } if kind == PlanKind::Package => {
            if !options.yes {
                show_package_plan(&plan);
            }
            Some(plan.project_revision)
        }
        PlansGetResult::ProjectCopy { plan } if kind == PlanKind::ProjectCopy => {
            if !options.yes {
                show_copy_plan(&plan);
            }
            Some(plan.source_project_revision)
        }
        PlansGetResult::ProjectDeleteDirectory { plan }
            if kind == PlanKind::ProjectDeleteDirectory =>
        {
            show_directory_delete(&plan, options);
            Some(plan.project_revision)
        }
        PlansGetResult::ExtensionInstall { plan } if kind == PlanKind::ExtensionInstall => {
            if !options.yes {
                eprintln!(
                    "{}",
                    serde_json::to_string_pretty(&plan)
                        .map_err(|_| alcomd_client::ClientError::InvalidResponse)?
                );
            }
            None
        }
        PlansGetResult::ExtensionUninstall { plan } if kind == PlanKind::ExtensionUninstall => {
            if !options.yes {
                eprintln!(
                    "{}",
                    serde_json::to_string_pretty(&plan)
                        .map_err(|_| alcomd_client::ClientError::InvalidResponse)?
                );
            }
            None
        }
        _ => return Err(alcomd_client::ClientError::InvalidResponse.into()),
    };
    confirm_high_impact(options)?;
    Ok(expected_revision.or(revision))
}

fn confirm_high_impact(options: ExecutionOptions) -> Result<(), CliError> {
    if options.yes {
        return Ok(());
    }
    if options.output_mode != OutputMode::Human
        || !std::io::stdin().is_terminal()
        || !std::io::stderr().is_terminal()
    {
        return Err(CliError::ConfirmationRequired);
    }
    eprint!("Apply this high-impact operation? [y/N] ");
    std::io::stderr().flush().map_err(CliError::LocalIo)?;
    let mut response = String::new();
    let read = std::io::stdin()
        .read_line(&mut response)
        .map_err(CliError::LocalIo)?;
    if read == 0 || !matches!(response.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
        return Err(CliError::ConfirmationRequired);
    }
    Ok(())
}

fn show_directory_delete(plan: &alcomd_protocol::ProjectDeletePlan, options: ExecutionOptions) {
    if !options.yes && options.output_mode == OutputMode::Human {
        eprintln!("Delete Project Directory");
        eprintln!("Project: {}", plan.normalized_leaf);
        eprintln!("Location: {}", plan.canonical_root_path);
        eprintln!("Writer observation: {}", plan.writer_evidence.state);
        eprintln!("Files will be permanently deleted.");
        eprintln!("They will not be moved to Trash or Recycle Bin.");
        eprintln!("No automatic Backup will be created.");
        eprintln!("Use Remove from list to keep the project directory.");
    }
}

async fn wait_for_operation(
    client: &mut alcomd_client::AlcomdClient,
    operation_id: String,
    output_mode: OutputMode,
) -> Result<alcomd_protocol::Operation, CliError> {
    let mut emitted_revision = None;
    loop {
        let operation = tokio::select! {
            operation = client.operation_get(operation_id.clone()) => operation?,
            interrupted = tokio::signal::ctrl_c() => {
                interrupted.map_err(CliError::LocalIo)?;
                return Err(CliError::OperationDetached(operation_id));
            }
        };
        if output_mode == OutputMode::Ndjson && emitted_revision != Some(operation.revision) {
            let stdout = io::stdout();
            let mut output = stdout.lock();
            writeln!(
                output,
                "{}",
                serde_json::json!({
                    "type": "operation",
                    "operationId": operation.operation_id,
                    "operation": operation,
                })
            )
            .map_err(CliError::LocalIo)?;
            if let Some(progress) = operation.progress {
                writeln!(
                    output,
                    "{}",
                    serde_json::json!({
                        "type": "progress",
                        "operationId": operation.operation_id,
                        "progress": progress,
                    })
                )
                .map_err(CliError::LocalIo)?;
            }
            emitted_revision = Some(operation.revision);
        }
        match operation.state {
            alcomd_protocol::OperationState::Succeeded => return Ok(operation),
            alcomd_protocol::OperationState::Failed
            | alcomd_protocol::OperationState::Cancelled => {
                return Err(CliError::OperationFailed(Box::new(operation)));
            }
            _ => {}
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

fn repository_source(
    source: String,
) -> Result<alcomd_protocol::RepositorySource, alcomd_client::ClientError> {
    if source.starts_with("http://") || source.starts_with("https://") {
        Ok(alcomd_protocol::RepositorySource::Remote { url: source })
    } else {
        let path =
            absolute_path(PathBuf::from(source)).map_err(alcomd_client::ClientError::Transport)?;
        Ok(alcomd_protocol::RepositorySource::Local { path })
    }
}

fn repository_credentials_from_stdin(
    enabled: bool,
) -> Result<Option<alcomd_protocol::RepositoryCredentials>, CliError> {
    if !enabled {
        return Ok(None);
    }
    let input = io::stdin();
    let invalid = |message| CliError::LocalIo(io::Error::new(io::ErrorKind::InvalidInput, message));
    if input.is_terminal() {
        return Err(invalid("pipe the credentials JSON into standard input"));
    }
    let mut bytes = Vec::new();
    input
        .lock()
        .take(8193)
        .read_to_end(&mut bytes)
        .map_err(CliError::LocalIo)?;
    if bytes.len() > 8192 {
        return Err(invalid("repository credentials input is too large"));
    }
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|_| invalid("invalid repository credentials JSON"))
}

fn print_error(output_mode: OutputMode, error: &CliError) -> io::Result<()> {
    if output_mode == OutputMode::Human
        && let CliError::ExtensionCommandFailed {
            core_effect: Some(effect),
            ..
        } = error
    {
        writeln!(io::stderr().lock(), "Accepted Core effect: {effect}")?;
    }
    if output_mode == OutputMode::Ndjson
        && let CliError::OperationFailed(operation) = error
    {
        let stdout = io::stdout();
        let mut output = stdout.lock();
        let terminal_error = alcomd_protocol::RpcError {
            code: operation
                .error_code
                .clone()
                .unwrap_or_else(|| "operation_cancelled".to_owned()),
            message: "The operation did not succeed.".to_owned(),
            diagnostic_id: operation.diagnostic_id.clone(),
            data: None,
        };
        let document = serde_json::json!({
            "type": "error",
            "operationId": operation.operation_id,
            "error": terminal_error,
        });
        return writeln!(output, "{document}");
    }
    let stderr = io::stderr();
    let mut output = stderr.lock();
    if matches!(output_mode, OutputMode::Json | OutputMode::Ndjson) {
        let (code, message) = match error {
            CliError::Usage(message) => ("invalid_arguments", *message),
            CliError::CredentialOutcomeUnknown => (
                "credential_outcome_unknown",
                "issuance may have succeeded; check current revision; plaintext cannot be recovered; no retry was made",
            ),
            CliError::OAuthCredentialOutcomeUnknown => (
                "credential_outcome_unknown",
                "OAuth registration update may have succeeded; list registrations and check its revision before explicit rotation; the one-time client secret cannot be recovered; no retry was made",
            ),
            CliError::ConfirmationRequired => ("confirmation_required", "confirmation is required"),
            CliError::ExtensionCommandFailed {
                code,
                core_effect,
                replayed,
            } => {
                let document = serde_json::json!({ "type": "error", "error": {
                    "code": code, "coreEffect": core_effect, "replayed": replayed,
                }});
                return writeln!(output, "{document}");
            }
            CliError::OperationFailed(operation) => (
                operation
                    .error_code
                    .as_deref()
                    .unwrap_or("operation_cancelled"),
                "the operation did not succeed",
            ),
            CliError::OperationDetached(_) => {
                ("operation_detached", "operation follow was interrupted")
            }
            CliError::LocalIo(_) => ("local_io_error", "local CLI I/O failed"),
            CliError::Client(alcomd_client::ClientError::Remote(remote)) => {
                let document = serde_json::json!({
                    "type": "error",
                    "error": remote,
                });
                return writeln!(output, "{document}");
            }
            CliError::Client(alcomd_client::ClientError::InvalidResponse) => (
                "invalid_response",
                "the daemon returned an invalid RPC response",
            ),
            CliError::Client(alcomd_client::ClientError::StartTimeout) => (
                "daemon_start_timeout",
                "the daemon did not become ready within five seconds",
            ),
            CliError::Client(alcomd_client::ClientError::ReadTimeout) => (
                "daemon_unavailable",
                "the daemon read did not finish within its deadline",
            ),
            CliError::Client(
                alcomd_client::ClientError::Transport(_)
                | alcomd_client::ClientError::StartDaemon(_)
                | alcomd_client::ClientError::DaemonPathUnavailable,
            ) => ("daemon_unavailable", "the daemon is unavailable"),
        };
        let document = serde_json::json!({
            "type": "error",
            "error": {
                "code": code,
                "message": message,
            },
        });
        writeln!(output, "{document}")
    } else {
        writeln!(output, "error: {error}")
    }
}

fn output_exit(result: io::Result<()>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}

fn credential_output_exit(result: io::Result<()>) -> ExitCode {
    if result.is_err() {
        eprintln!(
            "credential was issued, but output failed; plaintext cannot be recovered; no automatic reissue was made"
        );
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn output_error_exit(result: io::Result<()>) -> ExitCode {
    match result {
        Ok(()) | Err(_) => ExitCode::FAILURE,
    }
}

fn write_completion(shell: CompletionShell) -> ExitCode {
    use clap::CommandFactory;
    let root = Arguments::command();
    let commands = root
        .get_subcommands()
        .map(|group| group.get_name())
        .collect::<Vec<_>>()
        .join(" ");
    let script = match shell {
        CompletionShell::Bash => format!("complete -W '{commands}' alcomd-cli\n"),
        CompletionShell::Zsh => {
            format!("#compdef alcomd-cli\n_arguments '1:command:({commands})'\n")
        }
        CompletionShell::PowerShell => format!(
            "Register-ArgumentCompleter -Native -CommandName alcomd-cli -ScriptBlock {{ param($wordToComplete) '{commands}'.Split(' ') | Where-Object {{ $_ -like \"$wordToComplete*\" }} }}\n"
        ),
    };
    output_exit(io::stdout().lock().write_all(script.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_output_failure_is_never_a_successful_exit() {
        for kind in [io::ErrorKind::BrokenPipe, io::ErrorKind::PermissionDenied] {
            assert_eq!(credential_output_exit(Err(kind.into())), ExitCode::FAILURE);
        }
        assert_eq!(credential_output_exit(Ok(())), ExitCode::SUCCESS);
        assert_eq!(
            output_exit(Err(io::ErrorKind::BrokenPipe.into())),
            ExitCode::SUCCESS
        );
    }

    #[test]
    fn result_output_reports_a_failure_deferred_until_flush() {
        struct FlushFailure;
        impl Write for FlushFailure {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Err(io::ErrorKind::BrokenPipe.into())
            }
        }
        for mode in [OutputMode::Human, OutputMode::Json, OutputMode::Ndjson] {
            let error = write_result(
                &mut FlushFailure,
                mode,
                "fixture",
                &serde_json::json!({"result": "non-secret"}),
            )
            .expect_err("a completed format write does not prove buffered delivery");
            assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
        }
    }

    #[test]
    fn mcp_secret_output_requires_opt_in_before_any_connection() {
        let parse = |extra: &[&str]| {
            let mut args = vec![
                "alcomd-cli",
                "mcp",
                "client",
                "issue-credential",
                "00000000-0000-4000-8000-000000000001",
                "--expected-revision",
                "1",
                "--idempotency-key",
                "issue",
            ];
            args.extend_from_slice(extra);
            Arguments::try_parse_from(args).expect("CLI")
        };
        let command = parse(&[]).command;
        let mut options = ExecutionOptions {
            output_mode: OutputMode::Human,
            yes: true,
            dry_run: false,
            no_wait: false,
        };
        assert!(validate_mcp_output(&command, options, true).is_ok());
        assert!(matches!(
            validate_mcp_output(&command, options, false),
            Err(CliError::Usage(_))
        ));
        options.output_mode = OutputMode::Json;
        assert!(validate_mcp_output(&command, options, true).is_err());
        let permitted = parse(&["--secret-stdout"]).command;
        assert!(validate_mcp_output(&permitted, options, false).is_ok());
        options.output_mode = OutputMode::Ndjson;
        assert!(validate_mcp_output(&permitted, options, false).is_ok());
        options.dry_run = true;
        assert!(validate_mcp_output(&permitted, options, false).is_err());
        options.dry_run = false;
        options.no_wait = true;
        assert!(validate_mcp_output(&permitted, options, false).is_err());
    }

    #[test]
    fn project_create_requires_a_version_and_supports_no_template() {
        let base = [
            "alcomd-cli",
            "project",
            "create",
            ".",
            "Created",
            "--idempotency-key",
            "create",
        ];
        assert!(Arguments::try_parse_from(base).is_err());
        let arguments = base.into_iter().chain(["--unity-version", "2022.3.22f1"]);
        let parsed = Arguments::try_parse_from(arguments.clone()).expect("standalone creation");
        let Command::Project {
            command:
                ProjectCommand::Create {
                    unity_version,
                    template_id,
                    template_revision,
                    ..
                },
        } = parsed.command
        else {
            panic!("project create");
        };
        assert_eq!(unity_version, "2022.3.22f1");
        assert!(template_id.is_none() && template_revision.is_none());
        assert!(Arguments::try_parse_from(arguments.clone().chain(["--dry-run"])).is_err());
        assert!(
            Arguments::try_parse_from(
                arguments
                    .clone()
                    .chain(["--template-id", "com.example.avatar-tools",])
            )
            .is_err()
        );
        assert!(
            Arguments::try_parse_from(arguments.chain([
                "--template-id",
                "com.example.avatar-tools",
                "--template-revision",
                "1",
            ]))
            .is_ok()
        );
        assert!(Arguments::try_parse_from(["alcomd-cli", "template", "list"]).is_ok());
    }

    #[test]
    fn backup_compression_cli_publishes_only_the_six_approved_choices() {
        for (name, expected) in [
            ("store", BackupCompression::Store),
            ("fast", BackupCompression::Fast),
            ("level3", BackupCompression::Level3),
            ("level5", BackupCompression::Level5),
            ("level7", BackupCompression::Level7),
            ("maximum", BackupCompression::Maximum),
        ] {
            let parsed = Arguments::try_parse_from([
                "alcomd-cli",
                "backup",
                "create",
                "00000000-0000-4000-8000-000000000001",
                "--expected-revision",
                "1",
                "--compression",
                name,
                "--idempotency-key",
                "six-choices",
            ])
            .expect(name);
            let Command::Backup {
                command: BackupCommand::Create { compression, .. },
            } = parsed.command
            else {
                panic!("backup create command");
            };
            assert_eq!(compression, Some(expected));
        }
        for invalid in [
            "level2", "level4", "level6", "level8", "level9", "level-3", "3", "balanced",
        ] {
            assert!(
                Arguments::try_parse_from([
                    "alcomd-cli",
                    "backup",
                    "create",
                    "00000000-0000-4000-8000-000000000001",
                    "--expected-revision",
                    "1",
                    "--compression",
                    invalid,
                    "--idempotency-key",
                    "invalid-choice",
                ])
                .is_err(),
                "{invalid}"
            );
        }
    }

    #[test]
    fn backup_commands_accept_file_paths_and_options() {
        let create = Arguments::try_parse_from([
            "alcomd-cli",
            "backup",
            "create",
            "00000000-0000-4000-8000-000000000001",
            "--expected-revision",
            "1",
            "--compression",
            "maximum",
            "--exclude-vpm-packages",
            "--idempotency-key",
            "fixture",
            "--no-wait",
        ])
        .expect("backup create");
        assert!(matches!(
            create.command,
            Command::Backup {
                command: BackupCommand::Create {
                    compression: Some(BackupCompression::Maximum),
                    exclude_vpm_packages: Some(true),
                    ..
                }
            }
        ));
        assert!(create.no_wait);
        let inherited = Arguments::try_parse_from([
            "alcomd-cli",
            "backup",
            "create",
            "00000000-0000-4000-8000-000000000001",
            "--expected-revision",
            "1",
            "--idempotency-key",
            "inherit",
        ])
        .expect("backup defaults");
        assert!(matches!(
            inherited.command,
            Command::Backup {
                command: BackupCommand::Create {
                    compression: None,
                    exclude_vpm_packages: None,
                    include_library: None,
                    ..
                }
            }
        ));
        let explicit = Arguments::try_parse_from([
            "alcomd-cli",
            "backup",
            "create",
            "00000000-0000-4000-8000-000000000001",
            "--expected-revision",
            "1",
            "--idempotency-key",
            "explicit",
            "--exclude-vpm-packages=false",
            "--include-library",
        ])
        .expect("explicit boolean overrides");
        assert!(matches!(
            explicit.command,
            Command::Backup {
                command: BackupCommand::Create {
                    exclude_vpm_packages: Some(false),
                    include_library: Some(true),
                    ..
                }
            }
        ));
        let restore = Arguments::try_parse_from([
            "alcomd-cli",
            "backup",
            "restore",
            "C:\\Users\\Fixture\\external\\Unity Test.zip",
            "--name",
            "RestoredProject",
            "--idempotency-key",
            "restore-fixture",
            "--yes",
            "--no-wait",
        ])
        .expect("backup restore");
        assert!(matches!(
            restore.command,
            Command::Backup {
                command: BackupCommand::Restore { .. }
            }
        ));
        assert!(restore.yes);
        assert!(!restore.dry_run);
        assert!(restore.no_wait);
        assert!(
            Arguments::try_parse_from(["alcomd-cli", "--json", "--ndjson", "system", "status"])
                .is_err()
        );
    }

    #[test]
    fn command_catalog_matches_reachable_commands_and_aliases() {
        use clap::CommandFactory;
        use std::collections::BTreeSet;

        fn leaves(group: &clap::Command, prefix: &str, result: &mut BTreeSet<String>) {
            for child in group.get_subcommands() {
                for spelling in std::iter::once(child.get_name()).chain(child.get_all_aliases()) {
                    let path = format!("{prefix}{spelling}");
                    if child.has_subcommands() {
                        leaves(child, &format!("{path} "), result);
                    } else {
                        assert!(result.insert(path), "ambiguous command alias");
                    }
                }
            }
        }

        let catalog: serde_json::Value =
            serde_json::from_str(include_str!("../../../specs/cli/command-catalog-v1.json"))
                .expect("valid CLI contract");
        let root = Arguments::command();
        let groups = catalog["groups"].as_array().expect("command groups");
        assert_eq!(groups.len(), root.get_subcommands().count());
        for entry in groups {
            let group = root
                .find_subcommand(entry["name"].as_str().expect("group name"))
                .expect("implemented CLI group");
            let aliases = entry["aliases"]
                .as_array()
                .expect("group aliases")
                .iter()
                .map(|alias| alias.as_str().expect("group alias"))
                .collect::<BTreeSet<_>>();
            assert_eq!(group.get_all_aliases().collect::<BTreeSet<_>>(), aliases);
            let mut actual = BTreeSet::new();
            leaves(group, "", &mut actual);
            let expected = entry["commands"]
                .as_array()
                .expect("command list")
                .iter()
                .flat_map(|command| {
                    std::iter::once(command["name"].as_str().expect("command name"))
                        .chain(
                            command["aliases"]
                                .as_array()
                                .expect("command aliases")
                                .iter()
                                .map(|alias| alias.as_str().expect("command alias")),
                        )
                        .map(str::to_owned)
                })
                .collect::<BTreeSet<_>>();
            assert_eq!(actual, expected, "{}", group.get_name());
        }
    }

    #[test]
    fn core_catalog_groups_and_aliases_are_reachable() {
        assert!(Arguments::try_parse_from(["alcomd-cli", "operations", "list"]).is_ok());
        assert!(
            Arguments::try_parse_from([
                "alcomd-cli",
                "package",
                "i",
                "project",
                "package",
                "--expected-revision",
                "1",
                "--idempotency-key",
                "key",
                "--dry-run",
            ])
            .is_ok()
        );
        assert!(Arguments::try_parse_from(["alcomd-cli", "unity", "list"]).is_ok());
        assert!(Arguments::try_parse_from(["alcomd-cli", "completion", "powershell"]).is_ok());
    }

    #[test]
    fn package_source_flags_produce_one_explicit_source_constraint() {
        for (flag, expected) in [
            (
                "--repository-id",
                alcomd_protocol::PackageSourceSelector::Repository {
                    repository_id: "source".into(),
                },
            ),
            (
                "--user-package-id",
                alcomd_protocol::PackageSourceSelector::UserPackage {
                    user_package_id: "source".into(),
                },
            ),
        ] {
            let mut args = vec![
                "alcomd-cli",
                "package",
                "install",
                "project",
                "pkg",
                "--expected-revision",
                "1",
                "--idempotency-key",
                "key",
                flag,
                "source",
            ];
            let parsed = Arguments::try_parse_from(&args).expect("one selected source");
            let Command::Package {
                command: PackageCommand::Install { source, .. },
            } = parsed.command
            else {
                panic!("expected an install");
            };
            assert_eq!(source.selector(), Some(expected));
            args.extend([
                if flag == "--repository-id" {
                    "--user-package-id"
                } else {
                    "--repository-id"
                },
                "other",
            ]);
            assert_eq!(
                Arguments::try_parse_from(args)
                    .expect_err("only one source may constrain the plan")
                    .kind(),
                clap::error::ErrorKind::ArgumentConflict
            );
        }
    }

    #[test]
    fn package_install_and_upgrade_accept_core_recommendations() {
        for action in ["install", "upgrade"] {
            let args = Arguments::try_parse_from([
                "alcomd-cli",
                "package",
                action,
                "project",
                "pkg",
                "--expected-revision",
                "1",
                "--idempotency-key",
                "example",
            ])
            .expect("optional version");
            assert!(matches!(
                args.command,
                Command::Package {
                    command: PackageCommand::Install {
                        version_range: None,
                        include_prerelease: None,
                        allow_unity_incompatible: false,
                        ..
                    } | PackageCommand::Upgrade {
                        version_range: None,
                        include_prerelease: None,
                        allow_unity_incompatible: false,
                        ..
                    }
                }
            ));
        }
    }

    #[test]
    fn package_unity_override_requires_an_explicit_target() {
        for action in ["install", "upgrade", "downgrade"] {
            let base = [
                "alcomd-cli",
                "package",
                action,
                "project",
                "pkg",
                "--expected-revision",
                "1",
                "--idempotency-key",
                "example",
                "--allow-unity-incompatible",
            ];
            assert!(Arguments::try_parse_from(base).is_err());
            let target: &[&str] = if action == "downgrade" {
                &["1.2.3"]
            } else {
                &["--version-range", "=1.2.3"]
            };
            let args = Arguments::try_parse_from(base.into_iter().chain(target.iter().copied()))
                .expect("explicit package version");
            assert!(matches!(
                args.command,
                Command::Package {
                    command: PackageCommand::Install {
                        allow_unity_incompatible: true,
                        ..
                    } | PackageCommand::Upgrade {
                        allow_unity_incompatible: true,
                        ..
                    } | PackageCommand::Downgrade {
                        allow_unity_incompatible: true,
                        ..
                    }
                }
            ));
        }
    }

    #[test]
    fn project_delete_directory_accepts_project_identity_and_replay_key() {
        let arguments = Arguments::try_parse_from([
            "alcomd-cli",
            "project",
            "delete-directory",
            "--project-id",
            "00000000-0000-4000-8000-000000000001",
            "--plan-idempotency-key",
            "frozen-delete-preview",
            "--dry-run",
            "--no-wait",
        ])
        .expect("Project Delete CLI");
        assert!(matches!(
            arguments.command,
            Command::Project {
                command: ProjectCommand::DeleteDirectory { .. }
            }
        ));
        assert!(arguments.dry_run);
        assert!(arguments.no_wait);
        assert!(
            Arguments::try_parse_from([
                "alcomd-cli",
                "project",
                "delete-directory",
                "--project-id",
                "00000000-0000-4000-8000-000000000001",
                "--plan-idempotency-key",
                "frozen-delete-preview",
                "--dry-run",
                "--path",
                "C:/arbitrary",
            ])
            .is_err()
        );
    }
}
