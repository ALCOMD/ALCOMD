//! Independent v3 text-list format support; never reads a legacy database.
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use alcomd_application::{
    PreparedRepositoryList, REPOSITORY_LIST_MAX_BYTES, REPOSITORY_LIST_MAX_ENTRIES,
    RepositoryCredentials, RepositoryListEntry, RepositoryListEntryStatus as Status,
    RepositoryListError as Error, RepositoryListExclusion, RepositoryListFiles,
    RepositoryListInspection, RepositoryListPreview, RepositoryListRecord, RepositoryListSnapshot,
    RepositorySource, VRCHAT_CURATED_REPOSITORY_ID, VRCHAT_OFFICIAL_REPOSITORY_ID,
};
use alcomd_platform::file_identity_key;
use reqwest::Url;
use sha2::{Digest, Sha256};

#[derive(Clone)]
pub struct RepositoryListFileAdapter {
    data_root: PathBuf,
}

impl RepositoryListFileAdapter {
    pub fn new(data_root: PathBuf) -> Self {
        Self { data_root }
    }
}

impl RepositoryListFiles for RepositoryListFileAdapter {
    async fn inspect(
        &self,
        path: String,
        snapshot: RepositoryListSnapshot,
    ) -> Result<RepositoryListInspection, Error> {
        tokio::task::spawn_blocking(move || {
            let path = Path::new(&path);
            if !path.is_absolute() || path.as_os_str().len() > 32768 {
                return Err(Error("repository_list_invalid"));
            }
            let metadata =
                fs::symlink_metadata(path).map_err(|_| Error("repository_list_io_error"))?;
            if !metadata.is_file() || crate::is_link_or_reparse(&metadata) {
                return Err(Error("repository_list_invalid"));
            }
            if metadata.len() > REPOSITORY_LIST_MAX_BYTES as u64 {
                return Err(Error("repository_list_invalid"));
            }
            let mut file = File::open(path).map_err(|_| Error("repository_list_io_error"))?;
            if !file
                .metadata()
                .map_err(|_| Error("repository_list_io_error"))?
                .is_file()
            {
                return Err(Error("repository_list_invalid"));
            }
            let mut bytes = Vec::new();
            Read::by_ref(&mut file)
                .take(REPOSITORY_LIST_MAX_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| Error("repository_list_io_error"))?;
            parse_list(&bytes, &snapshot)
        })
        .await
        .map_err(|_| Error("repository_list_io_error"))?
    }

    async fn prepare(
        &self,
        records: Vec<RepositoryListRecord>,
    ) -> Result<PreparedRepositoryList, Error> {
        tokio::task::spawn_blocking(move || {
            prepare_list(&records, crate::repository_auth::load_from_vault)
        })
        .await
        .map_err(|_| Error("repository_list_io_error"))?
    }

    async fn create_new(
        &self,
        path: String,
        bytes: Vec<u8>,
        project_roots: Vec<String>,
    ) -> Result<(), Error> {
        let data_root = self.data_root.clone();
        tokio::task::spawn_blocking(move || {
            create_new_list(Path::new(&path), &bytes, &data_root, &project_roots)
        })
        .await
        .map_err(|_| Error("repository_list_io_error"))?
    }
}

fn parse_list(
    bytes: &[u8],
    snapshot: &RepositoryListSnapshot,
) -> Result<RepositoryListInspection, Error> {
    if bytes.len() > REPOSITORY_LIST_MAX_BYTES {
        return Err(Error("repository_list_invalid"));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| Error("repository_list_invalid"))?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut seen: HashSet<String> = snapshot
        .repositories
        .iter()
        .filter_map(|record| match &record.source {
            RepositorySource::Remote { url } => Some(url.clone()),
            RepositorySource::Local { .. } => None,
        })
        .collect();
    let mut entries = Vec::new();
    let mut ignored_lines = 0;
    for (index, line) in text.lines().enumerate() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            ignored_lines += 1;
            continue;
        }
        if entries.len() == REPOSITORY_LIST_MAX_ENTRIES {
            return Err(Error("repository_list_invalid"));
        }
        let mut entry = RepositoryListEntry {
            line: (index + 1) as u32,
            status: Status::Invalid,
            source: None,
            reason: None,
            credentials_configured: false,
            credentials: None,
        };
        match parse_source(line) {
            Ok((url, credentials)) => {
                entry.status = if seen.insert(url.clone()) {
                    Status::Candidate
                } else {
                    Status::Duplicate
                };
                entry.source = Some(RepositorySource::Remote { url });
                entry.credentials_configured = credentials.is_some();
                entry.credentials = credentials;
                if entry.status == Status::Duplicate {
                    entry.reason = Some("duplicate_source");
                }
            }
            Err((status, reason)) => {
                entry.status = status;
                entry.reason = Some(reason);
            }
        }
        entries.push(entry);
    }
    Ok(RepositoryListInspection {
        fingerprint: hex(&Sha256::digest(bytes)),
        entries,
        ignored_lines,
    })
}

fn parse_source(
    text: &str,
) -> Result<(String, Option<alcomd_application::RepositoryCredentials>), (Status, &'static str)> {
    if text.len() > 16 * 1024 || text.chars().any(char::is_control) {
        return Err((Status::Invalid, "source_invalid"));
    }
    let url = Url::parse(text).map_err(|_| (Status::Invalid, "source_invalid"))?;
    if !url.username().is_empty() || url.password().is_some() {
        return Err((Status::Unsupported, "authentication_not_supported"));
    }
    if url.scheme() == "vcc" {
        if url.host_str() != Some("vpm") || url.path() != "/addRepo" {
            return Err((Status::Unsupported, "vcc_link_not_supported"));
        }
        let mut source = None;
        let mut headers = std::collections::BTreeMap::new();
        for (key, value) in url.query_pairs() {
            match key.as_ref() {
                "url" => {
                    if source.replace(value.into_owned()).is_some() {
                        return Err((Status::Invalid, "source_invalid"));
                    }
                }
                "headers[]" => {
                    let (name, value) = value
                        .split_once(':')
                        .ok_or((Status::Invalid, "source_invalid"))?;
                    headers.insert(name.to_owned(), value.to_owned());
                }
                _ => {}
            }
        }
        let source = source.ok_or((Status::Invalid, "source_invalid"))?;
        // Nested links are not a repository address.
        let nested = Url::parse(&source).map_err(|_| (Status::Invalid, "source_invalid"))?;
        if !matches!(nested.scheme(), "http" | "https") {
            return Err((Status::Unsupported, "scheme_not_supported"));
        }
        let (normalized, _) = parse_source(&source)?;
        if headers.is_empty() {
            return Ok((normalized, None));
        }
        let credentials = alcomd_application::RepositoryCredentials { headers };
        let auth = crate::repository_auth::RepositoryAuth::new(&credentials)
            .map_err(|_| (Status::Invalid, "source_invalid"))?;
        if auth.contains_url_secret(&nested) {
            return Err((Status::Invalid, "source_invalid"));
        }
        return Ok((normalized, Some(credentials)));
    }
    if !matches!(url.scheme(), "http" | "https") {
        return Err((Status::Unsupported, "scheme_not_supported"));
    }
    let normalized = crate::normalize_remote_url(text)
        .map_err(|_| (Status::Invalid, "source_invalid"))?
        .to_string();
    if normalized.len() > 2048 {
        return Err((Status::Invalid, "source_invalid"));
    }
    Ok((normalized, None))
}

#[test]
fn vcc_list_headers_are_excluded_from_the_normalized_http_source() {
    let mut link = Url::parse("vcc://vpm/addRepo").expect("VCC link");
    link.query_pairs_mut()
        .append_pair("url", "http://repository.test/index.json")
        .append_pair("future", "ignored")
        .append_pair("headers[]", "Authorization:obsolete")
        .append_pair("headers[]", "Authorization:Bearer private-fixture-token");
    let (source, credentials) = parse_source(link.as_str()).expect("authenticated VCC link");
    let credentials = credentials.expect("write-only header");
    assert_eq!(source, "http://repository.test/index.json");
    assert_eq!(
        credentials.headers["Authorization"],
        "Bearer private-fixture-token"
    );
    assert!(!format!("{credentials:?}").contains("private-fixture-token"));

    for unsafe_source in [
        "https://repository.test/private%2Dfixture-token",
        "https://repository.test/?key=private%2Dfixture-token",
    ] {
        link.set_query(None);
        link.query_pairs_mut()
            .append_pair("url", unsafe_source)
            .append_pair("headers[]", "Authorization: Bearer private-fixture-token");
        assert!(parse_source(link.as_str()).is_err());
    }
}

fn prepare_list(
    records: &[RepositoryListRecord],
    load_credentials: impl Fn(&str) -> Result<RepositoryCredentials, alcomd_application::M3Error>,
) -> Result<PreparedRepositoryList, Error> {
    if records.len() > REPOSITORY_LIST_MAX_ENTRIES {
        return Err(Error("repository_list_invalid"));
    }
    let mut records: Vec<_> = records.iter().collect();
    records.sort_by(|left, right| left.repository_id.cmp(&right.repository_id));
    let mut bytes = Vec::new();
    let mut excluded = Vec::new();
    let mut scope = Vec::new();
    let mut exportable_count = 0;
    for record in records {
        if matches!(
            record.repository_id.as_str(),
            VRCHAT_OFFICIAL_REPOSITORY_ID | VRCHAT_CURATED_REPOSITORY_ID
        ) {
            continue;
        }
        // The fingerprint includes scope and revisions, including excluded sources.
        scope.push(serde_json::json!([
            record.repository_id,
            record.revision,
            record.source,
            record.credential_reference
        ]));
        let reason = match &record.source {
            RepositorySource::Local { .. } => Some("local_source"),
            RepositorySource::Remote { url } => match parse_source(url) {
                Ok(_) if !url.contains(['#', '\r', '\n']) => {
                    if let Some(reference) = &record.credential_reference {
                        let credentials = load_credentials(reference)
                            .map_err(|_| Error("repository_list_io_error"))?;
                        let mut link =
                            Url::parse("vcc://vpm/addRepo").expect("constant export URL");
                        let mut query = link.query_pairs_mut();
                        query.append_pair("url", url);
                        for (name, value) in credentials.headers {
                            query.append_pair("headers[]", &format!("{name}:{value}"));
                        }
                        drop(query);
                        bytes.extend_from_slice(link.as_str().as_bytes());
                    } else {
                        bytes.extend_from_slice(url.as_bytes());
                    }
                    bytes.push(b'\n');
                    exportable_count += 1;
                    None
                }
                Err((_, "authentication_not_supported")) => Some("authentication_not_supported"),
                _ => Some("unsupported_source"),
            },
        };
        if let Some(reason) = reason {
            excluded.push(RepositoryListExclusion {
                repository_id: record.repository_id.clone(),
                reason,
            });
        }
        if bytes.len() > REPOSITORY_LIST_MAX_BYTES {
            return Err(Error("repository_list_invalid"));
        }
    }
    let binding = serde_json::to_vec(&serde_json::json!({"format":"alcomd-repository-list-v1", "scope":scope,
        "exclusions":excluded.iter().map(|entry| (&entry.repository_id, entry.reason)).collect::<Vec<_>>() }))
        .map_err(|_| Error("repository_list_invalid"))?;
    let mut fingerprint = Sha256::new();
    fingerprint.update((bytes.len() as u64).to_le_bytes());
    fingerprint.update(binding);
    Ok(PreparedRepositoryList {
        preview: RepositoryListPreview {
            exportable_count,
            excluded,
            byte_count: bytes.len() as u32,
            fingerprint: hex(&fingerprint.finalize()),
        },
        sha256: hex(&Sha256::digest(&bytes)),
        bytes,
    })
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn under(path: &Path, root: &Path) -> bool {
    #[cfg(windows)]
    {
        path.to_string_lossy().to_lowercase().starts_with(&format!(
            "{}\\",
            root.to_string_lossy().trim_end_matches('\\').to_lowercase()
        )) || path
            .to_string_lossy()
            .eq_ignore_ascii_case(&root.to_string_lossy())
    }
    #[cfg(not(windows))]
    {
        path.starts_with(root)
    }
}

fn create_new_list(
    target: &Path,
    bytes: &[u8],
    data_root: &Path,
    project_roots: &[String],
) -> Result<(), Error> {
    create_new_list_with(target, bytes, data_root, project_roots, |file| {
        file.write_all(bytes)
            .and_then(|()| file.flush())
            .and_then(|()| file.sync_all())
    })
}

fn create_new_list_with(
    target: &Path,
    bytes: &[u8],
    data_root: &Path,
    project_roots: &[String],
    write: impl FnOnce(&mut File) -> std::io::Result<()>,
) -> Result<(), Error> {
    if !target.is_absolute()
        || target.as_os_str().len() > 32768
        || bytes.len() > REPOSITORY_LIST_MAX_BYTES
    {
        return Err(Error("repository_list_target_invalid"));
    }
    let name = target
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or(Error("repository_list_target_invalid"))?;
    #[cfg(windows)]
    if name.contains([':', '*', '?', '"', '<', '>', '|']) || name.ends_with(['.', ' ']) {
        return Err(Error("repository_list_target_invalid"));
    }
    let parent = target
        .parent()
        .ok_or(Error("repository_list_target_invalid"))?
        .canonicalize()
        .map_err(|_| Error("repository_list_target_invalid"))?;
    if !parent.is_dir() {
        return Err(Error("repository_list_target_invalid"));
    }
    let target = parent.join(name);
    let data = data_root
        .canonicalize()
        .map_err(|_| Error("repository_list_target_invalid"))?;
    if under(&target, &data)
        || project_roots.iter().any(|root| {
            let path = Path::new(root);
            under(
                &target,
                &path.canonicalize().unwrap_or_else(|_| path.to_path_buf()),
            )
        })
    {
        return Err(Error("repository_list_target_invalid"));
    }
    if fs::symlink_metadata(&target).is_ok() {
        return Err(Error("repository_list_target_exists"));
    }
    let parent_identity =
        file_identity_key(&parent).map_err(|_| Error("repository_list_io_error"))?;
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| Error("repository_list_io_error"))?
        .as_nanos();
    let temporary = parent.join(format!(
        ".alcomd-list-{}-{nonce}-{}.partial",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    // A complete private staging file is published by a no-clobber hard link. No final partial file.
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&temporary)
        .map_err(|_| Error("repository_list_io_error"))?;
    let identity = file_identity_key(&temporary).map_err(|_| Error("repository_list_io_error"))?;
    let result = (|| {
        write(&mut file).map_err(|_| Error("repository_list_io_error"))?;
        if file_identity_key(&parent).ok().as_ref() != Some(&parent_identity)
            || file_identity_key(&temporary).ok().as_ref() != Some(&identity)
        {
            return Err(Error("repository_list_io_error"));
        }
        fs::hard_link(&temporary, &target).map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                Error("repository_list_target_exists")
            } else {
                Error("repository_list_io_error")
            }
        })
    })();
    drop(file);
    // Never clean a path whose current object is not the one this request created.
    if file_identity_key(&parent).ok().as_ref() == Some(&parent_identity)
        && file_identity_key(&temporary).ok().as_ref() == Some(&identity)
    {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn remote(id: &str, url: &str) -> RepositoryListRecord {
        RepositoryListRecord {
            repository_id: id.into(),
            revision: 1,
            source: RepositorySource::Remote { url: url.into() },
            credential_reference: None,
        }
    }

    fn prepare_public(snapshot: &RepositoryListSnapshot) -> PreparedRepositoryList {
        prepare_list(&snapshot.repositories, |_| {
            panic!("anonymous export must not load credentials")
        })
        .expect("repository list fixture")
    }

    #[test]
    fn query_round_trip_comments_duplicates_and_unsupported_headers() {
        let text = "\u{feff}# list\r\nhttps://example.test/index.json?z=2&a=1&a=3 # inline\r\nvcc://vpm/addRepo?url=https%3A%2F%2Fother.test%2Findex.json%3Fb%3D2%26a%3D1\nhttps://example.test/index.json?z=2&a=1&a=3\nvcc://vpm/addRepo?url=https%3A%2F%2Fsecret.test&headers%5BX-Token%5D=secret\nfile:///private\ninvalid\n";
        let parsed = parse_list(text.as_bytes(), &RepositoryListSnapshot::default())
            .expect("repository list fixture");
        assert_eq!(
            parsed
                .entries
                .iter()
                .map(|entry| entry.status)
                .collect::<Vec<_>>(),
            [
                Status::Candidate,
                Status::Candidate,
                Status::Duplicate,
                Status::Candidate,
                Status::Unsupported,
                Status::Invalid
            ]
        );
        assert!(!parsed.entries[3].credentials_configured);
        let snapshot = RepositoryListSnapshot {
            repositories: vec![remote("a", "https://example.test/index.json?z=2&a=1&a=3")],
            project_roots: vec![],
        };
        let prepared = prepare_public(&snapshot);
        assert_eq!(
            prepared.bytes,
            b"https://example.test/index.json?z=2&a=1&a=3\n"
        );
        assert_eq!(
            parse_list(&prepared.bytes, &snapshot)
                .expect("repository list fixture")
                .entries[0]
                .status,
            Status::Duplicate
        );
    }

    #[test]
    fn authenticated_export_reimports_exact_headers_and_binds_the_credential_revision() {
        let mut record = remote("a", "https://example.test/index.json?z=2&a=1");
        record.credential_reference = Some("current-reference".into());
        let credentials = RepositoryCredentials {
            headers: std::collections::BTreeMap::from([
                (
                    "Authorization".into(),
                    "Bearer isolated-export-fixture".into(),
                ),
                ("X-Token".into(), "#%& value".repeat(200)),
            ]),
        };
        let export = |record| {
            prepare_list(&[record], |_| Ok(credentials.clone())).expect("authenticated export")
        };
        let prepared = export(record.clone());
        assert!(prepared.bytes.len() > 2048);
        let parsed = parse_list(&prepared.bytes, &RepositoryListSnapshot::default())
            .expect("export can be reimported");
        assert_eq!(parsed.entries[0].source, Some(record.source.clone()));
        assert_eq!(parsed.entries[0].credentials, Some(credentials.clone()));
        assert!(!format!("{:?}", prepared.preview).contains("isolated-export-fixture"));
        record.credential_reference = Some("rotated-reference".into());
        assert_ne!(
            prepared.preview.fingerprint,
            export(record).preview.fingerprint
        );
    }

    #[test]
    fn full_snapshot_and_scope_fingerprint_are_not_a_gui_page() {
        let mut snapshot = RepositoryListSnapshot {
            repositories: (0..230)
                .map(|index| {
                    remote(
                        &format!("{index:04}"),
                        &format!("https://example.test/{index}?b=2&a=1"),
                    )
                })
                .collect(),
            project_roots: vec![],
        };
        snapshot.repositories.push(RepositoryListRecord {
            repository_id: "local".into(),
            revision: 1,
            source: RepositorySource::Local {
                path: "/local".into(),
            },
            credential_reference: None,
        });
        snapshot.repositories.extend([
            remote(
                VRCHAT_OFFICIAL_REPOSITORY_ID,
                "https://official.test/index.json",
            ),
            remote(
                VRCHAT_CURATED_REPOSITORY_ID,
                "https://curated.test/index.json",
            ),
        ]);
        let prepared = prepare_public(&snapshot);
        assert_eq!(prepared.preview.exportable_count, 230);
        assert_eq!(prepared.preview.excluded[0].reason, "local_source");
        assert_eq!(
            prepared.bytes.iter().filter(|byte| **byte == b'\n').count(),
            230
        );
        snapshot
            .repositories
            .iter_mut()
            .find(|record| record.repository_id == "local")
            .expect("repository list fixture")
            .revision += 1;
        assert_ne!(
            prepared.preview.fingerprint,
            prepare_public(&snapshot).preview.fingerprint
        );
    }

    #[test]
    fn oversized_or_non_utf8_input_is_rejected_without_truncation() {
        assert!(parse_list(&[0xff], &RepositoryListSnapshot::default()).is_err());
        assert!(
            parse_list(
                &vec![b' '; REPOSITORY_LIST_MAX_BYTES + 1],
                &RepositoryListSnapshot::default()
            )
            .is_err()
        );
        assert!(
            parse_list(
                "https://example.test\n".repeat(4097).as_bytes(),
                &RepositoryListSnapshot::default()
            )
            .is_err()
        );
    }

    #[test]
    fn create_new_preserves_existing_and_protected_targets_and_leaves_no_partial() {
        let root = std::env::temp_dir().join(format!(
            "alcomd-list-test-{}-{}",
            std::process::id(),
            SEQUENCE_TEST.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).expect("repository list fixture");
        let data = root.join("data");
        let project = root.join("project");
        fs::create_dir(&data).expect("repository list fixture");
        fs::create_dir(&project).expect("repository list fixture");
        let target = root.join("repositories.txt");
        create_new_list(&target, b"complete\n", &data, &[]).expect("repository list fixture");
        assert_eq!(
            create_new_list(&target, b"replacement", &data, &[]),
            Err(Error("repository_list_target_exists"))
        );
        assert_eq!(
            fs::read(&target).expect("repository list fixture"),
            b"complete\n"
        );
        assert_eq!(
            create_new_list(&data.join("export.txt"), b"x", &data, &[]),
            Err(Error("repository_list_target_invalid"))
        );
        assert_eq!(
            create_new_list(
                &project.join("export.txt"),
                b"x",
                &data,
                &[project.to_str().expect("repository list fixture").into()]
            ),
            Err(Error("repository_list_target_invalid"))
        );
        assert!(
            create_new_list(&root.join("missing").join("export.txt"), b"x", &data, &[]).is_err()
        );
        let failed = root.join("failed.txt");
        let failure = create_new_list_with(&failed, b"complete", &data, &[], |file| {
            file.write_all(b"half")?;
            Err(std::io::Error::other("injected write failure"))
        });
        assert_eq!(failure, Err(Error("repository_list_io_error")));
        assert!(!failed.exists());
        let raced = root.join("created-by-another-request.txt");
        let collision = create_new_list_with(&raced, b"ours", &data, &[], |file| {
            file.write_all(b"ours")?;
            fs::write(&raced, b"foreign")
        });
        assert_eq!(collision, Err(Error("repository_list_target_exists")));
        assert_eq!(
            fs::read(&raced).expect("repository list fixture"),
            b"foreign"
        );
        assert!(
            !fs::read_dir(&root)
                .expect("repository list fixture")
                .any(|entry| {
                    entry
                        .expect("repository list fixture")
                        .file_name()
                        .to_string_lossy()
                        .ends_with(".partial")
                })
        );
        fs::remove_dir_all(root).expect("repository list fixture");
    }
    static SEQUENCE_TEST: AtomicU64 = AtomicU64::new(0);

    #[derive(Clone)]
    struct SnapshotStore(std::sync::Arc<std::sync::Mutex<RepositoryListSnapshot>>);
    impl alcomd_application::RepositoryListStore for SnapshotStore {
        async fn repository_list_snapshot(
            &self,
            _: alcomd_application::PrincipalId,
        ) -> Result<RepositoryListSnapshot, alcomd_application::M3Error> {
            Ok(self.0.lock().expect("repository list fixture").clone())
        }
    }

    #[tokio::test]
    async fn stale_preview_is_rejected_before_file_creation_and_file_io_requires_owner() {
        use alcomd_application::{
            AccessContext, Permission, PrincipalId, RepositoryListApplication,
        };
        let store = SnapshotStore(std::sync::Arc::new(std::sync::Mutex::new(
            RepositoryListSnapshot {
                repositories: vec![remote("a", "https://example.test/repo?z=1&a=2")],
                project_roots: vec![],
            },
        )));
        let root = std::env::temp_dir().join(format!(
            "alcomd-list-stale-{}-{}",
            std::process::id(),
            SEQUENCE_TEST.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).expect("repository list fixture");
        let data = root.join("data");
        fs::create_dir(&data).expect("repository list fixture");
        let app =
            RepositoryListApplication::new(store.clone(), RepositoryListFileAdapter::new(data));
        let owner = AccessContext::local_owner();
        let preview = app.preview(&owner).await.expect("repository list fixture");
        store
            .0
            .lock()
            .expect("repository list fixture")
            .repositories[0]
            .revision += 1;
        let output = root.join("stale.txt");
        assert_eq!(
            app.export(
                &owner,
                output.to_str().expect("repository list fixture").into(),
                preview.fingerprint
            )
            .await
            .err(),
            Some(Error("repository_list_changed"))
        );
        assert!(!output.exists());
        let external = AccessContext::new(
            PrincipalId::parse("extension:fixture").expect("repository list fixture"),
            [Permission::RepositoriesRead],
        );
        assert_eq!(
            app.inspect(&external, "nonexistent".into()).await.err(),
            Some(Error("permission_denied"))
        );
        assert_eq!(
            app.export(&external, "nonexistent".into(), "0".repeat(64))
                .await
                .err(),
            Some(Error("permission_denied"))
        );
        let denied = AccessContext::new(PrincipalId::local_owner(), []);
        assert_eq!(
            app.preview(&denied).await.err(),
            Some(Error("permission_denied"))
        );
        fs::remove_dir_all(root).expect("repository list fixture");
    }
}
