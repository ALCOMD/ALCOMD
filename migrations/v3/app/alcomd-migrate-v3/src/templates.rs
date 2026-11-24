//! Reads published template metadata and converts project archives into current ZIPs.
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use alcomd_domain::vpm_range::VpmRange;
use alcomd_import::{Template, TemplateSource};
use anyhow::{Context, Result, ensure};
use flate2::bufread::GzDecoder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zip::write::SimpleFileOptions;

use crate::profile::{checked_path, read_file};

pub const DERIVED_EDITOR: &str = "2022.3.22f1";
#[derive(Debug)]
pub struct UnsupportedTemplatePackages;

impl std::fmt::Display for UnsupportedTemplatePackages {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(
            "A derived template includes Unity package attachments that cannot be imported.",
        )
    }
}

impl std::error::Error for UnsupportedTemplatePackages {}

const TEMPLATE_BYTES: u64 = 256 * 1024 * 1024;
const EXPANDED_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_TEMPLATES: usize = 128;
const MAX_ENTRIES: usize = 65_536;
const HEADER_BYTES: usize = 256 * 1024;
const TAR_BLOCK: usize = 512;
const OLD_KIND: &str = "com.anatawa12.vrc-get.custom-template";

#[derive(Serialize)]
pub struct Archive {
    source: PathBuf,
    digest: [u8; 32],
    id: String,
    name: String,
    relative: String,
}

impl Archive {
    pub fn relative(&self) -> &str {
        &self.relative
    }

    pub fn convert(&self, source_root: &Path, stage_root: &Path) -> Result<()> {
        let bytes = read_file(source_root, &self.source, TEMPLATE_BYTES)?
            .context("missing source template")?;
        let digest: [u8; 32] = Sha256::digest(&bytes).into();
        ensure!(digest == self.digest, "template changed after inventory");
        let (_, payload) = separate(&bytes)?;
        let destination = stage_root.join(&self.relative);
        let file = File::options()
            .write(true)
            .create_new(true)
            .open(destination)?;
        rewrite_archive(file, payload, &self.id, &self.name)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Header {
    #[serde(rename = "$type")]
    subtype: String,
    format_version: String,
    display_name: String,
    id: Option<String>,
    template_kind: Option<String>,
    base: Option<String>,
    unity_version: Option<String>,
    #[serde(default)]
    vpm_dependencies: BTreeMap<String, String>,
    #[serde(default)]
    unity_packages: Vec<String>,
    archive: Option<LegacyArchive>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyArchive {
    format: String,
    unity_version: String,
}

#[derive(Serialize)]
pub struct Inventory {
    pub templates: Vec<Template>,
    pub attachments: Vec<Archive>,
    pub skipped_derived: usize,
}

pub fn inventory(root: &Path) -> Result<Inventory> {
    inventory_directories(root, &["templates", "vrc-get/templates"])
}

pub(super) fn inventory_legacy(root: &Path) -> Result<Inventory> {
    inventory_directories(root, &["vrc-get/templates"])
}

fn inventory_directories(root: &Path, directory_names: &[&str]) -> Result<Inventory> {
    let mut templates = Vec::new();
    let mut attachments = Vec::new();
    let mut id_claims = BTreeSet::new();
    let mut mappings = BTreeMap::new();
    for &dir_name in directory_names {
        let folder = root.join(dir_name);
        match alcomd_platform::resolve_plain_storage_directory(&folder) {
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(_) => anyhow::bail!("unsafe template directory"),
        }
        let mut selected = Vec::new();
        for entry in fs::read_dir(folder)? {
            let relative = PathBuf::from(dir_name).join(entry?.file_name());
            if relative
                .extension()
                .and_then(|part| part.to_str())
                .is_some_and(|part| part.eq_ignore_ascii_case("alcomtemplate"))
            {
                ensure!(selected.len() < MAX_TEMPLATES, "too many source templates");
                selected.push(relative);
            }
        }
        selected.sort_unstable();
        for relative in selected {
            ensure!(templates.len() < MAX_TEMPLATES, "too many source templates");
            let bytes =
                read_file(root, &relative, TEMPLATE_BYTES)?.context("template disappeared")?;
            let (header, content) = separate(&bytes)?;
            let entry: Header = serde_json::from_slice(header)
                .map_err(|_| anyhow::anyhow!("invalid template metadata"))?;
            ensure!(
                entry.subtype == OLD_KIND,
                "unrecognized source template format"
            );
            let name = entry.display_name.trim().to_owned();
            ensure!(
                !name.is_empty() && name.len() <= 128 && !name.chars().any(char::is_control),
                "invalid template display name"
            );
            let relative_text = relative
                .to_str()
                .context("invalid template filename")?
                .replace('\\', "/");
            let stable_id = format!(
                "com.vrclearn.alcomd.migration.{}",
                crate::hex_digest(&Sha256::digest(relative_text.as_bytes()))
            );
            let id = entry
                .id
                .as_deref()
                .filter(|id| valid_id(id))
                .map_or(stable_id, str::to_owned);
            ensure!(
                builtin(&id).is_none() && !is_builtin(&id) && id_claims.insert(id.clone()),
                "duplicate source template identifier"
            );
            if let Some(old_id) = entry.id {
                ensure!(
                    mappings.insert(old_id, id.clone()).is_none(),
                    "duplicate source template identifier"
                );
            }
            let source = match (
                entry.format_version.as_str(),
                entry.template_kind.as_deref(),
                entry.archive,
            ) {
                ("1.0", None | Some("derived"), None) | ("2.0", Some("derived"), None) => {
                    ensure!(content.is_empty(), "unexpected derived template payload");
                    let base_template = entry.base.context("derived template lacks a base")?;
                    let unity_range = entry.unity_version.unwrap_or_else(|| "*".into());
                    ensure!(
                        entry.vpm_dependencies.len() <= 512 && entry.unity_packages.len() <= 128,
                        "too many template ingredients"
                    );
                    let unity_packages = entry
                        .unity_packages
                        .into_iter()
                        .map(checked_path)
                        .collect::<Result<_>>()?;
                    TemplateSource::Manual {
                        base_template,
                        unity_range,
                        dependencies: entry.vpm_dependencies,
                        unity_packages,
                    }
                }
                ("2.0", Some("projectArchive"), Some(archive)) => {
                    ensure!(
                        archive.format == "tar.gz" && content.starts_with(&[0x1f, 0x8b]),
                        "unrecognized archived project payload"
                    );
                    let archive_path =
                        format!("templates/{:04}.alcomdtemplate", attachments.len() + 1);
                    attachments.push(Archive {
                        source: relative,
                        digest: Sha256::digest(&bytes).into(),
                        id: id.clone(),
                        name: name.clone(),
                        relative: archive_path.clone(),
                    });
                    TemplateSource::ProjectArchive {
                        unity_version: archive.unity_version,
                        archive_path,
                    }
                }
                _ => anyhow::bail!("unsupported source template version"),
            };
            templates.push(Template {
                id: Some(id),
                name,
                source,
            });
        }
    }
    for template in &mut templates {
        if let TemplateSource::Manual { base_template, .. } = &mut template.source {
            if let Some(id) = mappings.get(base_template) {
                base_template.clone_from(id);
            } else {
                *base_template = builtin(base_template)
                    .context("unresolved template base")?
                    .into();
            }
        }
    }
    let source_count = templates.len();
    let templates = select_derived_templates(templates)?;
    Ok(Inventory {
        skipped_derived: source_count - templates.len(),
        templates,
        attachments,
    })
}

fn separate(bytes: &[u8]) -> Result<(&[u8], &[u8])> {
    let bound = bytes.len().min(HEADER_BYTES);
    if let Some(index) = bytes[..bound].iter().position(|&byte| byte == 0) {
        Ok((&bytes[..index], &bytes[index + 1..]))
    } else {
        ensure!(
            bytes.len() <= HEADER_BYTES,
            "template header exceeds its byte limit"
        );
        Ok((bytes, &[]))
    }
}

fn select_derived_templates(templates: Vec<Template>) -> Result<Vec<Template>> {
    let target = semver::Version::new(2022, 3, 22);
    let ordered = alcomd_import::order_template_bases(templates).map_err(anyhow::Error::msg)?;
    // Only the old derived declarations add dependency constraints. A standalone
    // project archive already carries the exact package manifest it was saved with.
    let mut eligible = BTreeMap::<String, BTreeMap<String, String>>::new();
    for base_id in [
        "com.vrclearn.alcomd.templates.blank",
        "com.vrclearn.alcomd.templates.vrchat.avatars.2022",
        "com.vrclearn.alcomd.templates.vrchat.worlds.2022",
    ] {
        eligible.insert(base_id.into(), BTreeMap::new());
    }
    let mut selected = Vec::new();
    for mut template in ordered {
        let id = template
            .id
            .as_ref()
            .context("missing migrated template ID")?;
        match &mut template.source {
            TemplateSource::ProjectArchive { unity_version, .. } => {
                if unity_version == DERIVED_EDITOR {
                    eligible.insert(id.clone(), BTreeMap::new());
                }
            }
            TemplateSource::Manual {
                base_template,
                unity_range,
                dependencies,
                unity_packages,
            } => {
                let range = VpmRange::parse(unity_range).context("invalid derived Unity range")?;
                for constraint in dependencies.values_mut() {
                    *constraint = VpmRange::parse(constraint)
                        .context("invalid derived package range")?
                        .canonical();
                    // The v3 declaration uses an ordinary exact range. Preserve
                    // that meaning when the native project reads a direct requirement.
                    if semver::Version::parse(constraint).is_ok() {
                        constraint.insert(0, '=');
                    }
                }
                let Some(parent) = eligible.get(base_template) else {
                    continue;
                };
                if !range.matches(&target, false) {
                    continue;
                }
                if !unity_packages.is_empty() {
                    return Err(UnsupportedTemplatePackages.into());
                }
                let mut merged = parent.clone();
                for (package, constraint) in dependencies.iter_mut() {
                    if let Some(inherited) = parent.get(package) {
                        *constraint = intersect_package_constraints(inherited, constraint)?;
                    }
                    merged.insert(package.clone(), constraint.clone());
                }
                ensure!(merged.len() <= 128, "too many derived dependencies");
                eligible.insert(id.clone(), merged);
                *unity_range = DERIVED_EDITOR.into();
            }
        }
        selected.push(template);
    }
    Ok(selected)
}

fn intersect_package_constraints(inherited: &str, declared: &str) -> Result<String> {
    if inherited == declared || declared == "*" {
        return Ok(inherited.to_owned());
    }
    if inherited == "*" {
        return Ok(declared.to_owned());
    }
    let mut intersection = String::new();
    for left in inherited.split(" || ") {
        for right in declared.split(" || ") {
            if !intersection.is_empty() {
                intersection.push_str(" || ");
            }
            intersection.push_str(left);
            intersection.push(' ');
            intersection.push_str(right);
            ensure!(
                intersection.len() <= alcomd_domain::vpm_range::MAX_RANGE_BYTES,
                "derived dependency intersection exceeds limit"
            );
        }
    }
    Ok(VpmRange::parse(&intersection)
        .context("unrepresentable derived dependency intersection")?
        .canonical())
}

fn builtin(id: &str) -> Option<&'static str> {
    match id {
        "com.anatawa12.vrc-get.blank" => Some("com.vrclearn.alcomd.templates.blank"),
        "com.anatawa12.vrc-get.vrchat.avatars" => {
            Some("com.vrclearn.alcomd.templates.vrchat.avatars.2022")
        }
        "com.anatawa12.vrc-get.vrchat.worlds" => {
            Some("com.vrclearn.alcomd.templates.vrchat.worlds.2022")
        }
        _ => None,
    }
}

fn is_builtin(id: &str) -> bool {
    [
        "com.vrclearn.alcomd.templates.blank",
        "com.vrclearn.alcomd.templates.vrchat.avatars.2019",
        "com.vrclearn.alcomd.templates.vrchat.avatars.2022",
        "com.vrclearn.alcomd.templates.vrchat.worlds.2019",
        "com.vrclearn.alcomd.templates.vrchat.worlds.2022",
    ]
    .contains(&id)
}

fn valid_id(id: &str) -> bool {
    let edge = |byte: &u8| byte.is_ascii_lowercase() || byte.is_ascii_digit();
    id.len() <= 128
        && id.contains('.')
        && id.split('.').all(|part| {
            part.as_bytes().first().is_some_and(edge)
                && part.as_bytes().last().is_some_and(edge)
                && part
                    .as_bytes()
                    .iter()
                    .all(|byte| edge(byte) || *byte == b'-')
        })
}

fn rewrite_archive(file: File, archive: &[u8], id: &str, name: &str) -> Result<()> {
    let mut reader = GzDecoder::new(archive).take(EXPANDED_BYTES + 1);
    let mut writer = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    writer.start_file("package.json", options)?;
    serde_json::to_writer(
        &mut writer,
        &serde_json::json!({ "name": id, "displayName": name }),
    )?;
    let mut entries = BTreeSet::new();
    let mut captured = BTreeSet::new();
    let options_dir = SimpleFileOptions::default();
    let mut long_name = None;
    let mut header = [0_u8; TAR_BLOCK];
    loop {
        reader
            .read_exact(&mut header)
            .context("truncated template archive")?;
        if header.iter().all(|&byte| byte == 0) {
            ensure!(long_name.is_none(), "archive ended before its named entry");
            reader
                .read_exact(&mut header)
                .context("truncated tar terminator")?;
            ensure!(
                header.iter().all(|&byte| byte == 0),
                "archive lacks a complete tar terminator"
            );
            let mut padding = [0; 8192];
            loop {
                let count = reader.read(&mut padding)?;
                if count == 0 {
                    break;
                }
                ensure!(
                    padding[..count].iter().all(|&byte| byte == 0),
                    "unexpected data after archive terminator"
                );
            }
            ensure!(reader.limit() > 0, "template exceeds expanded byte limit");
            break;
        }
        check_tar_checksum(&header)?;
        let size = octal(&header[124..136])?;
        let entry_type = header[156];
        if entry_type == b'L' {
            ensure!(
                long_name.is_none() && size <= 4096,
                "invalid extended tar pathname"
            );
            let mut path = vec![0; size as usize];
            reader.read_exact(&mut path)?;
            long_name = Some(tar_field(&path)?.to_owned());
        } else {
            ensure!(
                matches!(entry_type, 0 | b'0' | b'5'),
                "template tar contains an unsupported file type"
            );
            let path = match long_name.take() {
                Some(path) => path,
                None => tar_path(&header)?,
            };
            let path = safe_archive_path(path.as_str())?;
            let directory = entry_type == b'5';
            ensure!(!directory || size == 0, "tar directory has file contents");
            ensure!(
                entries.len() < MAX_ENTRIES && entries.insert(path.to_lowercase()),
                "duplicate or excessive template archive entries"
            );
            if allowed(path) {
                captured.insert(path.to_owned());
                if directory {
                    writer.add_directory(format!("{path}/"), options_dir)?;
                } else {
                    writer.start_file(path, options)?;
                    ensure!(
                        io::copy(&mut (&mut reader).take(size), &mut writer)? == size,
                        "truncated template member"
                    );
                }
            } else if !directory {
                ensure!(
                    io::copy(&mut (&mut reader).take(size), &mut io::sink())? == size,
                    "truncated template member"
                );
            }
        }
        let remainder = (TAR_BLOCK as u64 - size % TAR_BLOCK as u64) % TAR_BLOCK as u64;
        reader.read_exact(&mut header[..remainder as usize])?;
    }
    ensure!(
        reader.into_inner().get_ref().is_empty(),
        "unexpected data after the gzip member"
    );
    for root in ["Assets", "Packages", "ProjectSettings"] {
        if !captured.contains(root) {
            ensure!(
                captured.iter().any(|path| path
                    .strip_prefix(root)
                    .is_some_and(|suffix| suffix.starts_with('/'))),
                "template lacks a required project root"
            );
            writer.add_directory(format!("{root}/"), options_dir)?;
        }
    }
    ensure!(
        captured.contains("ProjectSettings/ProjectVersion.txt"),
        "template lacks project version settings"
    );
    let file = writer.finish()?;
    ensure!(
        file.metadata()?.len() <= TEMPLATE_BYTES,
        "template ZIP exceeds its byte limit"
    );
    file.sync_all()?;
    Ok(())
}

fn octal(bytes: &[u8]) -> Result<u64> {
    let value = std::str::from_utf8(bytes)?.trim_matches(['\0', ' ']);
    if value.is_empty() {
        return Ok(0);
    }
    u64::from_str_radix(value, 8).context("invalid archive number")
}

fn check_tar_checksum(header: &[u8; TAR_BLOCK]) -> Result<()> {
    let expected = octal(&header[148..156])?;
    let actual: u64 = header
        .iter()
        .enumerate()
        .map(|(index, &byte)| {
            if (148..156).contains(&index) {
                u64::from(b' ')
            } else {
                u64::from(byte)
            }
        })
        .sum();
    ensure!(expected == actual, "corrupt archive header");
    Ok(())
}

fn tar_field(bytes: &[u8]) -> Result<&str> {
    let value = bytes.split(|&byte| byte == 0).next().unwrap_or(bytes);
    Ok(std::str::from_utf8(value)?)
}

fn tar_path(header: &[u8; TAR_BLOCK]) -> Result<String> {
    let name = tar_field(&header[..100])?;
    if &header[257..263] == b"ustar\0" {
        let prefix = tar_field(&header[345..500])?;
        if !prefix.is_empty() {
            return Ok(format!("{prefix}/{name}"));
        }
    }
    Ok(name.to_owned())
}

fn safe_archive_path(path: &str) -> Result<&str> {
    let path = path
        .strip_prefix("./")
        .unwrap_or(path)
        .trim_end_matches('/');
    ensure!(
        !path.is_empty()
            && path.len() <= 4096
            && !path.contains(['\\', ':'])
            && !path.chars().any(char::is_control)
            && path.split('/').all(|part| !part.is_empty()
                && part != "."
                && part != ".."
                && !part.ends_with([' ', '.'])),
        "invalid template archive pathname"
    );
    Ok(path)
}

fn allowed(path: &str) -> bool {
    let first = path.split('/').next().unwrap_or_default();
    ["Assets", "Packages", "ProjectSettings"].contains(&first)
        || [
            "LICENSE",
            "LICENSE.md",
            "LICENSE.txt",
            "README.md",
            ".gitignore",
        ]
        .iter()
        .any(|name| path.eq_ignore_ascii_case(name))
}

#[derive(Serialize)]
pub(super) struct DirectoryTemplate {
    source: PathBuf,
    target: String,
    name: String,
    id: String,
    members: Vec<DirectoryMember>,
}

#[derive(Serialize)]
struct DirectoryMember {
    name: String,
    sha256: Option<[u8; 32]>,
}

pub(super) fn inventory_vcc(root: &Path) -> Result<Vec<DirectoryTemplate>> {
    let folder = root.join("Templates");
    match alcomd_platform::resolve_plain_storage_directory(&folder) {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(_) => anyhow::bail!("unsafe VCC template directory"),
    }
    let mut names = Vec::new();
    for entry in fs::read_dir(&folder)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_file() {
            continue;
        }
        ensure!(kind.is_dir(), "unsafe VCC template entry");
        ensure!(names.len() < MAX_TEMPLATES, "too many source templates");
        names.push(
            entry
                .file_name()
                .into_string()
                .map_err(|_| anyhow::anyhow!("invalid template name"))?,
        );
    }
    names.sort_unstable();
    let mut unique = BTreeSet::new();
    let mut total = 0;
    let mut total_members = 0;
    names
        .into_iter()
        .enumerate()
        .map(|(index, name)| {
            ensure!(
                name.trim() == name
                    && !name.is_empty()
                    && name.len() <= 128
                    && !name.chars().any(char::is_control)
                    && unique.insert(name.to_lowercase()),
                "invalid or duplicate VCC template name"
            );
            let source = Path::new("Templates").join(&name);
            let path = root.join(&source);
            let (_, identity) = alcomd_platform::resolve_plain_storage_directory(&path)?;
            let mut members = Vec::new();
            collect_directory_members(&path, "", &mut members, &mut total)?;
            validate_directory_members(&members)?;
            total_members += members.len();
            ensure!(
                total_members <= MAX_ENTRIES,
                "too many source template archive entries"
            );
            ensure!(
                alcomd_platform::resolve_plain_storage_directory(&path)?.1 == identity,
                "VCC template directory changed"
            );
            Ok(DirectoryTemplate {
                source,
                target: format!("templates/{:04}.alcomdtemplate", index + 1),
                id: format!(
                    "com.vrclearn.alcomd.migration.vcc.{}",
                    crate::hex_digest(&Sha256::digest(name.as_bytes()))
                ),
                name,
                members,
            })
        })
        .collect()
}

fn collect_directory_members(
    root: &Path,
    parent: &str,
    members: &mut Vec<DirectoryMember>,
    expanded: &mut u64,
) -> Result<()> {
    let folder = root.join(parent);
    let (_, before) = alcomd_platform::resolve_plain_storage_directory(&folder)?;
    let mut entries = Vec::new();
    for entry in fs::read_dir(&folder)? {
        ensure!(
            entries.len() < MAX_ENTRIES,
            "too many template archive entries"
        );
        entries.push(entry?);
    }
    entries.sort_unstable_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let leaf = entry
            .file_name()
            .into_string()
            .map_err(|_| anyhow::anyhow!("invalid template entry name"))?;
        ensure!(!leaf.contains(['/', '\\']), "invalid template entry name");
        let name = if parent.is_empty() {
            leaf
        } else {
            format!("{parent}/{leaf}")
        };
        if !allowed(&name) {
            continue;
        }
        ensure!(
            safe_archive_path(&name)? == name && name.split('/').count() <= 32,
            "unsafe template entry path"
        );
        ensure!(
            members.len() < MAX_ENTRIES,
            "too many template archive entries"
        );
        let kind = entry.file_type()?;
        let sha256 = if kind.is_file() {
            let bytes =
                read_file(root, &name, TEMPLATE_BYTES)?.context("template file disappeared")?;
            *expanded += bytes.len() as u64;
            ensure!(
                *expanded <= EXPANDED_BYTES,
                "template exceeds expanded byte limit"
            );
            Some(Sha256::digest(&bytes).into())
        } else {
            ensure!(kind.is_dir(), "unsafe template archive entry");
            None
        };
        let is_directory = sha256.is_none();
        members.push(DirectoryMember {
            name: name.clone(),
            sha256,
        });
        if is_directory {
            collect_directory_members(root, &name, members, expanded)?;
        }
    }
    ensure!(
        alcomd_platform::resolve_plain_storage_directory(&folder)?.1 == before,
        "template directory changed"
    );
    Ok(())
}

fn validate_directory_members(members: &[DirectoryMember]) -> Result<()> {
    let mut paths = BTreeSet::new();
    for member in members {
        ensure!(
            paths.insert(member.name.to_lowercase()),
            "duplicate template archive entry"
        );
    }
    for required in ["Assets", "Packages", "ProjectSettings"] {
        ensure!(
            members
                .iter()
                .any(|member| member.name == required && member.sha256.is_none()),
            "template lacks a required project root"
        );
    }
    ensure!(
        members
            .iter()
            .any(|member| member.name == "ProjectSettings/ProjectVersion.txt"
                && member.sha256.is_some()),
        "template lacks project version settings"
    );
    Ok(())
}

impl DirectoryTemplate {
    pub(super) fn relative(&self) -> &str {
        &self.target
    }

    pub(super) fn convert(&self, root: &Path, stage: &Path) -> Result<()> {
        let file = File::options()
            .write(true)
            .create_new(true)
            .open(stage.join(&self.target))?;
        let mut zip = zip::ZipWriter::new(file);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("package.json", options)?;
        serde_json::to_writer(
            &mut zip,
            &serde_json::json!({"name": self.id, "displayName": self.name}),
        )?;
        let source = root.join(&self.source);
        for member in &self.members {
            if let Some(expected) = member.sha256 {
                let bytes = read_file(&source, &member.name, TEMPLATE_BYTES)?
                    .context("template file disappeared")?;
                ensure!(
                    Sha256::digest(&bytes).as_slice() == expected,
                    "template changed after preview"
                );
                zip.start_file(&member.name, options)?;
                io::copy(&mut bytes.as_slice(), &mut zip)?;
            } else {
                alcomd_platform::resolve_plain_storage_directory(&source.join(&member.name))?;
                zip.add_directory(format!("{}/", member.name), SimpleFileOptions::default())?;
            }
        }
        let file = zip.finish()?;
        ensure!(
            file.metadata()?.len() <= TEMPLATE_BYTES,
            "template ZIP exceeds its byte limit"
        );
        file.sync_all()?;
        Ok(())
    }
}
