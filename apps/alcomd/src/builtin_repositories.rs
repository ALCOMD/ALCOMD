use std::io;

use alcomd_application::{
    AccessContext, IdempotencyKey, OperationId, RepositoryId, RepositoryObservation,
    RepositorySource, RepositoryValidators, VRCHAT_CURATED_REPOSITORY_ID,
    VRCHAT_OFFICIAL_REPOSITORY_ID, with_log_caller,
};
use alcomd_platform::BindError;
use alcomd_store::StateStoreHandle;
use tokio::task::JoinSet;

use crate::{M3ReadApplication, current_time_ms};

const DEFAULTS: [(&str, &str, &str, &str); 2] = [
    (
        VRCHAT_OFFICIAL_REPOSITORY_ID,
        "com.vrchat.repos.official",
        "Official",
        "https://vrchat.github.io/packages/index.json",
    ),
    (
        VRCHAT_CURATED_REPOSITORY_ID,
        "com.vrchat.repos.curated",
        "Curated",
        "https://vrchat-community.github.io/vpm-listing-curated/index.json",
    ),
];

pub(super) async fn start(
    store: &StateStoreHandle,
    application: &M3ReadApplication,
) -> Result<JoinSet<()>, BindError> {
    let access = AccessContext::local_owner();
    let mut defaults = Vec::with_capacity(DEFAULTS.len());
    for (id, declared_id, name, url) in DEFAULTS {
        let id = RepositoryId::parse(id)
            .map_err(|_| BindError::Io(io::Error::other("invalid built-in repository ID")))?;
        defaults.push((
            id,
            RepositoryObservation {
                source: RepositorySource::Remote { url: url.into() },
                source_identity_key: url.as_bytes().to_vec(),
                declared_id: Some(declared_id.into()),
                name: Some(name.into()),
                declared_url: None,
                issues: Vec::new(),
                packages: Vec::new(),
                validators: RepositoryValidators::default(),
                refreshed_at_ms: 0,
            },
        ));
    }
    let records = store
        .ensure_builtin_repositories(access.principal().clone(), defaults, current_time_ms()?)
        .await
        .map_err(|error| BindError::Io(io::Error::other(error)))?;
    let mut sync = JoinSet::new();
    for record in records {
        if record.resolver_indexed {
            continue;
        }
        let access = access.clone();
        let application = application.clone();
        let store = store.clone();
        let key = IdempotencyKey::parse(OperationId::new().to_string())
            .map_err(|_| BindError::Io(io::Error::other("invalid initial repository sync key")))?;
        sync.spawn(async move {
            if let Err(error) = with_log_caller(
                store,
                &access,
                application.refresh_repository(&access, record.repository_id, record.revision, key),
            )
            .await
            {
                eprintln!(
                    "Initial sync of built-in repository {} failed: {error}",
                    record.repository_id
                );
            }
        });
    }
    Ok(sync)
}
