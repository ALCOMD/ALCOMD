//! Native state exchanged between a migration producer and the v4 staging importer.
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Derives an owner-authorized vault reference from a migration transaction.
/// The index is zero-based among source repositories that carry credentials.
#[must_use]
pub fn migration_credential_reference(transfer_id: uuid::Uuid, index: u32) -> uuid::Uuid {
    use sha2::{Digest, Sha256};

    let mut hash = Sha256::new();
    hash.update(b"alcomd:migration-credential:v1\0");
    hash.update(transfer_id.as_bytes());
    hash.update(index.to_be_bytes());
    let digest = hash.finalize();
    let mut identifier = [0; 16];
    identifier.copy_from_slice(&digest[..16]);
    identifier[6] = (identifier[6] & 0x0f) | 0x80;
    identifier[8] = (identifier[8] & 0x3f) | 0x80;
    uuid::Uuid::from_bytes(identifier)
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeState {
    pub schema: u32,
    pub source_version: String,
    pub projects: Vec<Project>,
    pub unity_installations: Vec<UnityInstallation>,
    pub repositories: Vec<Repository>,
    pub local_packages: Vec<String>,
    pub templates: Vec<Template>,
    pub settings: Settings,
    pub extensions: Vec<ExtensionPreference>,
    pub discord: DiscordPreference,
}

impl NativeState {
    pub fn counts(&self) -> BTreeMap<&'static str, usize> {
        BTreeMap::from([
            ("projects", self.projects.len()),
            ("unityInstallations", self.unity_installations.len()),
            ("repositories", self.repositories.len()),
            (
                "repositoryCredentials",
                self.repositories
                    .iter()
                    .filter(|repo| repo.has_credentials)
                    .count(),
            ),
            ("localPackages", self.local_packages.len()),
            ("templates", self.templates.len()),
            ("settings", 1),
            ("extensions", self.extensions.len()),
            ("discord", 1),
        ])
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Project {
    pub path: String,
    pub favorite: bool,
    pub registered_at_ms: Option<u64>,
    pub last_modified_at_ms: Option<u64>,
    pub unity_version: Option<String>,
    pub unity_revision: Option<String>,
    pub unity_executable_path: Option<String>,
    pub unity_launch_arguments: Option<Vec<String>>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UnityInstallation {
    pub executable_path: String,
    pub version: Option<String>,
    pub from_hub: bool,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Repository {
    pub source: RepositorySource,
    pub declared_id: Option<String>,
    pub display_name: Option<String>,
    pub hidden: bool,
    pub has_credentials: bool,
    pub credential_ref: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum RepositorySource {
    Local { path: String },
    Remote { url: String },
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub locale: String,
    pub appearance_mode: String,
    pub appearance_motion: String,
    pub fullscreen: bool,
    pub handle_vcc_links: bool,
    pub show_prerelease: bool,
    pub hide_local_user_packages: bool,
    pub hide_official_repository: bool,
    pub hide_curated_repository: bool,
    pub project_directory: Option<String>,
    pub backup_directory: Option<String>,
    pub hub_path: Option<String>,
    pub default_unity_launch_arguments: Option<Vec<String>>,
    pub backup_compression: String,
    pub favorite_template_ids: Vec<String>,
    pub last_selected_template_id: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Template {
    pub id: Option<String>,
    pub name: String,
    pub source: TemplateSource,
}

/// Orders a bounded transfer inventory so a derived template can read its saved base.
pub fn order_template_bases(mut pending: Vec<Template>) -> Result<Vec<Template>, &'static str> {
    if pending.len() > 128 {
        return Err("too many staged templates");
    }
    let mut pending_ids = std::collections::BTreeSet::new();
    for template in &pending {
        if let Some(id) = &template.id
            && !pending_ids.insert(id.clone())
        {
            return Err("duplicate template identifier");
        }
        if matches!(&template.source, TemplateSource::Manual { base_template, .. } if base_template.is_empty())
        {
            return Err("manual template requires a base");
        }
    }
    let mut ordered = Vec::with_capacity(pending.len());
    while !pending.is_empty() {
        let next = pending
            .iter()
            .position(|template| match &template.source {
                TemplateSource::Manual { base_template, .. } => {
                    !pending_ids.contains(base_template)
                }
                TemplateSource::ProjectArchive { .. } => true,
            })
            .ok_or("cyclic template base references")?;
        let template = pending.remove(next);
        if let Some(id) = &template.id {
            pending_ids.remove(id);
        }
        ordered.push(template);
    }
    Ok(ordered)
}

#[derive(Deserialize, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum TemplateSource {
    Manual {
        base_template: String,
        unity_range: String,
        dependencies: BTreeMap<String, String>,
        unity_packages: Vec<String>,
    },
    ProjectArchive {
        unity_version: String,
        archive_path: String,
    },
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtensionPreference {
    pub id: String,
    pub enabled: bool,
    pub visible: bool,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiscordPreference {
    pub enabled: bool,
    pub project_name: bool,
    pub unity_version: bool,
    pub editor_count: bool,
    pub custom_text: String,
}

#[cfg(test)]
mod tests {
    use super::migration_credential_reference;

    #[test]
    fn vault_references_are_deterministic_and_bound_to_the_transfer_and_index() {
        let first = uuid::Uuid::from_u128(1);
        let second = uuid::Uuid::from_u128(2);
        let reference = migration_credential_reference(first, 0);
        assert_eq!(reference, migration_credential_reference(first, 0));
        assert_ne!(reference, migration_credential_reference(first, 1));
        assert_ne!(reference, migration_credential_reference(second, 0));
        assert_eq!(reference.get_version_num(), 8);
        assert_eq!(reference.get_variant(), uuid::Variant::RFC4122);
    }
}
