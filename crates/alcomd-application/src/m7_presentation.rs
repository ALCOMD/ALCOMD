//! Read-only presentation observations. These are not package mutation authority.
use serde::{Deserialize, Serialize};

use crate::CandidateLinks;

pub const MAX_PRESENTATION_PACKAGES: usize = 4_096;
pub const MAX_PRESENTATION_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_PRESENTATION_TERMS: usize = 128;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PresentationState {
    Complete,
    Partial,
    #[default]
    Unobserved,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackageDisplayMetadata {
    pub yanked: bool,
    pub aliases: Vec<String>,
    pub keywords: Vec<String>,
    pub legacy_packages: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstalledPackagePresentation {
    pub package_id: String,
    pub version: String,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub metadata: PackageDisplayMetadata,
    pub links: CandidateLinks,
    pub yanked: bool,
    pub manifest_sha256: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectPackagePresentations {
    pub fingerprint: String,
    pub state: PresentationState,
    pub packages: Vec<InstalledPackagePresentation>,
}

use serde_json::Value;
use std::collections::BTreeSet;

fn strings(value: Option<&Value>, package_ids: bool) -> Option<Vec<String>> {
    let Some(value) = value else {
        return Some(Vec::new());
    };
    let values = value.as_array()?;
    if values.len() > MAX_PRESENTATION_TERMS {
        return None;
    }
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for value in values {
        let value = value.as_str()?;
        if value.is_empty()
            || value.len() > 256
            || value.chars().any(char::is_control)
            || (package_ids && !valid_presentation_package_id(value))
        {
            return None;
        }
        if seen.insert(value) {
            result.push(value.to_owned());
        }
    }
    Some(result)
}

pub fn valid_presentation_package_id(value: &str) -> bool {
    value.len() <= 128
        && value
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
}

pub fn package_yanked(manifest: &serde_json::Map<String, Value>) -> Option<bool> {
    let top = match manifest.get("yanked") {
        None => false,
        Some(Value::Bool(value)) => *value,
        _ => return None,
    };
    let extension = match manifest.get("vrc-get") {
        None => None,
        Some(value) => Some(value.as_object()?),
    };
    let extension = match extension.and_then(|value| value.get("yanked")) {
        None => false,
        Some(Value::Bool(value)) => *value,
        Some(Value::String(value)) if value.len() <= 4_096 => true,
        _ => return None,
    };
    Some(top || extension)
}

pub fn normalize_package_display_metadata(manifest: &Value) -> Option<PackageDisplayMetadata> {
    let manifest = manifest.as_object()?;
    let extension = match manifest.get("vrc-get") {
        None => None,
        Some(value) => Some(value.as_object()?),
    };
    Some(PackageDisplayMetadata {
        yanked: package_yanked(manifest)?,
        aliases: strings(
            extension.and_then(|extension| extension.get("aliases")),
            false,
        )?,
        keywords: strings(manifest.get("keywords"), false)?,
        legacy_packages: strings(manifest.get("legacyPackages"), true)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn withdrawal_authority_is_independent_of_optional_presentation_fields() {
        let value = json!({"keywords":false,"vrc-get":{"aliases":false,"yanked":"withdrawn"}});
        assert_eq!(
            package_yanked(value.as_object().expect("manifest")),
            Some(true)
        );
        assert!(normalize_package_display_metadata(&value).is_none());
        assert_eq!(
            package_yanked(
                json!({"yanked":false,"vrc-get":{"yanked":true}})
                    .as_object()
                    .expect("manifest")
            ),
            Some(true)
        );
        assert_eq!(
            package_yanked(
                json!({"vrc-get":{"yanked":42}})
                    .as_object()
                    .expect("manifest")
            ),
            None
        );
    }
    #[test]
    fn metadata_normalization_is_bounded_and_understands_extensions() {
        let value = normalize_package_display_metadata(&json!({"keywords":["search","search"], "legacyPackages":["com.old.package"], "vrc-get":{"aliases":["alias"],"yanked":"withdrawn"}})).expect("presentation fixture or query succeeds");
        assert_eq!(value.keywords, ["search"]);
        assert_eq!(value.aliases, ["alias"]);
        assert_eq!(value.legacy_packages, ["com.old.package"]);
        assert!(value.yanked);
        for invalid in [
            json!(false),
            json!({"keywords":["\n"]}),
            json!({"legacyPackages":["../outside"]}),
            json!({"vrc-get":{"aliases":false}}),
            json!({"keywords": vec!["x";129]}),
        ] {
            assert!(normalize_package_display_metadata(&invalid).is_none());
        }
    }
}
