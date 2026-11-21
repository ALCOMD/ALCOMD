mod external_mcp;
mod litedb;
mod profile;
mod selection;
mod templates;

use std::collections::BTreeMap;
use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, ensure};
use clap::{Parser, Subcommand};
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

const SOURCE_VERSION: &str = "3.4.0";
const STATE_LIMIT: u64 = 32 * 1024 * 1024;
const MANIFEST_LIMIT: u64 = 1024 * 1024;
const ATTACHMENTS_LIMIT: u64 = 1024 * 1024 * 1024;

/// Read a stopped ALCOMD3 3.4.0 data root into v4 migration staging.
#[derive(Parser)]
#[command(name = "alcomd-migrate-v3", version, about)]
struct Arguments {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Inspect an isolated external MCP configuration for the v3.4.0 quick-setup shape.
    PreviewMcpClient(external_mcp::Arguments),
    /// Preview an audited category, or apply the exact confirmed selection.
    Select(selection::Arguments),
    /// Count valid source records without writing data or emitting profile values.
    Inventory {
        #[arg(long)]
        source_root: PathBuf,
    },
    /// Publish a native bundle to a new directory within private current-user storage.
    Export {
        #[arg(long)]
        source_root: PathBuf,
        #[arg(long)]
        output: PathBuf,
        /// Create current-user vault entries for supported repository credentials.
        #[arg(long)]
        transfer_credentials: bool,
        /// Bind new vault references to the Bootstrap migration transaction.
        #[arg(long, requires = "transfer_credentials")]
        credential_transfer_id: Option<Uuid>,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Manifest<'a> {
    schema: u32,
    source_version: &'a str,
    created_at: String,
    records: BTreeMap<&'static str, usize>,
    files: Vec<BundleFile>,
}

#[derive(Serialize)]
struct BundleFile {
    path: String,
    sha256: String,
    size: u64,
}

fn main() -> ExitCode {
    let arguments = Arguments::parse();
    match run(arguments.command) {
        Ok(counts) => {
            if serde_json::to_writer(io::stdout().lock(), &counts).is_err() {
                return ExitCode::FAILURE;
            }
            println!();
            ExitCode::SUCCESS
        }
        Err(error) => {
            // Parser and filesystem errors may contain arbitrary v3 strings or
            // absolute private paths. Keep command output independent of them.
            if let Some(reason) = error.downcast_ref::<templates::UnsupportedTemplatePackages>() {
                eprintln!("{reason}");
            } else {
                eprintln!(
                    "The migration input, destination, or requested credential transfer could not be completed."
                );
            }
            ExitCode::FAILURE
        }
    }
}

fn run(command: Command) -> Result<serde_json::Value> {
    match command {
        Command::PreviewMcpClient(arguments) => external_mcp::preview(arguments),
        Command::Select(arguments) => selection::run(arguments),
        Command::Inventory { source_root } => {
            let source = profile::read(&source_root)?;
            report_skipped_templates(source.skipped_templates);
            Ok(serde_json::to_value(source.native.counts())?)
        }
        Command::Export {
            source_root,
            output,
            transfer_credentials,
            credential_transfer_id,
        } => Ok(serde_json::to_value(export(
            &source_root,
            &output,
            transfer_credentials,
            credential_transfer_id,
        )?)?),
    }
}

fn report_skipped_templates(count: usize) {
    if count != 0 {
        eprintln!(
            "Skipped {count} derived template(s) incompatible with Unity {}.",
            templates::DERIVED_EDITOR
        );
    }
}

fn export(
    source_root: &Path,
    output: &Path,
    transfer_credentials: bool,
    credential_transfer_id: Option<Uuid>,
) -> Result<BTreeMap<&'static str, usize>> {
    let mut source = profile::read(source_root)?;
    let mut vault_writes = Vec::new();
    if transfer_credentials {
        for (repository, credential) in source
            .native
            .repositories
            .iter_mut()
            .zip(&source.credentials)
        {
            if let Some(credential) = credential {
                let bytes = profile::credential_bytes(credential)?;
                let index = u32::try_from(vault_writes.len()).context("credential index")?;
                let reference = credential_transfer_id
                    .map_or_else(Uuid::new_v4, |id| {
                        alcomd_import::migration_credential_reference(id, index)
                    })
                    .to_string();
                match alcomd_platform::read_credential(&reference) {
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    _ => anyhow::bail!("new credential reference is unavailable"),
                }
                repository.credential_ref = Some(reference.clone());
                vault_writes.push((reference, bytes));
            }
        }
    }

    ensure!(
        output.is_absolute()
            && !output
                .components()
                .any(|part| matches!(part, Component::CurDir | Component::ParentDir)),
        "invalid absolute staging destination"
    );
    let leaf = output
        .file_name()
        .context("missing staging destination name")?;
    let directory = output.parent().context("missing staging parent")?;
    let (directory, directory_id) = alcomd_platform::resolve_plain_storage_directory(directory)?;
    let (source_root, _) = alcomd_platform::resolve_plain_storage_directory(source_root)?;
    let output = directory.join(leaf);
    ensure!(
        !output.starts_with(&source_root) && !directory.starts_with(&source_root),
        "output cannot be within source profile"
    );
    match fs::symlink_metadata(&output) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        _ => anyhow::bail!("bundle destination already exists or is unavailable"),
    }
    let mut stage = Stage::create(&directory)?;
    let path = &stage.path;
    let mut files = Vec::new();
    if !source.template_files.is_empty() {
        let templates = path.join("templates");
        fs::create_dir(&templates)?;
        let mut attachment_bytes = 0_u64;
        for archive in source.template_files {
            archive.convert(&source_root, path)?;
            let entry = file_summary(path, archive.relative())?;
            attachment_bytes = attachment_bytes
                .checked_add(entry.size)
                .context("bundle attachments exceed their total limit")?;
            ensure!(
                attachment_bytes <= ATTACHMENTS_LIMIT,
                "bundle attachments exceed their total limit"
            );
            files.push(entry);
        }
        alcomd_platform::sync_directory(&templates)?;
    }
    write_json(path, "state.json", &source.native, STATE_LIMIT)?;
    files.push(file_summary(path, "state.json")?);
    files.sort_by(|left, right| left.path.cmp(&right.path));
    let records = source.native.counts();
    let manifest = Manifest {
        schema: 1,
        source_version: SOURCE_VERSION,
        created_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        records: records.clone(),
        files,
    };
    write_json(path, "manifest.json", &manifest, MANIFEST_LIMIT)?;
    alcomd_platform::sync_directory(path)?;
    let (_, current_id) = alcomd_platform::resolve_plain_storage_directory(&directory)?;
    ensure!(
        directory_id == current_id && stage.current(),
        "staging destination was replaced"
    );
    alcomd_platform::publish_path_no_replace(path, &output)?;
    stage.published = true;
    alcomd_platform::sync_directory(&directory)?;

    // Persist the opaque references above before making any vault writes. A
    // bridge that stops here can inspect the known output and recover exactly
    // these references. The old profile and its credentials are never updated.
    let mut completed = Vec::<String>::new();
    for (reference, bytes) in vault_writes {
        if let Err(error) = alcomd_platform::write_credential(&reference, &bytes) {
            for reference in &completed {
                let _ = alcomd_platform::delete_credential(reference);
            }
            return Err(error).context("credential transfer failed");
        }
        completed.push(reference);
    }
    report_skipped_templates(source.skipped_templates);
    Ok(records)
}

fn write_json(root: &Path, name: &str, value: &impl Serialize, limit: u64) -> Result<()> {
    let data = serde_json::to_vec(value)?;
    ensure!(
        data.len() as u64 <= limit,
        "native bundle item exceeds its byte limit"
    );
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(root.join(name))?;
    file.write_all(&data)?;
    file.sync_all()?;
    Ok(())
}

fn file_summary(root: &Path, name: &str) -> Result<BundleFile> {
    let mut file = File::open(root.join(name))?;
    let size = file.metadata()?.len();
    let mut bytes = [0; 65536];
    let mut sha = Sha256::new();
    loop {
        let read = file.read(&mut bytes)?;
        if read == 0 {
            break;
        }
        sha.update(&bytes[..read]);
    }
    Ok(BundleFile {
        path: name.into(),
        sha256: hex_digest(&sha.finalize()),
        size,
    })
}

struct Stage {
    path: PathBuf,
    identity: Vec<u8>,
    published: bool,
}

impl Stage {
    fn create(parent: &Path) -> Result<Self> {
        let path = parent.join(format!(".alcomd-migration-{}", Uuid::new_v4()));
        let builder = &mut DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&path)?;
        let (path, identity) = alcomd_platform::resolve_plain_storage_directory(&path)?;
        Ok(Self {
            path,
            identity,
            published: false,
        })
    }

    fn current(&self) -> bool {
        alcomd_platform::resolve_plain_storage_directory(&self.path)
            .is_ok_and(|(_, identity)| identity == self.identity)
    }
}

impl Drop for Stage {
    fn drop(&mut self) {
        if !self.published && self.current() {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

fn hex_digest(digest: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(encoded, "{byte:02x}").expect("writing to String");
    }
    encoded
}
