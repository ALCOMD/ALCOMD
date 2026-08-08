use crate::alcomd3_config::{Alcomd3Config, UpdaterManifest};
use crate::utils::command::{CommandExt, create_command};
use anyhow::{Context, Result, bail};
use chrono::NaiveDate;
use clap::ValueEnum;
use semver::Version;
use serde::Deserialize;
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use toml_edit::{DocumentMut, value};

const DEFAULT_TARGET: &str = "x86_64-pc-windows-msvc";
pub const GH_TOKEN_ENV: &str = "GH_TOKEN";
pub const GITHUB_TOKEN_ENV: &str = "GITHUB_TOKEN";
pub const UPDATER_PRIVATE_KEY_ENV: &str = "ALCOMD3_UPDATER_PRIVATE_KEY";
pub const UPDATER_PRIVATE_KEY_PASSWORD_ENV: &str = "ALCOMD3_UPDATER_PRIVATE_KEY_PASSWORD";
const GITHUB_ACTIONS_ENV: &str = "GITHUB_ACTIONS";
const GITHUB_EVENT_NAME_ENV: &str = "GITHUB_EVENT_NAME";
const GITHUB_REF_ENV: &str = "GITHUB_REF";
const GITHUB_REPOSITORY_ENV: &str = "GITHUB_REPOSITORY";
const GITHUB_SHA_ENV: &str = "GITHUB_SHA";
const GITHUB_WORKFLOW_REF_ENV: &str = "GITHUB_WORKFLOW_REF";
const RELEASE_DRAFT_WORKFLOW: &str = ".github/workflows/release-draft.yml";
const RELEASE_UPDATER_WORKFLOW: &str = ".github/workflows/release-updater.yml";
const GITHUB_RELEASE_LOCALIZED_CHANGELOG_SOURCES: [(&str, &str, &str); 2] = [
    ("日本語", "CHANGELOG/CHANGELOG.ja.md", "# 変更履歴"),
    ("中文", "CHANGELOG/CHANGELOG.zh-CN.md", "# 更新日志"),
];

#[derive(Debug, Eq, PartialEq)]
struct ChangelogReleaseShape {
    date: NaiveDate,
    categories: Vec<(String, usize)>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum ReleaseChannel {
    Stable,
    Beta,
}

impl ReleaseChannel {
    pub fn is_prerelease(self) -> bool {
        matches!(self, Self::Beta)
    }

    pub fn updater_manifest<'a>(self, config: &'a Alcomd3Config) -> &'a UpdaterManifest {
        match self {
            Self::Stable => config.stable_updater_manifest(),
            Self::Beta => config.beta_updater_manifest(),
        }
    }

    pub fn updater_endpoint(self, config: &Alcomd3Config, site_base_url: &str) -> String {
        let suffix = &self.updater_manifest(config).public_path;
        format!(
            "{}/{}",
            site_base_url.trim_end_matches('/'),
            suffix.trim_start_matches('/')
        )
    }
}

impl fmt::Display for ReleaseChannel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stable => f.write_str("stable"),
            Self::Beta => f.write_str("beta"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum UpdaterSignaturePurpose {
    LocalTest,
    Release,
}

impl fmt::Display for UpdaterSignaturePurpose {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LocalTest => f.write_str("local-test"),
            Self::Release => f.write_str("release"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReleaseAutomation {
    Draft,
    Updater,
}

impl ReleaseAutomation {
    fn event_name(self) -> &'static str {
        match self {
            Self::Draft => "workflow_dispatch",
            Self::Updater => "release",
        }
    }

    fn workflow_path(self) -> &'static str {
        match self {
            Self::Draft => RELEASE_DRAFT_WORKFLOW,
            Self::Updater => RELEASE_UPDATER_WORKFLOW,
        }
    }

    fn expected_ref(self, ctx: &ReleaseContext) -> String {
        match self {
            Self::Draft => "refs/heads/main".to_string(),
            Self::Updater => format!("refs/tags/{}", ctx.tag),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ReleaseContext {
    pub version: String,
    pub channel: ReleaseChannel,
    pub repo: String,
    pub workspace_root: PathBuf,
    pub tag: String,
    pub changelog: PathBuf,
    pub updater_json: PathBuf,
    pub updater_endpoint: String,
    pub config: Alcomd3Config,
}

impl ReleaseContext {
    pub fn new(
        version: impl Into<String>,
        channel: ReleaseChannel,
        repo: Option<String>,
        site_base_url: Option<String>,
        _target: Option<String>,
    ) -> Result<Self> {
        let version = version.into();
        validate_version_for_channel(&version, channel)?;

        let metadata = crate::utils::cargo::cargo_metadata();
        let workspace_root = metadata.workspace_root.as_std_path().to_path_buf();
        let config = Alcomd3Config::load_from_workspace(&workspace_root)?;

        let repo = repo.unwrap_or_else(|| config.repository.clone());
        let site_base_url = site_base_url.unwrap_or_else(|| config.site_base_url().to_string());
        let tag = format!("v{version}");
        let changelog = workspace_root.join("CHANGELOG.md");
        let updater_json = config.workspace_path(
            &channel.updater_manifest(&config).output_path,
            &workspace_root,
        );
        let updater_endpoint = channel.updater_endpoint(&config, &site_base_url);

        Ok(Self {
            version,
            channel,
            repo,
            workspace_root,
            tag,
            changelog,
            updater_json,
            updater_endpoint,
            config,
        })
    }

    pub fn artifact_dir(&self) -> PathBuf {
        self.workspace_root
            .join("artifacts")
            .join("release")
            .join(&self.tag)
    }

    pub fn local_test_artifact_dir(&self) -> PathBuf {
        self.workspace_root
            .join("artifacts")
            .join("local-test")
            .join(&self.tag)
    }

    pub fn release_build_manifest(&self) -> PathBuf {
        self.workspace_root
            .join("artifacts")
            .join("release-state")
            .join(format!("{}.json", self.tag))
    }

    pub fn release_build_shard_dir(&self) -> PathBuf {
        self.workspace_root
            .join("artifacts")
            .join("release-state")
            .join(&self.tag)
    }

    pub fn resolved_release_platforms(
        &self,
    ) -> Vec<crate::release_assets::ResolvedReleasePlatform> {
        crate::release_assets::resolve_release_platforms(
            &self.config,
            &self.workspace_root,
            &self.version,
        )
    }

    pub fn expected_public_asset_names(&self) -> Vec<String> {
        crate::release_assets::expected_public_asset_names(&self.resolved_release_platforms())
    }

    pub fn artifact_path(&self, name: &str) -> PathBuf {
        self.artifact_dir().join(name)
    }

    pub fn release_check_dir(&self) -> PathBuf {
        self.workspace_root
            .join("artifacts")
            .join("release-check")
            .join(&self.tag)
    }

    pub fn release_body(&self) -> PathBuf {
        self.release_check_dir().join("github-release.md")
    }

    pub fn release_title(&self) -> String {
        format!("Version {}", self.version)
    }

    pub fn updater_notes(&self) -> PathBuf {
        self.workspace_root
            .join("release-metadata")
            .join("updater-notes")
            .join(format!("{}.json", self.version))
    }
}

pub struct CmdRunner {
    dry_run: bool,
}

impl CmdRunner {
    pub fn new(dry_run: bool) -> Self {
        Self { dry_run }
    }

    pub fn dry_run(&self) -> bool {
        self.dry_run
    }

    pub fn run(&self, mut cmd: ProcessCommand, what: &str) -> Result<()> {
        println!("$ {}", cmd.display_command());
        if self.dry_run {
            return Ok(());
        }
        cmd.run_checked(what)
    }

    pub fn capture(&self, mut cmd: ProcessCommand, what: &str) -> Result<String> {
        println!("$ {}", cmd.display_command());
        if self.dry_run {
            return Ok(String::new());
        }
        cmd.run_capture_checked(what)
    }
}

pub fn default_repo() -> String {
    Alcomd3Config::load()
        .map(|config| config.repository)
        .unwrap_or_else(|error| panic!("failed to load alcomd3.config.json: {error:#}"))
}

pub fn default_site_base_url() -> String {
    Alcomd3Config::load()
        .map(|config| config.site_base_url().to_string())
        .unwrap_or_else(|error| panic!("failed to load alcomd3.config.json: {error:#}"))
}

pub fn validate_release_source_versions(
    expected: &str,
    workspace_versions: &[String],
    gui_version: &str,
) -> Result<()> {
    for version in workspace_versions {
        if version != expected {
            bail!("workspace package version mismatch: expected {expected}, got {version}");
        }
    }
    if gui_version != expected {
        bail!("vrc-get-gui package version mismatch: expected {expected}, got {gui_version}");
    }
    Ok(())
}

pub fn default_target() -> String {
    DEFAULT_TARGET.to_string()
}

pub fn validate_version_for_channel(version: &str, channel: ReleaseChannel) -> Result<()> {
    let parsed = Version::parse(version).with_context(|| format!("invalid SemVer: {version}"))?;

    match channel {
        ReleaseChannel::Stable if !parsed.pre.is_empty() => {
            bail!("stable release version must not contain prerelease metadata: {version}")
        }
        ReleaseChannel::Beta if parsed.pre.is_empty() => {
            bail!("beta release version must contain prerelease metadata: {version}")
        }
        _ => Ok(()),
    }
}

pub fn validate_full_git_sha(source_sha: &str) -> Result<()> {
    if source_sha.len() != 40 || !source_sha.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("release source SHA must be a full 40-character hexadecimal commit ID");
    }
    Ok(())
}

pub fn ensure_github_actions_context(
    ctx: &ReleaseContext,
    automation: ReleaseAutomation,
    source_sha: &str,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        println!(
            "require GitHub Actions context: {} on {} at {source_sha}",
            automation.workflow_path(),
            automation.event_name()
        );
        return Ok(());
    }

    let github_actions = required_env(GITHUB_ACTIONS_ENV)?;
    let event_name = required_env(GITHUB_EVENT_NAME_ENV)?;
    let github_ref = required_env(GITHUB_REF_ENV)?;
    let repository = required_env(GITHUB_REPOSITORY_ENV)?;
    let github_sha = required_env(GITHUB_SHA_ENV)?;
    let workflow_ref = required_env(GITHUB_WORKFLOW_REF_ENV)?;

    validate_github_actions_context(
        ctx,
        automation,
        source_sha,
        &github_actions,
        &event_name,
        &github_ref,
        &repository,
        &github_sha,
        &workflow_ref,
    )
}

#[allow(clippy::too_many_arguments)]
fn validate_github_actions_context(
    ctx: &ReleaseContext,
    automation: ReleaseAutomation,
    source_sha: &str,
    github_actions: &str,
    event_name: &str,
    github_ref: &str,
    repository: &str,
    github_sha: &str,
    workflow_ref: &str,
) -> Result<()> {
    validate_full_git_sha(source_sha)?;
    if github_actions != "true" {
        bail!("release publication commands may only run in GitHub Actions");
    }
    let updater_dispatch =
        automation == ReleaseAutomation::Updater && event_name == "workflow_dispatch";
    if event_name != automation.event_name() && !updater_dispatch {
        let expected_event = if automation == ReleaseAutomation::Updater {
            "release or workflow_dispatch"
        } else {
            automation.event_name()
        };
        bail!("unexpected GitHub Actions event: expected {expected_event}, got {event_name}");
    }

    let expected_ref = if updater_dispatch {
        "refs/heads/main".to_string()
    } else {
        automation.expected_ref(ctx)
    };
    if github_ref != expected_ref {
        bail!("unexpected GitHub ref: expected {expected_ref}, got {github_ref}");
    }
    if !repository.eq_ignore_ascii_case(&ctx.repo) {
        bail!(
            "unexpected GitHub repository: expected {}, got {repository}",
            ctx.repo
        );
    }
    validate_full_git_sha(github_sha)?;
    if !updater_dispatch && !github_sha.eq_ignore_ascii_case(source_sha) {
        bail!("GitHub event SHA does not match the requested release source SHA");
    }

    let expected_workflow_ref = format!("{}/{}@", ctx.repo, automation.workflow_path());
    if !workflow_ref
        .to_ascii_lowercase()
        .starts_with(&expected_workflow_ref.to_ascii_lowercase())
    {
        bail!(
            "unexpected GitHub workflow: expected {}, got {workflow_ref}",
            automation.workflow_path()
        );
    }
    Ok(())
}

fn required_env(name: &str) -> Result<String> {
    std::env::var(name)
        .with_context(|| format!("{name} is required for GitHub Actions release automation"))
}

pub fn cargo_xtask() -> ProcessCommand {
    ProcessCommand::new(std::env::current_exe().expect("failed to locate current xtask executable"))
}

pub fn cargo() -> ProcessCommand {
    create_command("cargo")
}

pub fn npm() -> ProcessCommand {
    create_command("npm")
}

pub fn git() -> ProcessCommand {
    create_command("git")
}

pub fn gh() -> ProcessCommand {
    create_command("gh")
}

pub fn remove_updater_signing_env(cmd: &mut ProcessCommand) {
    cmd.env_remove(UPDATER_PRIVATE_KEY_ENV)
        .env_remove(UPDATER_PRIVATE_KEY_PASSWORD_ENV);
}

pub fn remove_github_auth_env(cmd: &mut ProcessCommand) {
    cmd.env_remove(GH_TOKEN_ENV).env_remove(GITHUB_TOKEN_ENV);
}

pub fn check_worktree_clean(ctx: &ReleaseContext) -> Result<()> {
    let mut cmd = git();
    cmd.arg("status")
        .arg("--short")
        .current_dir(&ctx.workspace_root);
    remove_github_auth_env(&mut cmd);
    remove_updater_signing_env(&mut cmd);
    let output = cmd.run_capture_checked("checking git status")?;
    if !output.trim().is_empty() {
        bail!("worktree is not clean:\n{output}");
    }
    Ok(())
}

pub fn current_head(ctx: &ReleaseContext) -> Result<String> {
    let mut cmd = git();
    cmd.arg("rev-parse")
        .arg("--verify")
        .arg("HEAD")
        .current_dir(&ctx.workspace_root);
    remove_github_auth_env(&mut cmd);
    remove_updater_signing_env(&mut cmd);
    let output = cmd.run_capture_checked("resolving release source commit")?;
    let source_sha = output.trim();
    validate_full_git_sha(source_sha)?;
    Ok(source_sha.to_string())
}

pub fn update_workspace_version(ctx: &ReleaseContext, dry_run: bool) -> Result<()> {
    let cargo_toml = ctx.workspace_root.join("Cargo.toml");
    let source = fs::read_to_string(&cargo_toml)
        .with_context(|| format!("reading {}", cargo_toml.display()))?;
    let mut doc = source
        .parse::<DocumentMut>()
        .with_context(|| format!("parsing {}", cargo_toml.display()))?;
    doc["workspace"]["package"]["version"] = value(&ctx.version);
    let rendered = doc.to_string();

    if source == rendered {
        println!("version unchanged: {}", cargo_toml.display());
        return Ok(());
    }

    println!(
        "update workspace package version: {} -> {}",
        cargo_toml.display(),
        ctx.version
    );
    if !dry_run {
        fs::write(&cargo_toml, rendered)
            .with_context(|| format!("writing {}", cargo_toml.display()))?;
    }
    Ok(())
}

pub fn ensure_changelog_ready(ctx: &ReleaseContext) -> Result<()> {
    read_release_changelog_sections(ctx).map(|_| ())
}

pub fn write_release_body_from_changelog(ctx: &ReleaseContext) -> Result<PathBuf> {
    let changelog_sections = read_release_changelog_sections(ctx)?;
    let body = changelog_sections.join("\n---\n\n");
    let path = ctx.release_body();
    let parent = path.parent().context("release body path has no parent")?;
    fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    fs::write(&path, body).with_context(|| format!("writing {}", path.display()))?;
    Ok(path)
}

fn read_release_changelog_sections(ctx: &ReleaseContext) -> Result<Vec<String>> {
    let changelog = fs::read_to_string(&ctx.changelog)
        .with_context(|| format!("reading {}", ctx.changelog.display()))?;
    let canonical_shape = validate_changelog_format_with_title(
        &ctx.version,
        ctx.channel,
        &ctx.repo,
        &changelog,
        "# Changelog",
    )?;
    let body = extract_changelog_release_body(&ctx.version, &changelog)?;
    let mut sections = vec![format!("## English\n\n{body}\n")];

    for (language, relative_path, title) in GITHUB_RELEASE_LOCALIZED_CHANGELOG_SOURCES {
        let path = ctx.workspace_root.join(relative_path);
        let localized =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let localized_shape = validate_changelog_format_with_title(
            &ctx.version,
            ctx.channel,
            &ctx.repo,
            &localized,
            title,
        )?;
        ensure_localized_changelog_matches(
            &canonical_shape,
            &localized_shape,
            &path.display().to_string(),
            &ctx.version,
        )?;
        let body = extract_changelog_release_body(&ctx.version, &localized)?;
        sections.push(format!("## {language}\n\n{body}\n"));
    }

    Ok(sections)
}

fn ensure_localized_changelog_matches(
    canonical: &ChangelogReleaseShape,
    localized: &ChangelogReleaseShape,
    source: &str,
    version: &str,
) -> Result<()> {
    if localized != canonical {
        bail!(
            "localized changelog {source} release {version} must match the canonical date, category order, and bullet counts"
        );
    }
    Ok(())
}

fn extract_changelog_release_body(version: &str, changelog: &str) -> Result<String> {
    let lines = changelog.lines().collect::<Vec<_>>();
    let release_prefix = format!("## [{version}] - ");
    let release_start = lines
        .iter()
        .position(|line| line.starts_with(&release_prefix))
        .with_context(|| format!("changelog release heading is missing for {version}"))?;
    let release_end = lines[release_start + 1..]
        .iter()
        .position(|line| line.starts_with("## [") || line.starts_with("[Unreleased]:"))
        .map(|offset| release_start + 1 + offset)
        .unwrap_or(lines.len());
    let body = lines[release_start + 1..release_end].join("\n");
    let body = body.trim();
    if body.is_empty() {
        bail!("changelog release {version} has no GitHub Release body");
    }
    Ok(format!("{body}\n"))
}

#[cfg(test)]
fn validate_changelog_format(
    version: &str,
    channel: ReleaseChannel,
    repo: &str,
    changelog: &str,
) -> Result<()> {
    validate_changelog_format_with_title(version, channel, repo, changelog, "# Changelog")
        .map(|_| ())
}

fn validate_changelog_format_with_title(
    version: &str,
    channel: ReleaseChannel,
    repo: &str,
    changelog: &str,
    expected_title: &str,
) -> Result<ChangelogReleaseShape> {
    const CHANGE_TYPES: [&str; 6] = [
        "Added",
        "Changed",
        "Deprecated",
        "Removed",
        "Fixed",
        "Security",
    ];

    validate_version_for_channel(version, channel)?;
    let lines = changelog.lines().collect::<Vec<_>>();
    if lines.first().copied() != Some(expected_title) {
        bail!("changelog title must be exactly: {expected_title}");
    }
    if changelog.contains("<!--") || changelog.contains("-->") {
        bail!("changelog must not contain an HTML comment or placeholder");
    }

    let unreleased_sections = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| **line == "## [Unreleased]")
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if unreleased_sections.len() != 1 {
        bail!("changelog must contain exactly one ## [Unreleased] section");
    }

    let release_prefix = format!("## [{version}] - ");
    let release_sections = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| line.strip_prefix(&release_prefix).map(|date| (index, date)))
        .collect::<Vec<_>>();
    if release_sections.len() != 1 {
        bail!("changelog must contain exactly one release heading: {release_prefix}YYYY-MM-DD");
    }
    let (release_start, release_date) = release_sections[0];
    let release_date = NaiveDate::parse_from_str(release_date, "%Y-%m-%d")
        .with_context(|| format!("changelog release date must use YYYY-MM-DD: {release_date}"))?;
    if unreleased_sections[0] >= release_start {
        bail!("changelog Unreleased section must appear before release {version}");
    }

    let release_end = lines[release_start + 1..]
        .iter()
        .position(|line| line.starts_with("## [") || line.starts_with("[Unreleased]:"))
        .map(|offset| release_start + 1 + offset)
        .unwrap_or(lines.len());
    let release_body = &lines[release_start + 1..release_end];
    if release_body.iter().any(|line| {
        let trimmed = line.trim_start();
        trimmed.starts_with("```") || trimmed.starts_with("~~~")
    }) {
        bail!("changelog release {version} must not contain fenced code blocks");
    }
    let change_sections = lines[release_start + 1..release_end]
        .iter()
        .enumerate()
        .filter_map(|(offset, line)| {
            line.strip_prefix("### ")
                .map(|heading| (release_start + 1 + offset, heading))
        })
        .collect::<Vec<_>>();
    if change_sections.is_empty() {
        bail!("changelog release {version} must contain at least one change category");
    }
    if lines[release_start + 1..change_sections[0].0]
        .iter()
        .any(|line| !line.trim().is_empty())
    {
        bail!("changelog release {version} must not contain content before its first category");
    }

    let mut seen_change_types = Vec::new();
    let mut release_categories = Vec::new();
    for (section_index, (section_start, heading)) in change_sections.iter().enumerate() {
        if !CHANGE_TYPES.contains(heading) {
            bail!("changelog release {version} contains unsupported change category: {heading}");
        }
        if seen_change_types.contains(heading) {
            bail!("changelog release {version} contains duplicate change category: {heading}");
        }
        seen_change_types.push(*heading);
        let section_end = change_sections
            .get(section_index + 1)
            .map(|(index, _)| *index)
            .unwrap_or(release_end);
        let mut has_bullet = false;
        let mut has_active_bullet = false;
        let mut bullet_count = 0;
        for line in &lines[*section_start + 1..section_end] {
            if line.trim().is_empty() {
                continue;
            }
            if let Some(bullet) = line.strip_prefix("- ") {
                if bullet.trim().is_empty() {
                    bail!(
                        "changelog release {version} category {heading} must contain non-empty top-level bullets"
                    );
                }
                has_bullet = true;
                has_active_bullet = true;
                bullet_count += 1;
                continue;
            }
            if has_active_bullet && line.starts_with("  ") && !line.trim_start().starts_with("- ") {
                continue;
            }
            bail!(
                "changelog release {version} category {heading} may contain only top-level bullets and their indented continuations"
            );
        }
        if !has_bullet {
            bail!(
                "changelog release {version} category {heading} must contain non-empty top-level bullets"
            );
        }
        release_categories.push(((*heading).to_string(), bullet_count));
    }

    let expected_unreleased_link =
        format!("[Unreleased]: https://github.com/{repo}/compare/v{version}...HEAD");
    let unreleased_links = lines
        .iter()
        .filter(|line| line.starts_with("[Unreleased]: "))
        .collect::<Vec<_>>();
    if unreleased_links.len() != 1 || *unreleased_links[0] != expected_unreleased_link.as_str() {
        bail!("changelog must contain the current comparison link: {expected_unreleased_link}");
    }

    let releases = lines
        .iter()
        .filter_map(|line| {
            if *line == "## [Unreleased]" {
                None
            } else {
                line.strip_prefix("## [")
            }
        })
        .map(|heading| {
            let (candidate, date) = heading
                .split_once("] - ")
                .with_context(|| format!("malformed changelog release heading: ## [{heading}"))?;
            let parsed = Version::parse(candidate)
                .with_context(|| format!("invalid changelog release version: {candidate}"))?;
            let date = NaiveDate::parse_from_str(date, "%Y-%m-%d").with_context(|| {
                format!("changelog release date must use YYYY-MM-DD for {candidate}: {date}")
            })?;
            Ok((candidate, parsed, date))
        })
        .collect::<Result<Vec<_>>>()?;
    let mut seen_releases = Vec::new();
    for (release, _, _) in &releases {
        if seen_releases.contains(release) {
            bail!("changelog contains duplicate release version: {release}");
        }
        seen_releases.push(*release);
    }
    for pair in releases.windows(2) {
        if pair[0].2 < pair[1].2 {
            bail!(
                "changelog releases must use reverse chronological order: {} appears before {}",
                pair[0].0,
                pair[1].0
            );
        }
    }

    for (release_index, (release, parsed, _)) in releases.iter().enumerate() {
        let release_channel = if parsed.pre.is_empty() {
            ReleaseChannel::Stable
        } else {
            ReleaseChannel::Beta
        };
        let previous_version = match release_channel {
            ReleaseChannel::Stable => releases[release_index + 1..]
                .iter()
                .find(|(_, candidate, _)| candidate.pre.is_empty()),
            ReleaseChannel::Beta => releases.get(release_index + 1),
        };
        let expected_release_link = match previous_version {
            Some((previous_version, _, _)) => format!(
                "[{release}]: https://github.com/{repo}/compare/v{previous_version}...v{release}"
            ),
            None => format!("[{release}]: https://github.com/{repo}/releases/tag/v{release}"),
        };
        let release_link_prefix = format!("[{release}]: ");
        let release_links = lines
            .iter()
            .filter(|line| line.starts_with(&release_link_prefix))
            .collect::<Vec<_>>();
        if release_links.len() != 1 || *release_links[0] != expected_release_link.as_str() {
            bail!(
                "changelog release {release} must use the correct {release_channel} comparison baseline: {expected_release_link}"
            );
        }
    }
    Ok(ChangelogReleaseShape {
        date: release_date,
        categories: release_categories,
    })
}

pub fn run_sign_updater_asset(
    workspace_root: &Path,
    asset: &Path,
    runner: &CmdRunner,
    key_loader: &Path,
    purpose: UpdaterSignaturePurpose,
) -> Result<()> {
    if runner.dry_run() {
        let mut cmd = cargo_xtask();
        cmd.arg("sign-alcom-updater")
            .arg("--purpose")
            .arg(purpose.to_string())
            .arg(asset);
        return runner.run(cmd, "signing updater asset");
    }

    let has_key = std::env::var_os(UPDATER_PRIVATE_KEY_ENV).is_some_and(|value| !value.is_empty());
    let has_password =
        std::env::var_os(UPDATER_PRIVATE_KEY_PASSWORD_ENV).is_some_and(|value| !value.is_empty());

    if has_key && has_password {
        let mut cmd = cargo_xtask();
        cmd.arg("sign-alcom-updater")
            .arg("--purpose")
            .arg(purpose.to_string())
            .arg(asset)
            .current_dir(workspace_root);
        return runner.run(cmd, "signing updater asset");
    }

    let key_loader = resolve_key_loader(key_loader)?;
    match key_loader_format(&key_loader) {
        Some(KeyLoaderFormat::PowerShell) => {
            run_sign_updater_with_ps1_loader(workspace_root, asset, runner, &key_loader, purpose)
        }
        Some(KeyLoaderFormat::DotEnv) => {
            run_sign_updater_with_env_loader(workspace_root, asset, runner, &key_loader, purpose)
        }
        _ => bail!("unsupported updater key loader: {}", key_loader.display()),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum KeyLoaderFormat {
    PowerShell,
    DotEnv,
}

fn key_loader_format(path: &Path) -> Option<KeyLoaderFormat> {
    let file_name = path.file_name()?.to_str()?;
    let extension = path.extension().and_then(|extension| extension.to_str());

    if file_name.eq_ignore_ascii_case(".env")
        || extension.is_some_and(|extension| extension.eq_ignore_ascii_case("env"))
    {
        Some(KeyLoaderFormat::DotEnv)
    } else if extension.is_some_and(|extension| extension.eq_ignore_ascii_case("ps1")) {
        Some(KeyLoaderFormat::PowerShell)
    } else {
        None
    }
}

fn run_sign_updater_with_ps1_loader(
    workspace_root: &Path,
    asset: &Path,
    runner: &CmdRunner,
    key_script: &Path,
    purpose: UpdaterSignaturePurpose,
) -> Result<()> {
    let command = format!(
        ". '{}'; cargo xtask sign-alcom-updater --purpose '{}' '{}'",
        ps_quote(key_script),
        purpose,
        ps_quote(asset),
    );

    let mut cmd = if cfg!(windows) {
        create_command("powershell")
    } else {
        create_command("pwsh")
    };
    cmd.arg("-NoProfile")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-Command")
        .arg(command)
        .current_dir(workspace_root);
    runner.run(cmd, "signing updater asset with key loader")
}

fn run_sign_updater_with_env_loader(
    workspace_root: &Path,
    asset: &Path,
    runner: &CmdRunner,
    key_env: &Path,
    purpose: UpdaterSignaturePurpose,
) -> Result<()> {
    let env = read_key_env_loader(key_env)?;
    let private_key = env
        .get(UPDATER_PRIVATE_KEY_ENV)
        .with_context(|| format!("{UPDATER_PRIVATE_KEY_ENV} is missing from updater key loader"))?;
    let password = env.get(UPDATER_PRIVATE_KEY_PASSWORD_ENV).with_context(|| {
        format!("{UPDATER_PRIVATE_KEY_PASSWORD_ENV} is missing from updater key loader")
    })?;

    let mut cmd = cargo_xtask();
    cmd.arg("sign-alcom-updater")
        .arg("--purpose")
        .arg(purpose.to_string())
        .arg(asset)
        .env(UPDATER_PRIVATE_KEY_ENV, private_key)
        .env(UPDATER_PRIVATE_KEY_PASSWORD_ENV, password)
        .current_dir(workspace_root);
    runner.run(cmd, "signing updater asset with env key loader")
}

fn resolve_key_loader(key_loader: &Path) -> Result<PathBuf> {
    if key_loader.is_file() {
        return Ok(key_loader.to_path_buf());
    }

    if key_loader.is_dir() {
        for file_name in ["private-key.ps1", "private-key.env"] {
            let candidate = key_loader.join(file_name);
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }

    bail!(
        "updater signing variables are not set and key loader does not exist: {}",
        key_loader.display()
    )
}

fn read_key_env_loader(key_env: &Path) -> Result<HashMap<String, String>> {
    let source =
        fs::read_to_string(key_env).with_context(|| format!("reading {}", key_env.display()))?;
    let mut env = HashMap::new();

    for (index, line) in source.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let (name, value) = line.split_once('=').with_context(|| {
            format!(
                "invalid updater key loader line {} in {}",
                index + 1,
                key_env.display()
            )
        })?;
        env.insert(name.trim().to_string(), value.trim().to_string());
    }

    Ok(env)
}

fn capture_github_release(
    ctx: &ReleaseContext,
    runner: &CmdRunner,
) -> Result<Option<(ReleaseState, String)>> {
    let mut cmd = gh();
    cmd.arg("release")
        .arg("view")
        .arg(&ctx.tag)
        .arg("--repo")
        .arg(&ctx.repo)
        .arg("--json")
        .arg("tagName,name,isDraft,isPrerelease,targetCommitish,publishedAt,assets");

    let output = runner.capture(cmd, "viewing GitHub Release")?;
    if runner.dry_run() {
        return Ok(None);
    }

    let state: ReleaseState =
        serde_json::from_str(&output).context("parsing GitHub Release JSON")?;
    Ok(Some((state, output)))
}

pub fn ensure_github_release_is_draft(ctx: &ReleaseContext, runner: &CmdRunner) -> Result<()> {
    let Some((state, _)) = capture_github_release(ctx, runner)? else {
        return Ok(());
    };

    validate_github_release_is_replaceable(ctx, &state)
}

fn validate_github_release_is_replaceable(
    ctx: &ReleaseContext,
    state: &ReleaseState,
) -> Result<()> {
    if !state.is_draft {
        bail!("refusing to replace assets on a published GitHub Release");
    }
    if state.is_prerelease != ctx.channel.is_prerelease() {
        bail!(
            "refusing to replace Draft assets with a mismatched prerelease flag for channel {}",
            ctx.channel
        );
    }
    Ok(())
}

pub fn verify_github_release(
    ctx: &ReleaseContext,
    runner: &CmdRunner,
    expected_draft: Option<bool>,
    expected_target_commit: Option<&str>,
) -> Result<Option<String>> {
    let Some((state, output)) = capture_github_release(ctx, runner)? else {
        return Ok(None);
    };

    validate_github_release_state(ctx, &state, expected_draft, expected_target_commit)?;
    let published_at = state.published_at.clone();
    println!("{output}");
    Ok(published_at)
}

fn validate_github_release_state(
    ctx: &ReleaseContext,
    state: &ReleaseState,
    expected_draft: Option<bool>,
    expected_target_commit: Option<&str>,
) -> Result<()> {
    let expected_title = ctx.release_title();
    let expected_assets = ctx.expected_public_asset_names();

    if state.name != expected_title {
        bail!(
            "GitHub Release title mismatch: expected {expected_title}, got {}",
            state.name
        );
    }

    for expected in &expected_assets {
        if !state.assets.iter().any(|asset| asset.name == *expected) {
            bail!("GitHub Release asset is missing: {expected}");
        }
    }
    for asset in &state.assets {
        if !expected_assets
            .iter()
            .any(|expected| asset.name == *expected)
        {
            bail!("GitHub Release has unexpected asset: {}", asset.name);
        }
    }
    if state.assets.len() != expected_assets.len() {
        bail!(
            "GitHub Release asset count mismatch: expected {}, got {}",
            expected_assets.len(),
            state.assets.len()
        );
    }

    if state.is_prerelease != ctx.channel.is_prerelease() {
        bail!(
            "GitHub Release prerelease flag does not match channel {}",
            ctx.channel
        );
    }
    if let Some(expected_draft) = expected_draft
        && state.is_draft != expected_draft
    {
        bail!(
            "GitHub Release draft state mismatch: expected {expected_draft}, got {}",
            state.is_draft
        );
    }
    if expected_draft == Some(false) && state.published_at.is_none() {
        bail!("published GitHub Release has no publishedAt timestamp");
    }
    if let Some(expected_target_commit) = expected_target_commit
        && state.target_commitish != expected_target_commit
    {
        bail!(
            "GitHub Release target commit mismatch: expected {expected_target_commit}, got {}",
            state.target_commitish
        );
    }
    Ok(())
}

pub fn check_public_updater_endpoint(ctx: &ReleaseContext) -> Result<()> {
    let mut response = crate::utils::ureq()
        .get(&ctx.updater_endpoint)
        .call()
        .with_context(|| format!("requesting {}", ctx.updater_endpoint))?;
    let mut body = String::new();
    response
        .body_mut()
        .as_reader()
        .read_to_string(&mut body)
        .with_context(|| format!("reading {}", ctx.updater_endpoint))?;

    let json: serde_json::Value =
        serde_json::from_str(&body).context("parsing public updater JSON")?;
    let expected_source = std::fs::read_to_string(&ctx.updater_json)
        .with_context(|| format!("reading {}", ctx.updater_json.display()))?;
    let expected_json: serde_json::Value = serde_json::from_str(&expected_source)
        .with_context(|| format!("parsing {}", ctx.updater_json.display()))?;
    let expected_url = updater_url(&expected_json)?;
    let expected_signature = updater_signature(&expected_json)?;

    validate_public_updater_document(&ctx.version, expected_url, expected_signature, &json)?;
    validate_public_updater_matches_expected(&expected_json, &json)?;

    println!("public updater endpoint passed: {}", ctx.updater_endpoint);
    Ok(())
}

pub fn validate_public_updater_document(
    expected_version: &str,
    expected_url: &str,
    expected_signature: &str,
    json: &serde_json::Value,
) -> Result<()> {
    let version = json
        .get("version")
        .and_then(|value| value.as_str())
        .context("public updater JSON has no string version")?;
    if version != expected_version {
        bail!("public updater version mismatch: expected {expected_version}, got {version}");
    }

    let url = updater_url(json)?;
    if url != expected_url {
        bail!("public updater URL mismatch: expected {expected_url}, got {url}");
    }
    let signature = updater_signature(json)?;
    if signature != expected_signature {
        bail!("public updater signature mismatch");
    }

    Ok(())
}

pub fn validate_public_updater_matches_expected(
    expected: &serde_json::Value,
    actual: &serde_json::Value,
) -> Result<()> {
    if actual != expected {
        bail!("public updater JSON does not match generated updater JSON");
    }
    Ok(())
}

fn updater_url(json: &serde_json::Value) -> Result<&str> {
    json.pointer("/platforms/windows-x86_64/url")
        .and_then(|value| value.as_str())
        .context("updater JSON has no platforms.windows-x86_64.url")
}

fn updater_signature(json: &serde_json::Value) -> Result<&str> {
    json.pointer("/platforms/windows-x86_64/signature")
        .and_then(|value| value.as_str())
        .context("updater JSON has no platforms.windows-x86_64.signature")
}

fn ps_quote(path: &Path) -> String {
    path.as_os_str().to_string_lossy().replace('\'', "''")
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReleaseState {
    name: String,
    is_draft: bool,
    is_prerelease: bool,
    published_at: Option<String>,
    target_commitish: String,
    assets: Vec<ReleaseAsset>,
}

#[derive(Deserialize)]
struct ReleaseAsset {
    name: String,
}

#[cfg(test)]
mod tests {
    use super::{
        GH_TOKEN_ENV, GITHUB_TOKEN_ENV, KeyLoaderFormat, ReleaseAsset, ReleaseAutomation,
        ReleaseChannel, ReleaseContext, ReleaseState, UPDATER_PRIVATE_KEY_ENV,
        UPDATER_PRIVATE_KEY_PASSWORD_ENV, ensure_localized_changelog_matches,
        extract_changelog_release_body, key_loader_format, remove_github_auth_env,
        remove_updater_signing_env, validate_changelog_format,
        validate_changelog_format_with_title, validate_github_actions_context,
        validate_github_release_is_replaceable, validate_github_release_state,
        validate_public_updater_document, validate_public_updater_matches_expected,
        validate_release_source_versions,
    };
    use serde_json::json;
    use std::process::Command as ProcessCommand;

    fn expected_release_assets(ctx: &ReleaseContext) -> Vec<ReleaseAsset> {
        ctx.expected_public_asset_names()
            .into_iter()
            .map(|name| ReleaseAsset { name })
            .collect()
    }

    fn valid_changelog() -> String {
        r#"# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

## [3.0.0] - 2026-07-26

### Added

- Published the first ALCOMD3 release.

[Unreleased]: https://github.com/ALCOMD3/ALCOMD3/compare/v3.0.0...HEAD
[3.0.0]: https://github.com/ALCOMD3/ALCOMD3/releases/tag/v3.0.0
"#
        .to_string()
    }

    fn stable_comparison_changelog() -> String {
        r#"# Changelog

## [Unreleased]

## [3.1.0] - 2026-08-01

### Added

- Published the stable release.

## [3.1.0-beta.2] - 2026-07-28

### Fixed

- Fixed the second beta.

## [3.1.0-beta.1] - 2026-07-27

### Added

- Published the first beta.

## [3.0.0] - 2026-07-26

### Added

- Published the first stable release.

[Unreleased]: https://github.com/ALCOMD3/ALCOMD3/compare/v3.1.0...HEAD
[3.1.0]: https://github.com/ALCOMD3/ALCOMD3/compare/v3.0.0...v3.1.0
[3.1.0-beta.2]: https://github.com/ALCOMD3/ALCOMD3/compare/v3.1.0-beta.1...v3.1.0-beta.2
[3.1.0-beta.1]: https://github.com/ALCOMD3/ALCOMD3/compare/v3.0.0...v3.1.0-beta.1
[3.0.0]: https://github.com/ALCOMD3/ALCOMD3/releases/tag/v3.0.0
"#
        .to_string()
    }

    fn beta_comparison_changelog() -> String {
        r#"# Changelog

## [Unreleased]

## [3.1.0-beta.2] - 2026-07-28

### Fixed

- Fixed the second beta.

## [3.1.0-beta.1] - 2026-07-27

### Added

- Published the first beta.

## [3.0.0] - 2026-07-26

### Added

- Published the first stable release.

[Unreleased]: https://github.com/ALCOMD3/ALCOMD3/compare/v3.1.0-beta.2...HEAD
[3.1.0-beta.2]: https://github.com/ALCOMD3/ALCOMD3/compare/v3.1.0-beta.1...v3.1.0-beta.2
[3.1.0-beta.1]: https://github.com/ALCOMD3/ALCOMD3/compare/v3.0.0...v3.1.0-beta.1
[3.0.0]: https://github.com/ALCOMD3/ALCOMD3/releases/tag/v3.0.0
"#
        .to_string()
    }

    #[test]
    fn changelog_accepts_keep_a_changelog_release_entry() {
        validate_changelog_format(
            "3.0.0",
            ReleaseChannel::Stable,
            "ALCOMD3/ALCOMD3",
            &valid_changelog(),
        )
        .unwrap();
    }

    #[test]
    fn repository_changelog_matches_current_workspace_release() {
        let version = env!("CARGO_PKG_VERSION");
        let parsed = semver::Version::parse(version).unwrap();
        let channel = if parsed.pre.is_empty() {
            ReleaseChannel::Stable
        } else {
            ReleaseChannel::Beta
        };

        let canonical = validate_changelog_format_with_title(
            version,
            channel,
            "ALCOMD3/ALCOMD3",
            include_str!("../../CHANGELOG.md"),
            "# Changelog",
        )
        .unwrap();
        let japanese = validate_changelog_format_with_title(
            version,
            channel,
            "ALCOMD3/ALCOMD3",
            include_str!("../../CHANGELOG/CHANGELOG.ja.md"),
            "# 変更履歴",
        )
        .unwrap();
        let simplified_chinese = validate_changelog_format_with_title(
            version,
            channel,
            "ALCOMD3/ALCOMD3",
            include_str!("../../CHANGELOG/CHANGELOG.zh-CN.md"),
            "# 更新日志",
        )
        .unwrap();

        ensure_localized_changelog_matches(&canonical, &japanese, "Japanese", version).unwrap();
        ensure_localized_changelog_matches(
            &canonical,
            &simplified_chinese,
            "Simplified Chinese",
            version,
        )
        .unwrap();
    }

    #[test]
    fn localized_changelog_rejects_different_bullet_count() {
        let canonical = validate_changelog_format_with_title(
            "3.0.0",
            ReleaseChannel::Stable,
            "ALCOMD3/ALCOMD3",
            &valid_changelog(),
            "# Changelog",
        )
        .unwrap();
        let localized = valid_changelog()
            .replace("# Changelog", "# 更新日志")
            .replace(
                "- Published the first ALCOMD3 release.",
                "- 发布首个版本。\n- 第二条遗漏检查。",
            );
        let localized = validate_changelog_format_with_title(
            "3.0.0",
            ReleaseChannel::Stable,
            "ALCOMD3/ALCOMD3",
            &localized,
            "# 更新日志",
        )
        .unwrap();

        let error = ensure_localized_changelog_matches(
            &canonical,
            &localized,
            "CHANGELOG.zh-CN.md",
            "3.0.0",
        )
        .unwrap_err();

        assert!(error.to_string().contains("bullet counts"));
    }

    #[test]
    fn changelog_stable_release_compares_with_previous_stable() {
        validate_changelog_format(
            "3.1.0",
            ReleaseChannel::Stable,
            "ALCOMD3/ALCOMD3",
            &stable_comparison_changelog(),
        )
        .unwrap();
    }

    #[test]
    fn changelog_beta_release_compares_with_immediately_previous_release() {
        validate_changelog_format(
            "3.1.0-beta.2",
            ReleaseChannel::Beta,
            "ALCOMD3/ALCOMD3",
            &beta_comparison_changelog(),
        )
        .unwrap();
    }

    #[test]
    fn changelog_rejects_stable_comparison_against_a_beta() {
        let changelog =
            stable_comparison_changelog().replace("v3.0.0...v3.1.0", "v3.1.0-beta.2...v3.1.0");
        let error = validate_changelog_format(
            "3.1.0",
            ReleaseChannel::Stable,
            "ALCOMD3/ALCOMD3",
            &changelog,
        )
        .unwrap_err();

        assert!(error.to_string().contains("stable comparison baseline"));
    }

    #[test]
    fn changelog_rejects_beta_comparison_that_skips_a_release() {
        let changelog = beta_comparison_changelog()
            .replace("v3.1.0-beta.1...v3.1.0-beta.2", "v3.0.0...v3.1.0-beta.2");
        let error = validate_changelog_format(
            "3.1.0-beta.2",
            ReleaseChannel::Beta,
            "ALCOMD3/ALCOMD3",
            &changelog,
        )
        .unwrap_err();

        assert!(error.to_string().contains("beta comparison baseline"));
    }

    #[test]
    fn changelog_rejects_stale_historical_comparison_link() {
        let changelog = stable_comparison_changelog()
            .replace("v3.1.0-beta.1...v3.1.0-beta.2", "v3.0.0...v3.1.0-beta.2");
        let error = validate_changelog_format(
            "3.1.0",
            ReleaseChannel::Stable,
            "ALCOMD3/ALCOMD3",
            &changelog,
        )
        .unwrap_err();

        assert!(error.to_string().contains("beta comparison baseline"));
    }

    #[test]
    fn changelog_rejects_duplicate_version_links() {
        let link = "[3.0.0]: https://github.com/ALCOMD3/ALCOMD3/releases/tag/v3.0.0";
        let changelog = valid_changelog().replace(link, &format!("{link}\n{link}"));
        let error = validate_changelog_format(
            "3.0.0",
            ReleaseChannel::Stable,
            "ALCOMD3/ALCOMD3",
            &changelog,
        )
        .unwrap_err();

        assert!(error.to_string().contains("stable comparison baseline"));
    }

    #[test]
    fn changelog_rejects_non_chronological_release_order() {
        let changelog = stable_comparison_changelog()
            .replace("3.1.0-beta.2] - 2026-07-28", "3.1.0-beta.2] - 2026-08-02");
        let error = validate_changelog_format(
            "3.1.0",
            ReleaseChannel::Stable,
            "ALCOMD3/ALCOMD3",
            &changelog,
        )
        .unwrap_err();

        assert!(error.to_string().contains("reverse chronological order"));
    }

    #[test]
    fn changelog_release_body_contains_only_target_version_content() {
        let body = extract_changelog_release_body("3.0.0", &valid_changelog()).unwrap();

        assert_eq!(
            body,
            "### Added\n\n- Published the first ALCOMD3 release.\n"
        );
    }

    #[test]
    fn changelog_rejects_missing_target_release() {
        let changelog = valid_changelog().replace("## [3.0.0] - 2026-07-26", "");
        let error = validate_changelog_format(
            "3.0.0",
            ReleaseChannel::Stable,
            "ALCOMD3/ALCOMD3",
            &changelog,
        )
        .unwrap_err();

        assert!(error.to_string().contains("release heading"));
    }

    #[test]
    fn changelog_rejects_non_iso_release_date() {
        let changelog = valid_changelog().replace("2026-07-26", "July 26, 2026");
        let error = validate_changelog_format(
            "3.0.0",
            ReleaseChannel::Stable,
            "ALCOMD3/ALCOMD3",
            &changelog,
        )
        .unwrap_err();

        assert!(error.to_string().contains("YYYY-MM-DD"));
    }

    #[test]
    fn changelog_rejects_unknown_change_category() {
        let changelog = valid_changelog().replace("### Added", "### Updates");
        let error = validate_changelog_format(
            "3.0.0",
            ReleaseChannel::Stable,
            "ALCOMD3/ALCOMD3",
            &changelog,
        )
        .unwrap_err();

        assert!(error.to_string().contains("unsupported change category"));
    }

    #[test]
    fn changelog_rejects_empty_change_category() {
        let changelog = valid_changelog().replace("- Published the first ALCOMD3 release.", "");
        let error = validate_changelog_format(
            "3.0.0",
            ReleaseChannel::Stable,
            "ALCOMD3/ALCOMD3",
            &changelog,
        )
        .unwrap_err();

        assert!(error.to_string().contains("non-empty top-level bullets"));
    }

    #[test]
    fn changelog_rejects_content_before_first_change_category() {
        let changelog = valid_changelog().replace("### Added", "Unexpected summary.\n\n### Added");
        let error = validate_changelog_format(
            "3.0.0",
            ReleaseChannel::Stable,
            "ALCOMD3/ALCOMD3",
            &changelog,
        )
        .unwrap_err();

        assert!(error.to_string().contains("before its first category"));
    }

    #[test]
    fn changelog_rejects_duplicate_change_category() {
        let changelog = valid_changelog().replace(
            "[Unreleased]:",
            "### Added\n\n- Duplicated category.\n\n[Unreleased]:",
        );
        let error = validate_changelog_format(
            "3.0.0",
            ReleaseChannel::Stable,
            "ALCOMD3/ALCOMD3",
            &changelog,
        )
        .unwrap_err();

        assert!(error.to_string().contains("duplicate change category"));
    }

    #[test]
    fn changelog_rejects_fenced_code_blocks() {
        let changelog = valid_changelog().replace(
            "- Published the first ALCOMD3 release.",
            "- Published the first ALCOMD3 release.\n\n```text\ninternal detail\n```",
        );
        let error = validate_changelog_format(
            "3.0.0",
            ReleaseChannel::Stable,
            "ALCOMD3/ALCOMD3",
            &changelog,
        )
        .unwrap_err();

        assert!(error.to_string().contains("fenced code blocks"));
    }

    #[test]
    fn changelog_rejects_stale_unreleased_comparison_link() {
        let changelog = valid_changelog().replace("v3.0.0...HEAD", "v2.1.0...HEAD");
        let error = validate_changelog_format(
            "3.0.0",
            ReleaseChannel::Stable,
            "ALCOMD3/ALCOMD3",
            &changelog,
        )
        .unwrap_err();

        assert!(error.to_string().contains("current comparison link"));
    }

    #[test]
    fn release_source_versions_accept_matching_versions() {
        validate_release_source_versions("2.1.1", &["2.1.1".to_string()], "2.1.1").unwrap();
    }

    #[test]
    fn updater_key_loader_recognizes_dotenv_and_named_env_files() {
        assert_eq!(
            key_loader_format(std::path::Path::new(".env")),
            Some(KeyLoaderFormat::DotEnv)
        );
        assert_eq!(
            key_loader_format(std::path::Path::new("private-key.env")),
            Some(KeyLoaderFormat::DotEnv)
        );
        assert_eq!(
            key_loader_format(std::path::Path::new("private-key.ps1")),
            Some(KeyLoaderFormat::PowerShell)
        );
    }

    #[test]
    fn release_source_versions_reject_workspace_mismatch() {
        let error = validate_release_source_versions(
            "2.1.1",
            &["2.1.0".to_string(), "2.1.1".to_string()],
            "2.1.1",
        )
        .unwrap_err();

        assert!(error.to_string().contains("workspace package version"));
    }

    #[test]
    fn release_source_versions_reject_npm_mismatch() {
        let error =
            validate_release_source_versions("2.1.1", &["2.1.1".to_string()], "2.1.0").unwrap_err();

        assert!(error.to_string().contains("vrc-get-gui package version"));
    }

    #[test]
    fn public_updater_document_rejects_stale_signature() {
        let document = json!({
            "version": "2.1.1",
            "platforms": {
                "windows-x86_64": {
                    "url": "https://example.test/alcomd3-2.1.1-setup.exe",
                    "signature": "old-signature"
                }
            }
        });

        let error = validate_public_updater_document(
            "2.1.1",
            "https://example.test/alcomd3-2.1.1-setup.exe",
            "new-signature",
            &document,
        )
        .unwrap_err();

        assert!(error.to_string().contains("signature mismatch"));
    }

    #[test]
    fn public_updater_document_rejects_unexpected_url() {
        let document = json!({
            "version": "2.1.1",
            "platforms": {
                "windows-x86_64": {
                    "url": "https://wrong.example/alcomd3-2.1.1-setup.exe",
                    "signature": "signature"
                }
            }
        });

        let error = validate_public_updater_document(
            "2.1.1",
            "https://example.test/alcomd3-2.1.1-setup.exe",
            "signature",
            &document,
        )
        .unwrap_err();

        assert!(error.to_string().contains("URL mismatch"));
    }

    #[test]
    fn public_updater_document_rejects_stale_notes() {
        let expected = json!({
            "version": "2.1.1",
            "notes": "Current notes",
            "platforms": {
                "windows-x86_64": {
                    "url": "https://example.test/alcomd3-2.1.1-setup.exe",
                    "signature": "signature"
                }
            }
        });
        let mut actual = expected.clone();
        actual["notes"] = json!("Stale notes");

        let error = validate_public_updater_matches_expected(&expected, &actual).unwrap_err();

        assert!(error.to_string().contains("does not match"));
    }

    #[test]
    fn github_release_state_rejects_published_release_when_draft_is_required() {
        let ctx = ReleaseContext::new("2.1.1", ReleaseChannel::Stable, None, None, None).unwrap();
        let state = ReleaseState {
            name: ctx.release_title(),
            is_draft: false,
            is_prerelease: false,
            published_at: Some("2026-07-12T01:02:03Z".to_string()),
            target_commitish: "0123456789abcdef".to_string(),
            assets: expected_release_assets(&ctx),
        };

        let error = validate_github_release_state(&ctx, &state, Some(true), None).unwrap_err();

        assert!(error.to_string().contains("draft state mismatch"));
    }

    #[test]
    fn published_release_state_requires_published_at() {
        let ctx = ReleaseContext::new("2.1.1", ReleaseChannel::Stable, None, None, None).unwrap();
        let state = ReleaseState {
            name: ctx.release_title(),
            is_draft: false,
            is_prerelease: false,
            published_at: None,
            target_commitish: "0123456789abcdef".to_string(),
            assets: expected_release_assets(&ctx),
        };

        let error = validate_github_release_state(&ctx, &state, Some(false), None).unwrap_err();

        assert!(error.to_string().contains("publishedAt"));
    }

    #[test]
    fn github_release_state_rejects_unexpected_asset() {
        let ctx = ReleaseContext::new("2.1.1", ReleaseChannel::Stable, None, None, None).unwrap();
        let state = ReleaseState {
            name: ctx.release_title(),
            is_draft: true,
            is_prerelease: false,
            published_at: None,
            target_commitish: "0123456789abcdef".to_string(),
            assets: {
                let mut assets = expected_release_assets(&ctx);
                assets.push(ReleaseAsset {
                    name: "stale-build.zip".to_string(),
                });
                assets
            },
        };

        let error = validate_github_release_state(&ctx, &state, Some(true), None).unwrap_err();

        assert!(error.to_string().contains("unexpected asset"));
    }

    #[test]
    fn github_release_state_rejects_wrong_source_commit() {
        let ctx = ReleaseContext::new("2.1.1", ReleaseChannel::Stable, None, None, None).unwrap();
        let state = ReleaseState {
            name: ctx.release_title(),
            is_draft: true,
            is_prerelease: false,
            published_at: None,
            target_commitish: "wrong-commit".to_string(),
            assets: expected_release_assets(&ctx),
        };

        let error =
            validate_github_release_state(&ctx, &state, Some(true), Some("0123456789abcdef"))
                .unwrap_err();

        assert!(error.to_string().contains("target commit"));
    }

    #[test]
    fn replacement_draft_may_target_an_older_source_commit() {
        let ctx = ReleaseContext::new("2.1.1", ReleaseChannel::Stable, None, None, None).unwrap();
        let state = ReleaseState {
            name: ctx.release_title(),
            is_draft: true,
            is_prerelease: false,
            published_at: None,
            target_commitish: "older-source-commit".to_string(),
            assets: vec![],
        };

        validate_github_release_is_replaceable(&ctx, &state).unwrap();
    }

    #[test]
    fn replacement_refuses_published_release() {
        let ctx = ReleaseContext::new("2.1.1", ReleaseChannel::Stable, None, None, None).unwrap();
        let state = ReleaseState {
            name: ctx.release_title(),
            is_draft: false,
            is_prerelease: false,
            published_at: Some("2026-07-13T00:00:00Z".to_string()),
            target_commitish: "0123456789abcdef".to_string(),
            assets: vec![],
        };

        let error = validate_github_release_is_replaceable(&ctx, &state).unwrap_err();
        assert!(error.to_string().contains("published GitHub Release"));
    }

    #[test]
    fn non_signing_commands_remove_updater_secrets() {
        let mut command = ProcessCommand::new("test");
        command
            .env(UPDATER_PRIVATE_KEY_ENV, "private")
            .env(UPDATER_PRIVATE_KEY_PASSWORD_ENV, "password");

        remove_updater_signing_env(&mut command);

        let removed = command
            .get_envs()
            .filter(|(_, value)| value.is_none())
            .map(|(name, _)| name.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert!(removed.contains(&UPDATER_PRIVATE_KEY_ENV.to_string()));
        assert!(removed.contains(&UPDATER_PRIVATE_KEY_PASSWORD_ENV.to_string()));
    }

    #[test]
    fn non_github_commands_remove_github_tokens() {
        let mut command = ProcessCommand::new("test");
        command
            .env(GH_TOKEN_ENV, "gh-token")
            .env(GITHUB_TOKEN_ENV, "github-token");

        remove_github_auth_env(&mut command);

        let removed = command
            .get_envs()
            .filter(|(_, value)| value.is_none())
            .map(|(name, _)| name.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert!(removed.contains(&GH_TOKEN_ENV.to_string()));
        assert!(removed.contains(&GITHUB_TOKEN_ENV.to_string()));
    }

    #[test]
    fn draft_automation_accepts_only_the_pinned_dispatch_context() {
        let ctx = ReleaseContext::new("2.1.1", ReleaseChannel::Stable, None, None, None).unwrap();
        let source_sha = "0123456789abcdef0123456789abcdef01234567";

        validate_github_actions_context(
            &ctx,
            ReleaseAutomation::Draft,
            source_sha,
            "true",
            "workflow_dispatch",
            "refs/heads/main",
            &ctx.repo,
            source_sha,
            &format!(
                "{}/.github/workflows/release-draft.yml@refs/heads/main",
                ctx.repo
            ),
        )
        .unwrap();
    }

    #[test]
    fn updater_automation_rejects_the_wrong_workflow() {
        let ctx = ReleaseContext::new("2.1.1", ReleaseChannel::Stable, None, None, None).unwrap();
        let source_sha = "0123456789abcdef0123456789abcdef01234567";

        let error = validate_github_actions_context(
            &ctx,
            ReleaseAutomation::Updater,
            source_sha,
            "true",
            "release",
            "refs/tags/v2.1.1",
            &ctx.repo,
            source_sha,
            &format!(
                "{}/.github/workflows/release-draft.yml@refs/heads/main",
                ctx.repo
            ),
        )
        .unwrap_err();

        assert!(error.to_string().contains("workflow"));
    }

    #[test]
    fn updater_automation_accepts_a_main_branch_recovery_dispatch() {
        let ctx = ReleaseContext::new("2.1.1", ReleaseChannel::Stable, None, None, None).unwrap();
        let source_sha = "0123456789abcdef0123456789abcdef01234567";
        let workflow_sha = "89abcdef0123456789abcdef0123456789abcdef";

        validate_github_actions_context(
            &ctx,
            ReleaseAutomation::Updater,
            source_sha,
            "true",
            "workflow_dispatch",
            "refs/heads/main",
            &ctx.repo,
            workflow_sha,
            &format!(
                "{}/.github/workflows/release-updater.yml@refs/heads/main",
                ctx.repo
            ),
        )
        .unwrap();
    }
}
