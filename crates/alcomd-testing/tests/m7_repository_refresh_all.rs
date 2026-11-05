//! Refresh scheduling exercises the application against an isolated Store.
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use alcomd_application::{
    AccessContext, IdempotencyKey, M3Application, M3Error, M3ErrorCode, M3ReadAdapter,
    M3RegistryStore, ProjectDiscoveryMode, ProjectObservation, RepositoryId, RepositoryListStore,
    RepositoryObservation, RepositoryReadOutcome, RepositorySource, RepositoryValidators, Revision,
};
use alcomd_store::StateStoreHandle;
use tokio::sync::{Semaphore, mpsc};

#[derive(Clone)]
struct ControlledReader {
    gates: Arc<HashMap<String, Semaphore>>,
    entered: mpsc::UnboundedSender<String>,
    failed: String,
}

impl M3ReadAdapter for ControlledReader {
    fn normalize_remote_repository_url(&self, url: &str) -> Result<String, M3Error> {
        Ok(url.to_owned())
    }

    async fn inspect_project(
        &self,
        _: String,
        _: ProjectDiscoveryMode,
    ) -> Result<ProjectObservation, M3Error> {
        panic!("refresh must not inspect projects")
    }

    async fn inspect_repository(
        &self,
        source: RepositorySource,
        _: Option<RepositoryValidators>,
    ) -> Result<RepositoryReadOutcome, M3Error> {
        let RepositorySource::Remote { url } = source else {
            panic!("remote fixture source expected")
        };
        self.entered.send(url.clone()).expect("record entry");
        self.gates[&url]
            .acquire()
            .await
            .expect("test gate")
            .forget();
        if url == self.failed {
            return Err(M3Error::new(M3ErrorCode::RepositoryUnavailable));
        }
        Ok(RepositoryReadOutcome::Fresh(observation(&url, "refreshed")))
    }
}

fn observation(url: &str, name: &str) -> RepositoryObservation {
    RepositoryObservation {
        source: RepositorySource::Remote { url: url.into() },
        source_identity_key: url.as_bytes().to_vec(),
        declared_id: None,
        name: Some(name.into()),
        declared_url: None,
        issues: Vec::new(),
        packages: Vec::new(),
        validators: RepositoryValidators::default(),
        refreshed_at_ms: 1,
    }
}

fn key(value: impl Into<String>) -> IdempotencyKey {
    IdempotencyKey::parse(value).expect("test key")
}

struct Directory(PathBuf);
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[tokio::test]
async fn refresh_all_refills_the_window_and_retries_one_revision_conflict() {
    let directory = Directory(
        std::env::temp_dir().join(format!("alcomd-refresh-all-{}", uuid::Uuid::new_v4())),
    );
    std::fs::create_dir(&directory.0).expect("test directory");
    let store = StateStoreHandle::open(directory.0.join("state.db")).expect("isolated Store");
    let access = AccessContext::local_owner();
    let owner = access.principal().clone();
    for index in 0..65 {
        store
            .register_repository(
                owner.clone(),
                observation(&format!("https://test.invalid/{index}"), "initial"),
                key(format!("register-{index}")),
                1,
            )
            .await
            .expect("register fixture");
    }
    let ordered = store
        .repository_list_snapshot(owner.clone())
        .await
        .expect("initial snapshot")
        .repositories
        .into_iter()
        .map(|record| {
            let RepositorySource::Remote { url } = record.source else {
                panic!("remote fixture source expected")
            };
            (
                RepositoryId::parse(&record.repository_id).expect("test ID"),
                url,
            )
        })
        .collect::<Vec<_>>();
    let gates = Arc::new(
        ordered
            .iter()
            .map(|(_, url)| (url.clone(), Semaphore::new(0)))
            .collect::<HashMap<_, _>>(),
    );
    let (entered, mut started) = mpsc::unbounded_channel();
    let reader = ControlledReader {
        gates: gates.clone(),
        entered,
        failed: ordered[3].1.clone(),
    };
    let application = M3Application::new(store.clone(), reader);
    let refresh = tokio::spawn(async move {
        application
            .refresh_all_repositories(&access, key("all"))
            .await
    });
    let mut first = HashSet::new();
    for _ in 0..64 {
        first.insert(next_started(&mut started).await);
    }
    assert_eq!(
        first,
        ordered[..64].iter().map(|(_, url)| url.clone()).collect()
    );
    assert!(started.try_recv().is_err());
    gates[&ordered[1].1].add_permits(1);
    assert_eq!(next_started(&mut started).await, ordered[64].1);

    let (changed_id, changed_url) = &ordered[2];
    store
        .refresh_repository(
            owner.clone(),
            *changed_id,
            Revision::INITIAL,
            observation(changed_url, "changed concurrently"),
            key("independent-refresh"),
            2,
        )
        .await
        .expect("force revision conflict on the first attempt");
    gates[changed_url].add_permits(1);
    assert_eq!(&next_started(&mut started).await, changed_url);
    for gate in gates.values() {
        gate.add_permits(1);
    }
    let outcomes = tokio::time::timeout(Duration::from_secs(5), refresh)
        .await
        .expect("refresh complete")
        .expect("refresh task")
        .expect("refresh results");
    let expected = ordered
        .iter()
        .enumerate()
        .map(|(index, (id, _))| {
            (
                *id,
                if index == 3 {
                    Err(M3ErrorCode::RepositoryUnavailable)
                } else {
                    Ok(())
                },
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(outcomes, expected);
    let changed = store
        .get_repository(owner, *changed_id)
        .await
        .expect("retried repository");
    assert_eq!(changed.revision.get(), 3);
    assert_eq!(changed.observation.name.as_deref(), Some("refreshed"));
}

async fn next_started(started: &mut mpsc::UnboundedReceiver<String>) -> String {
    tokio::time::timeout(Duration::from_secs(5), started.recv())
        .await
        .expect("repository started")
        .expect("recorded entry")
}
