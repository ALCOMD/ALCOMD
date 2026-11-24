//! Read-only classification of v3.4.0 quick-setup transport shapes.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, ensure};
use clap::{Args, ValueEnum};
use serde::{Deserialize, Serialize, de::IgnoredAny};
use serde_json::{Value, json, value::RawValue};
use sha2::{Digest, Sha256};

const ENTRY: &str = "alcomd3";
const TOKEN_VARIABLE: &str = "ALCOMD3_MCP_BEARER_TOKEN";
const MAX_CONFIG_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Args)]
pub(super) struct Arguments {
    /// An explicitly selected, isolated directory containing both fixture files.
    #[arg(long)]
    fixture_root: PathBuf,
    /// Relative path of the stopped v3 profile's gui-config.json.
    #[arg(long)]
    v3_gui_config: PathBuf,
    #[arg(long, value_enum)]
    client: Client,
    /// Relative path of the selected client's fixture configuration.
    #[arg(long)]
    client_config: PathBuf,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "camelCase")]
enum Client {
    Codex,
    ClaudeCode,
    Cursor,
}

impl Client {
    fn filename(self) -> &'static str {
        match self {
            Self::Codex => "config.toml",
            Self::ClaudeCode => ".claude.json",
            Self::Cursor => "mcp.json",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
enum Status {
    Absent,
    ExactV3Transport,
    Unverified,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProfilePort {
    mcp_http_port: Option<u16>,
}

#[derive(Deserialize, Default)]
struct JsonServers {
    #[serde(default)]
    alcomd3: JsonCandidate,
}

#[derive(Deserialize)]
struct JsonConfig {
    #[serde(default, rename = "mcpServers")]
    servers: JsonServers,
}

#[derive(Deserialize, Default)]
#[serde(untagged)]
enum JsonCandidate {
    #[default]
    #[serde(skip)]
    Absent,
    Http(JsonHttp),
    Other(IgnoredAny),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonHttp {
    #[serde(rename = "type")]
    transport: String,
    url: String,
    headers: JsonHeaders,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonHeaders {
    #[serde(rename = "Authorization")]
    authorization: String,
}

pub(super) fn preview(args: Arguments) -> Result<serde_json::Value> {
    let (root, identity) = alcomd_platform::resolve_plain_storage_directory(&args.fixture_root)?;
    validate_paths(&args)?;
    let endpoint = endpoint(&root, &args.v3_gui_config)?;
    let status = match super::profile::read_file(&root, &args.client_config, MAX_CONFIG_BYTES)? {
        Some(contents) => classify(args.client, without_bom(&contents), endpoint.as_deref())?,
        None => Status::Absent,
    };
    ensure!(
        alcomd_platform::resolve_plain_storage_directory(&root)?.1 == identity,
        "fixture directory changed during inspection"
    );
    Ok(json!({"client": args.client, "status": status, "ownershipVerified": false}))
}

fn validate_paths(args: &Arguments) -> Result<()> {
    ensure!(
        args.v3_gui_config
            .file_name()
            .is_some_and(|name| name == "gui-config.json")
            && args
                .client_config
                .file_name()
                .is_some_and(|name| name == args.client.filename()),
        "unexpected client or profile filename"
    );
    Ok(())
}

fn endpoint(root: &Path, profile: &Path) -> Result<Option<String>> {
    let port = super::profile::read_file(root, profile, MAX_CONFIG_BYTES)?
        .map(|bytes| serde_json::from_slice::<ProfilePort>(without_bom(&bytes)))
        .transpose()
        .context("invalid legacy GUI config")?
        .and_then(|profile| profile.mcp_http_port)
        .filter(|port| *port != 0);
    Ok(port.map(|port| format!("http://127.0.0.1:{port}/mcp")))
}

fn without_bom(contents: &[u8]) -> &[u8] {
    contents.strip_prefix(b"\xef\xbb\xbf").unwrap_or(contents)
}

fn classify(client: Client, contents: &[u8], endpoint: Option<&str>) -> Result<Status> {
    let source = std::str::from_utf8(contents).context("client config must be UTF-8")?;
    if source.trim().is_empty() {
        return Ok(Status::Absent);
    }
    let is_exact = match client {
        Client::Codex => {
            let document: toml::Table = toml::from_str(source).context("invalid Codex config")?;
            let Some(servers) = document.get("mcp_servers") else {
                return Ok(Status::Absent);
            };
            let servers = servers.as_table().context("invalid Codex MCP table")?;
            let Some(entry) = servers.get(ENTRY) else {
                return Ok(Status::Absent);
            };
            entry.as_table().is_some_and(|fields| {
                fields.len() == 2
                    && endpoint.is_some_and(|endpoint| {
                        fields.get("url").and_then(toml::Value::as_str) == Some(endpoint)
                    })
                    && fields
                        .get("bearer_token_env_var")
                        .and_then(toml::Value::as_str)
                        == Some(TOKEN_VARIABLE)
            })
        }
        Client::ClaudeCode | Client::Cursor => {
            let document: JsonConfig =
                serde_json::from_str(source).context("invalid MCP JSON config")?;
            let authorization = match client {
                Client::ClaudeCode => format!("Bearer {dollar}{{{TOKEN_VARIABLE}}}", dollar = '$'),
                Client::Cursor => format!("Bearer {dollar}{{env:{TOKEN_VARIABLE}}}", dollar = '$'),
                Client::Codex => unreachable!(),
            };
            match document.servers.alcomd3 {
                JsonCandidate::Absent => return Ok(Status::Absent),
                JsonCandidate::Http(entry) => {
                    entry.transport == "http"
                        && endpoint == Some(entry.url.as_str())
                        && entry.headers.authorization == authorization
                }
                JsonCandidate::Other(_) => false,
            }
        }
    };
    Ok(if is_exact {
        Status::ExactV3Transport
    } else {
        Status::Unverified
    })
}

#[derive(Args)]
pub(super) struct PatchArguments {
    #[command(flatten)]
    selection: Arguments,
    /// Relative path of the same client file captured before v3 quick setup.
    #[arg(long)]
    before_quick_setup: PathBuf,
    /// Name of a private transaction directory directly beneath the fixture root.
    #[arg(long)]
    journal: PathBuf,
    /// Apply the exact digest produced by review.
    #[arg(long)]
    apply_plan: Option<String>,
}

#[derive(Args)]
pub(super) struct RollbackArguments {
    #[arg(long)]
    fixture_root: PathBuf,
    #[arg(long)]
    journal: PathBuf,
    #[arg(long)]
    plan_digest: String,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FileState {
    identity: Vec<u8>,
    sha256: [u8; 32],
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PatchPlan {
    digest: String,
    client: Client,
    target: PathBuf,
    root_identity: Vec<u8>,
    parent_identity: Vec<u8>,
    original: FileState,
    replacement: FileState,
}

const ORIGINAL: &str = "original.bin";
const REPLACEMENT: &str = "replacement.bin";
const PLAN: &str = "plan.json";
const LOCK: &str = "transaction.lock";

pub(super) fn patch(args: PatchArguments) -> Result<Value> {
    let selection = args.selection;
    validate_paths(&selection)?;
    let (root, root_identity) =
        alcomd_platform::resolve_plain_storage_directory(&selection.fixture_root)?;
    let journal = journal_path(&root, &args.journal)?;
    let target = selection.client_config;
    let before = args.before_quick_setup;
    ensure!(
        before != target
            && before
                .file_name()
                .is_some_and(|name| name == selection.client.filename())
            && [&target, &before, &selection.v3_gui_config]
                .iter()
                .all(|path| !path.starts_with(&args.journal)),
        "invalid fixture evidence or journal location"
    );
    if let Some(approved) = args.apply_plan.as_deref()
        && fs::symlink_metadata(&journal).is_ok()
    {
        return finish_existing(
            &root,
            &journal,
            approved,
            Some((selection.client, &target)),
            false,
        );
    }
    let endpoint = endpoint(&root, &selection.v3_gui_config)?;
    let earlier = super::profile::read_file(&root, &before, MAX_CONFIG_BYTES)?
        .context("missing before-setup snapshot")?;
    let config_path = root.join(&target);
    let (_, parent_identity) =
        alcomd_platform::resolve_plain_storage_directory(parent(&config_path)?)?;
    let (original_file, original_id) = alcomd_platform::open_plain_file(&config_path)?;
    let original_bytes = super::profile::read_file(&root, &target, MAX_CONFIG_BYTES)?
        .context("missing selected client configuration")?;
    let original = FileState {
        identity: original_id,
        sha256: Sha256::digest(&original_bytes).into(),
    };
    ensure!(
        classify(
            selection.client,
            without_bom(&original_bytes),
            endpoint.as_deref()
        )? == Status::ExactV3Transport,
        "selected config is not exact v3 transport"
    );
    ensure!(
        classify(selection.client, without_bom(&earlier), endpoint.as_deref())? == Status::Absent,
        "v3 entry already existed in the before-setup snapshot"
    );
    let replacement = remove_entry(selection.client, &original_bytes)?;
    ensure!(
        classify(
            selection.client,
            without_bom(&replacement),
            endpoint.as_deref()
        )? == Status::Absent
            && equivalent_after_setup(selection.client, &earlier, &replacement)?,
        "before-setup evidence does not match the non-v3 entries"
    );
    let before_digest: [u8; 32] = Sha256::digest(&earlier).into();
    let replacement_digest: [u8; 32] = Sha256::digest(&replacement).into();
    let plan_bytes = serde_json::to_vec(&(
        "alcomd-external-mcp-patch-v1",
        &root_identity,
        &parent_identity,
        selection.client,
        &target,
        &args.journal,
        &original,
        before_digest,
        replacement_digest,
    ))?;
    let digest = crate::hex_digest(&Sha256::digest(plan_bytes));
    ensure!(
        alcomd_platform::resolve_plain_storage_directory(&root)?.1 == root_identity,
        "fixture directory changed during review"
    );
    let Some(approved) = args.apply_plan.as_deref() else {
        return Ok(
            json!({"client": selection.client, "planDigest": digest, "beforeSetupSnapshotMatched": true}),
        );
    };
    ensure!(
        approved == digest,
        "client configuration no longer matches the approved patch"
    );
    let mut stage = crate::Stage::create(&root)?;
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(stage.path.join(LOCK))?
        .sync_all()?;
    let replacement_path = stage.path.join(REPLACEMENT);
    let mut replacement_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&replacement_path)?;
    replacement_file.write_all(&replacement)?;
    replacement_file.set_permissions(original_file.metadata()?.permissions())?;
    replacement_file.sync_all()?;
    drop((replacement_file, original_file));
    let plan = PatchPlan {
        digest,
        client: selection.client,
        target: target.clone(),
        root_identity,
        parent_identity,
        original,
        replacement: file_state(&replacement_path)?.context("prepared replacement disappeared")?,
    };
    crate::write_json(&stage.path, PLAN, &plan, 8192)?;
    alcomd_platform::sync_directory(&stage.path)?;
    ensure!(
        stage.current()
            && alcomd_platform::resolve_plain_storage_directory(&root)?.1 == plan.root_identity,
        "patch staging was replaced"
    );
    match alcomd_platform::publish_path_no_replace(&stage.path, &journal) {
        Ok(()) => {
            stage.published = true;
            alcomd_platform::sync_directory(&root)?;
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.into()),
    }
    finish_existing(
        &root,
        &journal,
        approved,
        Some((selection.client, &target)),
        false,
    )
}

pub(super) fn rollback(args: RollbackArguments) -> Result<Value> {
    let (root, _) = alcomd_platform::resolve_plain_storage_directory(&args.fixture_root)?;
    let journal = journal_path(&root, &args.journal)?;
    finish_existing(&root, &journal, &args.plan_digest, None, true)
}

fn parent(path: &Path) -> Result<&Path> {
    path.parent().context("missing patch directory")
}

fn journal_path(root: &Path, name: &Path) -> Result<PathBuf> {
    ensure!(
        matches!(name.components().next(), Some(Component::Normal(_)))
            && name.components().count() == 1,
        "patch journal must be a single directory name"
    );
    Ok(root.join(name))
}

fn file_state(path: &Path) -> Result<Option<FileState>> {
    let identity = match alcomd_platform::open_plain_file(path) {
        Ok((_, identity)) => identity,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let contents = super::profile::read_file(
        parent(path)?,
        Path::new(path.file_name().context("missing file name")?),
        MAX_CONFIG_BYTES,
    )?
    .context("patch file disappeared")?;
    ensure!(
        alcomd_platform::open_plain_file(path)?.1 == identity,
        "patch file changed"
    );
    Ok(Some(FileState {
        identity,
        sha256: Sha256::digest(contents).into(),
    }))
}

fn finish_existing(
    root: &Path,
    journal: &Path,
    approved: &str,
    selection: Option<(Client, &Path)>,
    rollback: bool,
) -> Result<Value> {
    let (_, journal_identity) = alcomd_platform::resolve_plain_storage_directory(journal)?;
    let lock_path = journal.join(LOCK);
    let (_, lock_identity) = alcomd_platform::open_plain_file(&lock_path)?;
    let lock = OpenOptions::new().read(true).write(true).open(&lock_path)?;
    ensure!(
        alcomd_platform::open_plain_file(&lock_path)?.1 == lock_identity,
        "patch lock changed"
    );
    lock.try_lock()
        .context("patch transaction is already in use")?;
    let record =
        super::profile::read_file(journal, PLAN, 8192)?.context("patch journal is incomplete")?;
    let plan: PatchPlan = serde_json::from_slice(&record)?;
    ensure!(
        plan.digest == approved
            && selection
                .is_none_or(|(client, target)| client == plan.client && target == plan.target),
        "patch journal does not match the approved plan"
    );
    ensure!(
        plan.target
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
            && plan
                .target
                .file_name()
                .is_some_and(|name| name == plan.client.filename())
            && !root.join(&plan.target).starts_with(journal),
        "invalid patch target in journal"
    );
    let target = root.join(&plan.target);
    let check_directories = || -> Result<()> {
        ensure!(
            alcomd_platform::resolve_plain_storage_directory(root)?.1 == plan.root_identity
                && alcomd_platform::resolve_plain_storage_directory(journal)?.1 == journal_identity
                && alcomd_platform::resolve_plain_storage_directory(parent(&target)?)?.1
                    == plan.parent_identity,
            "patch directories changed"
        );
        Ok(())
    };
    check_directories()?;
    let (from_name, to_name, from, to) = if rollback {
        (REPLACEMENT, ORIGINAL, &plan.replacement, &plan.original)
    } else {
        (ORIGINAL, REPLACEMENT, &plan.original, &plan.replacement)
    };
    let parked = journal.join(from_name);
    let prepared = journal.join(to_name);
    let current = file_state(&target)?;
    if current.as_ref() == Some(to) {
        ensure!(
            file_state(&prepared)?.is_none() && file_state(&parked)?.as_ref() == Some(from),
            "duplicate or changed patch state"
        );
    } else {
        ensure!(
            file_state(&prepared)?.as_ref() == Some(to),
            "prepared patch state changed"
        );
        if current.as_ref() == Some(from) {
            ensure!(
                file_state(&parked)?.is_none(),
                "original patch state changed"
            );
            check_directories()?;
            alcomd_platform::publish_path_no_replace(&target, &parked)?;
            alcomd_platform::sync_directory(journal)?;
            alcomd_platform::sync_directory(parent(&target)?)?;
        } else {
            ensure!(
                current.is_none(),
                "client configuration was modified outside this patch"
            );
        }
        if file_state(&parked)?.as_ref() != Some(from) {
            // If an external writer won the race before the move, restore the
            // moved file without clobbering anything the writer has since created.
            check_directories()?;
            if file_state(&parked)?.is_some() && file_state(&target)?.is_none() {
                alcomd_platform::publish_path_no_replace(&parked, &target)?;
                alcomd_platform::sync_directory(journal)?;
                alcomd_platform::sync_directory(parent(&target)?)?;
            }
            anyhow::bail!("parked patch state changed");
        }
        check_directories()?;
        alcomd_platform::publish_path_no_replace(&prepared, &target)?;
    }
    alcomd_platform::sync_directory(journal)?;
    alcomd_platform::sync_directory(parent(&target)?)?;
    check_directories()?;
    ensure!(
        file_state(&target)?.as_ref() == Some(to),
        "client configuration changed after patch"
    );
    Ok(
        json!({"client": plan.client, "planDigest": approved, "status": if rollback { "rolledBack" } else { "applied" }}),
    )
}

fn equivalent_after_setup(client: Client, before: &[u8], patched: &[u8]) -> Result<bool> {
    let before = std::str::from_utf8(without_bom(before))?;
    let patched = std::str::from_utf8(without_bom(patched))?;
    match client {
        Client::Codex => {
            let parse = |text: &str| -> Result<toml::Table> {
                let mut document: toml::Table = toml::from_str(text)?;
                if document
                    .get("mcp_servers")
                    .and_then(toml::Value::as_table)
                    .is_some_and(toml::Table::is_empty)
                {
                    document.remove("mcp_servers");
                }
                Ok(document)
            };
            Ok(parse(before)? == parse(patched)?)
        }
        Client::ClaudeCode | Client::Cursor => {
            let parse = |text: &str| -> Result<Value> {
                let mut document: serde_json::Map<String, Value> = serde_json::from_str(text)?;
                if document
                    .get("mcpServers")
                    .and_then(Value::as_object)
                    .is_some_and(serde_json::Map::is_empty)
                {
                    document.remove("mcpServers");
                }
                Ok(Value::Object(document))
            };
            Ok(parse(before)? == parse(patched)?)
        }
    }
}

fn remove_entry(client: Client, contents: &[u8]) -> Result<Vec<u8>> {
    let text = std::str::from_utf8(without_bom(contents))?;
    let mut patched = contents[..contents.len() - text.len()].to_vec();
    match client {
        Client::Codex => {
            let mut matching = text
                .split_inclusive('\n')
                .filter(|line| line.trim() == "[mcp_servers.alcomd3]");
            ensure!(
                matching.next().is_some() && matching.next().is_none(),
                "unrecognized Codex entry layout"
            );
            let mut within = false;
            let mut keys = 0;
            for line in text.split_inclusive('\n') {
                let trimmed = line.trim();
                if trimmed.starts_with('[') {
                    within = trimmed == "[mcp_servers.alcomd3]";
                    if within {
                        continue;
                    }
                }
                if within && !trimmed.is_empty() && !trimmed.starts_with('#') {
                    let (key, _) = trimmed
                        .split_once('=')
                        .context("unrecognized Codex entry assignment")?;
                    ensure!(
                        matches!(key.trim(), "url" | "bearer_token_env_var") && !line.contains('#'),
                        "unexpected Codex entry decoration"
                    );
                    keys += 1;
                } else {
                    patched.extend_from_slice(line.as_bytes());
                }
            }
            ensure!(keys == 2, "unexpected Codex entry assignment count");
        }
        Client::ClaudeCode | Client::Cursor => {
            #[derive(Deserialize)]
            struct Root<'a> {
                #[serde(borrow, rename = "mcpServers")]
                servers: Servers<'a>,
            }
            #[derive(Deserialize)]
            struct Servers<'a> {
                #[serde(borrow)]
                alcomd3: &'a RawValue,
            }
            let entry = serde_json::from_str::<Root<'_>>(text)?
                .servers
                .alcomd3
                .get();
            let value_start = entry.as_ptr() as usize - text.as_ptr() as usize;
            let value_end = value_start + entry.len();
            let key_end = text[..value_start]
                .trim_end()
                .strip_suffix(':')
                .context("missing client key separator")?
                .trim_end();
            let key_start = key_end
                .strip_suffix("\"alcomd3\"")
                .context("unrecognized client key layout")?
                .len();
            let before = text[..key_start].trim_end();
            let mut after = &text[value_end..];
            let from = if let Some(remaining) = before.strip_suffix(',') {
                remaining.len()
            } else {
                ensure!(before.ends_with('{'), "unexpected client member separator");
                if after.trim_start().starts_with(',') {
                    after = &after.trim_start()[1..];
                }
                key_start
            };
            patched.extend_from_slice(&text.as_bytes()[..from]);
            patched.extend_from_slice(after.as_bytes());
        }
    }
    Ok(patched)
}
