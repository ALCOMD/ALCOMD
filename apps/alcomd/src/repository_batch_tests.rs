use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use alcomd_application::{M4Store, RepositoryBatchItem, RepositorySource, ResolverCatalogSource};
use serde_json::json;
use tokio::net::TcpListener;
use tokio::sync::{Notify, watch};

use super::*;

const DOWNLOAD_LIMIT: usize = 64;

#[tokio::test]
async fn repository_list_file_requires_matching_preview_and_preserves_line_numbers() {
    use alcomd_application::{RepositoryListEntryStatus as Status, RepositoryListError};

    let root = std::env::temp_dir()
        .canonicalize()
        .expect("temporary directory")
        .join(format!("alcomd-repo-list-file-{}", OperationId::new()));
    std::fs::create_dir(&root).expect("isolated directory");
    let store = StateStoreHandle::open(root.join("file-import.db")).expect("fixture store");
    let files = RepositoryListApplication::new(
        store.clone(),
        alcomd_vpm::RepositoryListFileAdapter::new(root.clone()),
    );
    let registry = M3ReadApplication::new(store, alcomd_vpm::VpmReader::new().expect("reader"));
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("fixture listener");
    let address = listener.local_addr().expect("fixture address");
    let counters = Arc::new(Counters::default());
    let server = tokio::spawn(serve(listener, counters.clone()));
    let access = AccessContext::local_owner();
    let file = root.join("repositories.txt");
    let path = file.to_str().expect("file path").to_owned();
    let contents = format!(
        "# reviewed\r\n\r\nhttp://{address}/1 # comment\r\nhttp://{address}/1\r\nnot a URL\r\nhttp://{address}/2\r\n"
    );
    std::fs::write(&file, &contents).expect("write list");
    let reviewed = files
        .inspect(&access, path.clone())
        .await
        .expect("inspect list");
    assert_eq!(reviewed.ignored_lines, 2);
    assert_eq!(
        reviewed
            .entries
            .iter()
            .map(|entry| (entry.line, entry.status))
            .collect::<Vec<_>>(),
        vec![
            (3, Status::Candidate),
            (4, Status::Duplicate),
            (5, Status::Invalid),
            (6, Status::Candidate)
        ]
    );

    // Changing only a comment invalidates the exact reviewed file.
    std::fs::write(&file, contents.replace("# reviewed", "# changed")).expect("edit list comment");
    assert_eq!(
        files
            .import(
                &access,
                &registry,
                path.clone(),
                reviewed.fingerprint.clone(),
                key("changed")
            )
            .await,
        Err(RepositoryListError("repository_list_changed")),
    );
    assert_eq!(counters.requests.load(Ordering::SeqCst), 0);
    assert!(
        registry
            .list_repositories(&access, None, 10)
            .await
            .expect("unchanged registry")
            .repositories
            .is_empty()
    );

    let reviewed_again = files
        .inspect(&access, path.clone())
        .await
        .expect("inspect changed list");
    assert_ne!(reviewed_again.fingerprint, reviewed.fingerprint);
    let stopped = key("stop-before-import");
    assert_eq!(files.cancel_registration(&access, &stopped), Ok(true));
    assert_eq!(
        files
            .import(
                &access,
                &registry,
                path.clone(),
                reviewed_again.fingerprint.clone(),
                stopped
            )
            .await,
        Ok(vec![
            (3, RepositoryBatchItem::Unprocessed),
            (6, RepositoryBatchItem::Unprocessed)
        ]),
    );
    assert_eq!(counters.requests.load(Ordering::SeqCst), 0);
    assert!(
        registry
            .list_repositories(&access, None, 10)
            .await
            .expect("unchanged registry after early stop")
            .repositories
            .is_empty()
    );

    assert_eq!(
        files
            .import(
                &access,
                &registry,
                path.clone(),
                reviewed_again.fingerprint.clone(),
                key("import")
            )
            .await,
        Ok(vec![
            (3, RepositoryBatchItem::Added),
            (6, RepositoryBatchItem::Added)
        ]),
    );
    assert_eq!(counters.requests.load(Ordering::SeqCst), 2);
    assert_eq!(
        registry
            .list_repositories(&access, None, 10)
            .await
            .expect("imported registry")
            .repositories
            .len(),
        2
    );
    assert_eq!(
        files
            .import(
                &access,
                &registry,
                path,
                reviewed_again.fingerprint,
                key("repeat")
            )
            .await,
        Ok(vec![
            (3, RepositoryBatchItem::Existing),
            (6, RepositoryBatchItem::Existing)
        ]),
    );
    assert_eq!(counters.requests.load(Ordering::SeqCst), 2);
    server.abort();
}

#[tokio::test]
async fn repository_register_reuses_only_its_own_remote_preview() {
    use alcomd_protocol as rpc;

    let root = std::env::temp_dir()
        .canonicalize()
        .expect("temporary directory")
        .join(format!("alcomd-repo-preview-{}", OperationId::new()));
    std::fs::create_dir(&root).expect("isolated directory");
    let store = StateStoreHandle::open(root.join("preview.db")).expect("fixture store");
    let registry =
        M3ReadApplication::new(store.clone(), alcomd_vpm::VpmReader::new().expect("reader"));
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("fixture listener");
    let address = listener.local_addr().expect("fixture address");
    let counters = Arc::new(Counters::default());
    let server = tokio::spawn(serve(listener, counters.clone()));
    let access = AccessContext::local_owner();
    let connection = |id: &str| ConnectionState {
        connection_id: id.to_owned(),
        client_instance_id: None,
        actor: "GUI".to_owned(),
        handshake_complete: true,
        capabilities: HashSet::from([
            rpc::CAPABILITY_REPOSITORIES_READ_V1.to_owned(),
            rpc::CAPABILITY_REPOSITORIES_REGISTRY_V1.to_owned(),
        ]),
    };
    let first = connection("preview");
    let second = connection("other");
    for (index, registration) in [(1, &first), (2, &second)] {
        let url = format!("http://{address}/{index}");
        let preview = m3_rpc::dispatch(
            RequestEnvelope {
                id: format!("preview-{index}"),
                method: rpc::METHOD_REPOSITORIES_INSPECT.to_owned(),
                params: json!({"source":{"kind":"remote","url":format!("{url}#fragment")}}),
            },
            &first,
            &registry,
            &access,
        )
        .await;
        let source = preview.response["result"]["repository"]["source"].clone();
        assert_eq!(source, json!({"kind":"remote","url":url}));
        assert_eq!(counters.requests.load(Ordering::SeqCst), index);
        let source = if index == 1 {
            json!({"kind":"remote","url":format!("{url}#same-source")})
        } else {
            source
        };
        let registered = m3_rpc::dispatch(
            RequestEnvelope {
                id: format!("register-{index}"),
                method: rpc::METHOD_REPOSITORIES_REGISTER.to_owned(),
                params: json!({"source":source,"idempotencyKey":format!("register-{index}")}),
            },
            registration,
            &registry,
            &access,
        )
        .await;
        assert_eq!(
            registered.response["result"]["repository"]["declaredId"],
            format!("example.{index}")
        );
        assert_eq!(
            counters.requests.load(Ordering::SeqCst),
            if index == 1 { 1 } else { 3 }
        );
    }
    let catalogue = store
        .resolver_catalog(access.principal().clone(), false)
        .await
        .expect("indexed catalogue");
    assert!(catalogue.complete);
    assert_eq!(catalogue.entries.len(), 2, "the full preview was indexed");
    server.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn repository_list_parallel_fetch_keeps_order_and_skips_existing() {
    let limit = DOWNLOAD_LIMIT;
    let root = std::env::temp_dir()
        .canonicalize()
        .expect("temporary directory")
        .join(format!("alcomd-list-bench-{}", OperationId::new()));
    std::fs::create_dir(&root).expect("isolated directory");
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("fixture listener");
    let address = listener.local_addr().expect("fixture address");
    let counters = Arc::new(Counters::default());
    let server = tokio::spawn(serve(listener, counters.clone()));
    let access = AccessContext::local_owner();
    let reader = alcomd_vpm::VpmReader::new().expect("repository reader");
    let urls = (0..limit)
        .map(|index| format!("http://{address}/{index}"))
        .collect::<Vec<_>>();
    let mut serial_timings = Vec::new();
    let mut batch_timings = Vec::new();
    for sample in 0..3 {
        let serial_store = StateStoreHandle::open(root.join(format!("serial-{sample}.db")))
            .expect("serial fixture store");
        let serial = M3ReadApplication::new(serial_store, reader.clone());
        counters.reset();
        let started = Instant::now();
        for (index, url) in urls.iter().enumerate() {
            serial
                .register_repository(
                    &access,
                    RepositorySource::Remote { url: url.clone() },
                    key(&format!("serial-{index}")),
                )
                .await
                .expect("serial registration");
        }
        serial_timings.push(started.elapsed());
        assert_eq!(counters.requests.load(Ordering::SeqCst), limit);
        assert_eq!(counters.peak.load(Ordering::SeqCst), 1);

        let store = StateStoreHandle::open(root.join(format!("batch-{sample}.db")))
            .expect("parallel fixture store");
        let batch = M3ReadApplication::new(store.clone(), reader.clone());
        counters.reset();
        let oversized = (0..=alcomd_application::REPOSITORY_LIST_MAX_ENTRIES)
            .map(|index| format!("http://{address}/{index}"))
            .collect();
        assert!(
            batch
                .register_repository_list(&access, oversized, key("oversized"), None)
                .await
                .is_err()
        );
        assert_eq!(counters.requests.load(Ordering::SeqCst), 0);
        let started = Instant::now();
        let result = batch
            .register_repository_list(&access, urls.clone(), key("parallel"), None)
            .await
            .expect("batch registration");
        batch_timings.push(started.elapsed());
        assert_eq!(result, vec![RepositoryBatchItem::Added; limit]);
        assert_eq!(counters.requests.load(Ordering::SeqCst), limit);
        assert!((2..=limit).contains(&counters.peak.load(Ordering::SeqCst)));
        let catalogue = store
            .resolver_catalog(access.principal().clone(), false)
            .await
            .expect("indexed catalogue");
        assert!(catalogue.complete);
        let indexed = catalogue
            .entries
            .into_iter()
            .map(|entry| match entry.source {
                ResolverCatalogSource::Repository {
                    repository_locator, ..
                } => repository_locator,
                ResolverCatalogSource::UserPackage { .. } => unreachable!("fixture repository"),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            indexed, urls,
            "resolver precedence follows input, independent of fetch order"
        );
        let repeated = urls.iter().map(|url| format!("{url}#ignored")).collect();
        let repeated = batch
            .register_repository_list(&access, repeated, key("repeat"), None)
            .await
            .expect("repeat import");
        assert_eq!(repeated, vec![RepositoryBatchItem::Existing; limit]);
        assert_eq!(
            counters.requests.load(Ordering::SeqCst),
            limit,
            "no repeat HTTP calls"
        );
    }
    serial_timings.sort_unstable();
    batch_timings.sort_unstable();
    eprintln!(
        "repository benchmark {limit} endpoints, 3 samples: serial {serial_timings:?}, batch {batch_timings:?}, median speedup {:.2}x, duplicate GETs 0",
        serial_timings[1].as_secs_f64() / batch_timings[1].as_secs_f64()
    );
    server.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn repository_list_refills_while_first_download_waits_and_stops_admission() {
    let limit = DOWNLOAD_LIMIT;
    let root = std::env::temp_dir()
        .canonicalize()
        .expect("temporary directory")
        .join(format!("alcomd-list-rolling-{}", OperationId::new()));
    std::fs::create_dir(&root).expect("isolated directory");
    let store = StateStoreHandle::open(root.join("rolling.db")).expect("fixture store");
    let registry =
        M3ReadApplication::new(store.clone(), alcomd_vpm::VpmReader::new().expect("reader"));
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
    let address = listener.local_addr().expect("fixture address");
    let first = Arc::new(Notify::new());
    let counters = Arc::new(Counters {
        release_first: Some(first.clone()),
        ..Counters::default()
    });
    let server = tokio::spawn(serve(listener, counters.clone()));
    let urls = (0..(2 * limit + 1))
        .map(|index| format!("http://{address}/{index}"))
        .collect::<Vec<_>>();
    let (_already_stopped, stopped_signal) = watch::channel(true);
    let stopped = registry
        .register_repository_list(
            &AccessContext::local_owner(),
            urls.clone(),
            key("already-stopped"),
            Some(stopped_signal),
        )
        .await
        .expect("stop before admission");
    assert_eq!(stopped, vec![RepositoryBatchItem::Unprocessed; urls.len()]);
    assert_eq!(counters.requests.load(Ordering::SeqCst), 0);
    let (stop, signal) = watch::channel(false);
    let import = tokio::spawn({
        let urls = urls.clone();
        async move {
            registry
                .register_repository_list(
                    &AccessContext::local_owner(),
                    urls,
                    key("rolling"),
                    Some(signal),
                )
                .await
                .expect("rolling import results")
        }
    });

    // The first HTTP response is held while slots from later responses admit a
    // second group. Registration remains blocked until the first source finishes.
    tokio::time::timeout(Duration::from_secs(15), async {
        while counters.requests.load(Ordering::SeqCst) < 2 * limit {
            counters.arrived.notified().await;
        }
    })
    .await
    .expect("rolling scheduler refills freed slots");
    assert!(
        store
            .resolver_catalog(AccessContext::local_owner().principal().clone(), false)
            .await
            .expect("catalogue before first response")
            .entries
            .is_empty()
    );
    stop.send(true).expect("stop pending admissions");
    first.notify_one();
    let outcomes = tokio::time::timeout(Duration::from_secs(15), import)
        .await
        .expect("admitted downloads drained")
        .expect("import task completed");
    assert_eq!(
        outcomes[..2 * limit],
        vec![RepositoryBatchItem::Added; 2 * limit]
    );
    assert_eq!(outcomes[2 * limit], RepositoryBatchItem::Unprocessed);
    assert_eq!(counters.requests.load(Ordering::SeqCst), 2 * limit);
    assert!(counters.peak.load(Ordering::SeqCst) <= limit);
    let catalogue = store
        .resolver_catalog(AccessContext::local_owner().principal().clone(), false)
        .await
        .expect("indexed catalogue");
    let indexed = catalogue
        .entries
        .into_iter()
        .map(|entry| match entry.source {
            ResolverCatalogSource::Repository {
                repository_locator, ..
            } => repository_locator,
            ResolverCatalogSource::UserPackage { .. } => unreachable!("fixture repository"),
        })
        .collect::<Vec<_>>();
    assert_eq!(indexed, urls[..2 * limit]);
    server.abort();
}

#[derive(Default)]
struct Counters {
    requests: AtomicUsize,
    active: AtomicUsize,
    peak: AtomicUsize,
    arrived: Notify,
    release_first: Option<Arc<Notify>>,
}

impl Counters {
    fn reset(&self) {
        assert_eq!(self.active.load(Ordering::SeqCst), 0);
        self.requests.store(0, Ordering::SeqCst);
        self.peak.store(0, Ordering::SeqCst);
    }
}

async fn serve(listener: TcpListener, counters: Arc<Counters>) {
    loop {
        let (mut stream, _) = listener.accept().await.expect("fixture connection");
        let counters = counters.clone();
        tokio::spawn(async move {
            let mut request = Vec::new();
            let mut buffer = [0; 1024];
            while !request.ends_with(b"\r\n\r\n") {
                let count = stream.read(&mut buffer).await.expect("read request");
                assert!(count > 0 && request.len() < 8192);
                request.extend_from_slice(&buffer[..count]);
            }
            let request = std::str::from_utf8(&request).expect("HTTP request text");
            let index = request
                .split_whitespace()
                .nth(1)
                .expect("request path")
                .trim_start_matches('/');
            counters.requests.fetch_add(1, Ordering::SeqCst);
            let active = counters.active.fetch_add(1, Ordering::SeqCst) + 1;
            counters.peak.fetch_max(active, Ordering::SeqCst);
            counters.arrived.notify_one();
            if index == "0"
                && let Some(release) = &counters.release_first
            {
                release.notified().await;
            }
            tokio::time::sleep(Duration::from_millis(if index == "0" { 220 } else { 100 })).await;
            let document = json!({
                "id":format!("example.{index}"),
                "packages":{"com.example.test":{"versions":{"1.0.0":{
                    "name":"com.example.test", "displayName":"Example", "version":"1.0.0",
                    "author":{"name":"Example"}, "url":"https://example.invalid/pkg.zip"
                }}}}
            })
            .to_string();
            counters.active.fetch_sub(1, Ordering::SeqCst);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{document}",
                document.len()
            );
            stream
                .write_all(response.as_bytes())
                .await
                .expect("fixture HTTP response");
        });
    }
}

fn key(value: &str) -> IdempotencyKey {
    IdempotencyKey::parse(value).expect("fixture key")
}
