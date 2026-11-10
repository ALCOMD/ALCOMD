//! Versioned repository configuration documents. Secrets only leave Core in the file.
use super::*;
use alcomd_application::RepositoryConfiguration;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    format: Format,
    version: u32,
    repositories: Vec<Entry>,
}

#[derive(Serialize, Deserialize)]
enum Format {
    #[serde(rename = "alcomd-repositories")]
    Repositories,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Entry {
    source: Source,
    configuration: RepositoryConfiguration,
    credentials: Option<RepositoryCredentials>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
enum Source {
    Remote { url: String },
    Local { path: String },
}

impl From<RepositorySource> for Source {
    fn from(source: RepositorySource) -> Self {
        match source {
            RepositorySource::Remote { url } => Self::Remote { url },
            RepositorySource::Local { path } => Self::Local { path },
        }
    }
}

impl From<Source> for RepositorySource {
    fn from(source: Source) -> Self {
        match source {
            Source::Remote { url } => Self::Remote { url },
            Source::Local { path } => Self::Local { path },
        }
    }
}

pub(super) fn parse(
    bytes: &[u8],
    text: &str,
    snapshot: &RepositoryListSnapshot,
) -> Result<RepositoryListInspection, Error> {
    let document: Document =
        serde_json::from_str(text).map_err(|_| Error("repository_list_invalid"))?;
    if document.version != 1 || document.repositories.len() > REPOSITORY_LIST_MAX_ENTRIES {
        return Err(Error("repository_list_invalid"));
    }
    let mut seen = HashSet::new();
    let mut total_disabled = 0;
    let mut disabled_repositories = 0;
    let mut package_groups = 0;
    let mut entries = Vec::new();
    for (index, mut entry) in document.repositories.into_iter().enumerate() {
        if entry.configuration.priority == 0
            || entry
                .configuration
                .display_name
                .as_ref()
                .is_some_and(|name| {
                    name.chars().count() > 128 || name.chars().any(char::is_control)
                })
        {
            return Err(Error("repository_list_invalid"));
        }
        entry.configuration.display_name = entry
            .configuration
            .display_name
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_owned);
        entry.configuration.disabled_package_ids.sort();
        if entry
            .configuration
            .disabled_package_ids
            .windows(2)
            .any(|ids| ids[0] == ids[1])
        {
            return Err(Error("repository_list_invalid"));
        }
        for id in &entry.configuration.disabled_package_ids {
            crate::package::parse_dependencies(Some(&serde_json::json!({id:"*"})))
                .map_err(|_| Error("repository_list_invalid"))?;
        }
        total_disabled += entry.configuration.disabled_package_ids.len();
        disabled_repositories += usize::from(!entry.configuration.enabled);
        package_groups += usize::from(!entry.configuration.disabled_package_ids.is_empty());
        if total_disabled > 4096 || disabled_repositories > 256 || package_groups > 256 {
            return Err(Error("repository_list_invalid"));
        }
        let mut source: RepositorySource = entry.source.into();
        match &mut source {
            RepositorySource::Remote { url } => {
                if !url.starts_with("https://") && !url.starts_with("http://") {
                    return Err(Error("repository_list_invalid"));
                }
                let (normalized, _) =
                    parse_source(url).map_err(|_| Error("repository_list_invalid"))?;
                *url = normalized;
            }
            RepositorySource::Local { path } => {
                if path.is_empty()
                    || path.len() > 32768
                    || path.chars().any(char::is_control)
                    || entry.credentials.is_some()
                {
                    return Err(Error("repository_list_invalid"));
                }
            }
        }
        if let Some(credentials) = &entry.credentials {
            crate::repository_auth::RepositoryAuth::new(credentials)
                .map_err(|_| Error("repository_list_invalid"))?;
        }
        let source_key =
            serde_json::to_string(&source).map_err(|_| Error("repository_list_invalid"))?;
        if !seen.insert(source_key) {
            return Err(Error("repository_list_invalid"));
        }
        let duplicate = snapshot
            .repositories
            .iter()
            .any(|record| record.source == source);
        entries.push(RepositoryListEntry {
            line: (index + 1) as u32,
            status: if duplicate {
                Status::Duplicate
            } else {
                Status::Candidate
            },
            source: Some(source),
            reason: None,
            credentials_configured: entry.credentials.is_some(),
            credentials: entry.credentials,
            configuration: Some(entry.configuration),
        });
    }
    Ok(RepositoryListInspection {
        format: RepositoryListFormat::Native,
        entries,
        ignored_lines: 0,
        fingerprint: hex(&Sha256::digest(bytes)),
    })
}

pub(super) fn prepare(
    records: Vec<RepositoryListRecord>,
    load_credentials: impl Fn(&str) -> Result<RepositoryCredentials, alcomd_application::M3Error>,
) -> Result<PreparedRepositoryList, Error> {
    if records.len() > REPOSITORY_LIST_MAX_ENTRIES {
        return Err(Error("repository_list_invalid"));
    }
    let mut records = records;
    records.sort_by(|a, b| {
        a.configuration
            .priority
            .cmp(&b.configuration.priority)
            .then_with(|| a.repository_id.cmp(&b.repository_id))
    });
    let mut repositories = Vec::new();
    for record in records {
        let credentials = record
            .credential_reference
            .map(|reference| {
                load_credentials(&reference).map_err(|_| Error("repository_list_io_error"))
            })
            .transpose()?;
        repositories.push(Entry {
            source: record.source.into(),
            configuration: record.configuration,
            credentials,
        });
    }
    let count = repositories.len() as u32;
    let mut bytes = serde_json::to_vec_pretty(&Document {
        format: Format::Repositories,
        version: 1,
        repositories,
    })
    .map_err(|_| Error("repository_list_invalid"))?;
    bytes.push(b'\n');
    if bytes.len() > REPOSITORY_LIST_MAX_BYTES {
        return Err(Error("repository_list_invalid"));
    }
    let sha256 = hex(&Sha256::digest(&bytes));
    Ok(PreparedRepositoryList {
        preview: RepositoryListPreview {
            exportable_count: count,
            excluded: Vec::new(),
            byte_count: bytes.len() as u32,
            fingerprint: sha256.clone(),
        },
        bytes,
        sha256,
    })
}
