use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use alcomd_application::{Revision, StoredTemplateRecord, TemplateId, TemplateSourceKind};
use serde::Deserialize;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use zip::CompressionMethod;
use zip::write::SimpleFileOptions;

use crate::{
    TemplateDependency, TemplateEngine, TemplateManifest, TemplatePayload, TemplateProvenance,
    TemplateProvenanceKind, TemplateUnityCompatibility, inspect_template_bundle,
};

const BLANK: &str = include_str!("../../../specs/templates/builtin-scaffolds/blank-v1.json");
const AVATARS: &str = include_str!("../../../specs/templates/builtin-scaffolds/avatars-v1.json");
const WORLDS: &str = include_str!("../../../specs/templates/builtin-scaffolds/worlds-v1.json");

struct BuiltinDefinition {
    template_id: &'static str,
    template_version: &'static str,
    display_name: &'static str,
    description: &'static str,
    dependency: Option<&'static str>,
    descriptor: &'static str,
}

const DEFINITIONS: [BuiltinDefinition; 3] = [
    BuiltinDefinition {
        template_id: "7e2233c8-0b3f-4cf2-aeb4-57d3d240b001",
        template_version: "1",
        display_name: "Blank",
        description: "Minimal independently authored Unity project scaffold.",
        dependency: None,
        descriptor: BLANK,
    },
    BuiltinDefinition {
        template_id: "7e2233c8-0b3f-4cf2-aeb4-57d3d240b002",
        template_version: "1",
        display_name: "VRChat Avatars",
        description: "Unity project scaffold whose VRChat SDK content is resolved as a VPM dependency.",
        dependency: Some("com.vrchat.avatars"),
        descriptor: AVATARS,
    },
    BuiltinDefinition {
        template_id: "7e2233c8-0b3f-4cf2-aeb4-57d3d240b003",
        template_version: "1",
        display_name: "VRChat Worlds",
        description: "Unity project scaffold whose VRChat SDK content is resolved as a VPM dependency.",
        dependency: Some("com.vrchat.worlds"),
        descriptor: WORLDS,
    },
];

impl TemplateEngine {
    /// Materializes the three machine-gated native builtins into the durable object store.
    pub fn materialize_builtins(
        &self,
        staging: &Path,
    ) -> Result<Vec<StoredTemplateRecord>, alcomd_application::M5TemplateError> {
        std::fs::create_dir_all(staging).map_err(|_| internal())?;
        let mut records = Vec::with_capacity(DEFINITIONS.len());
        for definition in DEFINITIONS {
            let path = staging.join(format!("{}.alcomdtemplate", definition.template_id));
            let _ = std::fs::remove_file(&path);
            write_builtin_bundle(&path, &definition)?;
            let inspection = inspect_template_bundle(&path).map_err(|_| internal())?;
            let object = self
                .objects
                .publish(&path, inspection.bundle_sha256)
                .map_err(super::template_engine::map_object_error)?;
            std::fs::remove_file(&path).map_err(|_| internal())?;
            let template_id = TemplateId::parse(definition.template_id).map_err(|_| internal())?;
            records.push(StoredTemplateRecord {
                template_id,
                source_kind: TemplateSourceKind::Builtin,
                template_version: definition.template_version.to_owned(),
                manifest_json: inspection.normalized_manifest_json,
                payload_locator: format!(
                    "builtin:{}@{}",
                    definition.template_id, definition.template_version
                ),
                bundle_sha256: object.digest,
                favorite: false,
                revision: Revision::INITIAL,
                created_at_ms: 0,
                updated_at_ms: 0,
            });
        }
        alcomd_platform::sync_directory(staging).map_err(|_| internal())?;
        Ok(records)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Descriptor {
    descriptor_version: u32,
    family: String,
    files: Vec<DescriptorFile>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DescriptorFile {
    path: String,
    utf8: String,
}

fn write_builtin_bundle(
    path: &Path,
    definition: &BuiltinDefinition,
) -> Result<(), alcomd_application::M5TemplateError> {
    let descriptor: Descriptor =
        serde_json::from_str(definition.descriptor).map_err(|_| internal())?;
    if descriptor.descriptor_version != 1 || descriptor.family.is_empty() {
        return Err(internal());
    }
    let mut files = BTreeMap::new();
    for file in descriptor.files {
        if files.insert(file.path, file.utf8.into_bytes()).is_some() {
            return Err(internal());
        }
    }
    let total_bytes = files.values().try_fold(0_u64, |total, bytes| {
        total.checked_add(bytes.len() as u64).ok_or_else(internal)
    })?;
    let tree_digest = payload_tree_digest(&files)?;
    let dependencies = definition
        .dependency
        .map(|package_id| {
            vec![TemplateDependency {
                package_id: package_id.to_owned(),
                version_range: ">=3.0.0".to_owned(),
                include_prerelease: false,
            }]
        })
        .unwrap_or_default();
    let manifest = TemplateManifest {
        format_version: 1,
        template_id: definition.template_id.to_owned(),
        template_version: definition.template_version.to_owned(),
        display_name: definition.display_name.to_owned(),
        description: Some(definition.description.to_owned()),
        unity: TemplateUnityCompatibility {
            major: 2022,
            minor: 3,
        },
        dependencies,
        additional_resources: Vec::new(),
        payload: TemplatePayload {
            root: "payload/".to_owned(),
            tree_sha256: hex(&tree_digest),
            entry_count: files.len() as u64,
            total_bytes,
        },
        provenance: TemplateProvenance {
            created_by: TemplateProvenanceKind::Authored,
            derived_from_template_id: None,
            derived_from_project_id: None,
        },
    };
    let manifest = canonical_json(&manifest)?;
    let output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| internal())?;
    let mut archive = zip::ZipWriter::new(output);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .unix_permissions(0o644);
    archive
        .start_file("template.json", options)
        .and_then(|()| archive.write_all(manifest.as_bytes()).map_err(Into::into))
        .map_err(|_| internal())?;
    for (name, bytes) in files {
        archive
            .start_file(format!("payload/{name}"), options)
            .and_then(|()| archive.write_all(&bytes).map_err(Into::into))
            .map_err(|_| internal())?;
    }
    let output = archive.finish().map_err(|_| internal())?;
    output.sync_all().map_err(|_| internal())?;
    Ok(())
}

fn payload_tree_digest(
    files: &BTreeMap<String, Vec<u8>>,
) -> Result<[u8; 32], alcomd_application::M5TemplateError> {
    let mut digest = Sha256::new();
    for (path, bytes) in files {
        digest.update(
            u32::try_from(path.len())
                .map_err(|_| internal())?
                .to_le_bytes(),
        );
        digest.update(path.as_bytes());
        digest.update(
            u64::try_from(bytes.len())
                .map_err(|_| internal())?
                .to_le_bytes(),
        );
        digest.update(bytes);
    }
    Ok(digest.finalize().into())
}

fn canonical_json(
    value: &impl serde::Serialize,
) -> Result<String, alcomd_application::M5TemplateError> {
    let mut value = serde_json::to_value(value).map_err(|_| internal())?;
    sort_json(&mut value);
    serde_json::to_string(&value).map_err(|_| internal())
}

fn sort_json(value: &mut Value) {
    match value {
        Value::Object(object) => {
            let mut sorted = std::mem::take(object)
                .into_iter()
                .collect::<BTreeMap<_, _>>();
            for value in sorted.values_mut() {
                sort_json(value);
            }
            *object = sorted.into_iter().collect::<Map<_, _>>();
        }
        Value::Array(values) => values.iter_mut().for_each(sort_json),
        _ => {}
    }
}

fn hex(value: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(64);
    for byte in value {
        result.push(char::from(HEX[(byte >> 4) as usize]));
        result.push(char::from(HEX[(byte & 0x0f) as usize]));
    }
    result
}

fn internal() -> alcomd_application::M5TemplateError {
    alcomd_application::M5TemplateError::new(alcomd_application::M5TemplateErrorCode::Internal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQUENCE: AtomicU64 = AtomicU64::new(1);

    #[test]
    fn builtin_materialization_matches_stable_ids_and_tree_digest() {
        let root = std::env::temp_dir().join(format!(
            "alcomd-builtins-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let engine = TemplateEngine::new(root.join("objects")).expect("engine");
        let records = engine
            .materialize_builtins(&root.join("staging"))
            .expect("builtins");
        assert_eq!(records.len(), 3);
        assert_eq!(
            records[0].template_id.to_string(),
            DEFINITIONS[0].template_id
        );
        assert!(records.iter().all(|record| {
            record.source_kind == TemplateSourceKind::Builtin
                && record.payload_locator.starts_with("builtin:")
        }));
        std::fs::remove_dir_all(root).expect("remove fixture");
    }
}
