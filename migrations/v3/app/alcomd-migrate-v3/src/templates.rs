//! Reads published template metadata and converts project archives into current ZIPs.
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use alcomd_import::{Template, TemplateSource};
use anyhow::{Context, Result, ensure};
use flate2::bufread::GzDecoder;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use zip::write::SimpleFileOptions;

use crate::profile::{checked_path, read_file};

const TEMPLATE_BYTES: u64 = 256 * 1024 * 1024;
const EXPANDED_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_TEMPLATES: usize = 128;
const MAX_ENTRIES: usize = 65_536;
const HEADER_BYTES: usize = 256 * 1024;
const TAR_BLOCK: usize = 512;
const OLD_KIND: &str = "com.anatawa12.vrc-get.custom-template";

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

pub fn inventory(root: &Path) -> Result<(Vec<Template>, Vec<Archive>)> {
    let mut templates = Vec::new();
    let mut attachments = Vec::new();
    let mut id_claims = BTreeSet::new();
    let mut mappings = BTreeMap::new();
    for dir_name in ["templates", "vrc-get/templates"] {
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
                !is_builtin(&id) && id_claims.insert(id.clone()),
                "duplicate source template identifier"
            );
            if let Some(old_id) = entry.id {
                mappings.insert(old_id, id.clone());
            }
            let source = match (
                entry.format_version.as_str(),
                entry.template_kind.as_deref(),
                entry.archive,
            ) {
                ("1.0", None | Some("derived"), None) => {
                    ensure!(content.is_empty(), "unexpected derived template payload");
                    let base_template = entry.base.context("derived template lacks a base")?;
                    let unity_range = entry
                        .unity_version
                        .context("derived template lacks a Unity range")?;
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
            } else if let Some(id) = builtin(base_template) {
                *base_template = id.into();
            }
            ensure!(
                is_builtin(base_template) || id_claims.contains(base_template),
                "unresolved template base"
            );
        }
    }
    Ok((templates, attachments))
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

pub fn builtin(id: &str) -> Option<&'static str> {
    match id {
        "com.anatawa12.vrc-get.blank" => Some("com.vrclearn.alcomd.templates.blank"),
        "com.anatawa12.vrc-get.vrchat.avatars" => {
            Some("com.vrclearn.alcomd.templates.vrchat.avatars")
        }
        "com.anatawa12.vrc-get.vrchat.worlds" => {
            Some("com.vrclearn.alcomd.templates.vrchat.worlds")
        }
        _ => None,
    }
}

fn is_builtin(id: &str) -> bool {
    [
        "com.vrclearn.alcomd.templates.blank",
        "com.vrclearn.alcomd.templates.vrchat.avatars",
        "com.vrclearn.alcomd.templates.vrchat.worlds",
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
