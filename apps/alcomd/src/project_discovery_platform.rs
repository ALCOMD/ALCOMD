use std::path::{Path, PathBuf};

use alcomd_application as app;
use alcomd_platform as platform;

#[derive(Clone, Copy)]
pub(super) struct PlatformProjectDiscovery;

impl app::ProjectDiscoveryPlatform for PlatformProjectDiscovery {
    fn creation_resource(
        &self,
        root: &app::ProjectDiscoveryRoot,
        candidate: &app::ProjectDiscoveryCandidate,
    ) -> Result<app::ResourceKey, app::ProjectDiscoveryError> {
        let leaf = Path::new(&candidate.path)
            .file_name()
            .and_then(|leaf| leaf.to_str())
            .ok_or_else(|| {
                app::ProjectDiscoveryError::DirectoryUnavailable.with_path(&candidate.path)
            })?;
        Ok(alcomd_vpm::project_creation_resource(
            &root.identity,
            leaf.to_owned(),
        ))
    }

    async fn entries(
        &self,
        root: String,
    ) -> Result<app::ProjectDiscoveryEntries, app::ProjectDiscoveryError> {
        let failure_path = root.clone();
        tokio::task::spawn_blocking(move || {
            let discovered = platform::discover_directories(
                Path::new(&root),
                app::PROJECT_DISCOVERY_ENTRY_LIMIT,
            )
            .map_err(|error| {
                app::ProjectDiscoveryError::DirectoryUnavailable
                    .with_path(&root)
                    .with_source(error)
            })?;
            let path = path_text(discovered.root.path)?;
            let candidates = discovered
                .children
                .into_iter()
                .filter_map(|entry| {
                    Some(app::ProjectDiscoveryCandidate {
                        path: path_text(entry.path).ok()?,
                        identity: entry.identity,
                    })
                })
                .collect();
            Ok(app::ProjectDiscoveryEntries {
                root: app::ProjectDiscoveryRoot {
                    path,
                    identity: discovered.root.identity,
                },
                candidates,
                reached_limit: discovered.reached_limit,
            })
        })
        .await
        .map_err(|source| {
            app::ProjectDiscoveryError::DirectoryUnavailable
                .with_path(failure_path)
                .with_source(source)
        })?
    }

    async fn still_current(
        &self,
        root: app::ProjectDiscoveryRoot,
        candidate: app::ProjectDiscoveryCandidate,
    ) -> Result<bool, app::ProjectDiscoveryError> {
        let failure_path = candidate.path.clone();
        tokio::task::spawn_blocking(move || {
                let candidate_path = candidate.path.clone();
                let root_path = root.path.clone();
                platform::discovered_directory_is_current(
                    &platform::DiscoveredDirectory {
                        path: PathBuf::from(root.path),
                        identity: root.identity,
                    },
                    &platform::DiscoveredDirectory {
                        path: PathBuf::from(candidate.path),
                        identity: candidate.identity,
                    },
                )
                .map_err(|error| {
                    app::ProjectDiscoveryError::DirectoryUnavailable
                        .with_path(&candidate_path)
                        .with_source(error)
                        .with_reason(format!("failed to recheck discovery root {root_path} and candidate {candidate_path}"))
                })
        })
        .await
        .map_err(|source| {
            app::ProjectDiscoveryError::DirectoryUnavailable
                .with_path(failure_path)
                .with_source(source)
        })?
    }
}

fn path_text(path: PathBuf) -> Result<String, app::ProjectDiscoveryError> {
    let failure_path = path.to_string_lossy().into_owned();
    path.into_os_string().into_string().map_err(|_| {
        app::ProjectDiscoveryError::DirectoryUnavailable
            .with_path(failure_path)
            .with_source(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "project discovery path is not valid UTF-8",
            ))
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::time::Duration;

    use alcomd_application::{
        ConfigNullableUpdate, ConfigPathUpdate, ConfigProjectUpdate, ConfigUpdate, M3ReadAdapter,
        M3RegistryStore, ProjectDiscoveryPlatform, ResourceLockCoordinator, StateStore,
    };
    use alcomd_store::{FileConfigStore, StateStoreHandle};
    use tokio::sync::Notify;

    use super::*;

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            Self::at_time(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("clock")
                    .as_nanos(),
            )
        }

        fn at_time(timestamp: u128) -> Self {
            let path = std::env::temp_dir().join(format!(
                "alcomd-project-discovery-{}-{timestamp}-{}",
                std::process::id(),
                NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).expect("exclusive fixture root");
            fs::create_dir(path.join("Projects")).expect("fixture Projects");
            fs::create_dir(path.join("Backups")).expect("fixture Backups");
            Self(fs::canonicalize(path).expect("canonical fixture"))
        }

        async fn applications(&self) -> (StateStoreHandle, app::M7OfficialApplication) {
            let store = StateStoreHandle::open(self.0.join("state.db")).expect("fixture store");
            let platform = Arc::new(FixturePlatform(self.0.clone()));
            let settings = app::M7OfficialApplication::new(
                Arc::new(FileConfigStore::new(self.0.join("config/settings.toml"))),
                platform.clone(),
                platform.clone(),
                platform,
            );
            settings
                .initialize_settings()
                .await
                .expect("fixture config");
            (store, settings)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            for _ in 0..20 {
                if fs::remove_dir_all(&self.0).is_ok() || !self.0.exists() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            panic!("owned project discovery fixture could not be removed");
        }
    }

    #[test]
    fn fixtures_created_in_same_clock_tick_are_isolated() {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let first = Fixture::at_time(timestamp);
        let sentinel = first.0.join("sentinel.txt");
        fs::write(&sentinel, b"first fixture").expect("first sentinel");
        let second = Fixture::at_time(timestamp);
        assert_ne!(first.0, second.0, "each fixture owns a distinct root");
        assert!(second.0.join("Projects").is_dir());
        assert!(second.0.join("Backups").is_dir());
        assert!(!second.0.join("sentinel.txt").exists());
        drop(second);
        assert_eq!(
            fs::read(sentinel).expect("first survives"),
            b"first fixture"
        );
    }

    struct FixturePlatform(PathBuf);

    impl app::StorageDirectoryPlatform for FixturePlatform {
        fn default_directories(&self) -> Result<app::StorageDirectories, app::OfficialGuiError> {
            Ok(app::StorageDirectories {
                project_directory: self.0.join("Projects").to_string_lossy().into_owned(),
                backup_directory: self.0.join("Backups").to_string_lossy().into_owned(),
            })
        }

        fn normalize_directory(&self, path: &str) -> Result<String, app::OfficialGuiError> {
            platform::normalize_storage_directory(path)
                .map_err(|_| app::OfficialGuiError::InvalidInput)
        }
    }

    impl app::UnityHubPlatform for FixturePlatform {
        fn normalize_path(&self, _: &str) -> Result<String, app::OfficialGuiError> {
            panic!("project discovery must not validate Hub")
        }

        fn inspect(&self, _: Option<&str>) -> app::UnityHubLocation {
            panic!("project discovery must not inspect Hub")
        }
    }

    impl app::UnityHubLauncher for FixturePlatform {
        fn launch(&self, _: &str) -> Result<(), app::UnityHubOpenError> {
            panic!("project discovery must not launch Hub")
        }
    }

    fn project(path: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        fs::create_dir_all(path.join("ProjectSettings")).expect("ProjectSettings");
        fs::create_dir(path.join("Packages")).expect("Packages");
        let files = [
            (
                "ProjectSettings/ProjectVersion.txt",
                b"m_EditorVersion: 2022.3.22f1\n".as_slice(),
            ),
            (
                "Packages/manifest.json",
                br#"{"dependencies":{}}"#.as_slice(),
            ),
            (
                "Packages/vpm-manifest.json",
                br#"{"dependencies":{},"locked":{}}"#.as_slice(),
            ),
        ];
        files
            .into_iter()
            .map(|(name, bytes)| {
                let path = path.join(name);
                fs::write(&path, bytes).expect("fixture project file");
                (path, bytes.to_vec())
            })
            .collect()
    }

    fn reader() -> alcomd_vpm::VpmReader {
        alcomd_vpm::VpmReader::new().expect("reader")
    }

    fn key(value: &str) -> app::IdempotencyKey {
        app::IdempotencyKey::parse(value).expect("fixture key")
    }

    fn empty_update() -> ConfigUpdate {
        ConfigUpdate {
            appearance: None,
            locale: None,
            packages: None,
            unity: None,
            paths: None,
            integrations: None,
            network_proxy: None,
            window: None,
            projects: None,
            backups: None,
        }
    }

    #[tokio::test]
    async fn missing_directory_failure_preserves_original_path_and_cause_on_retry() {
        use std::error::Error;
        let fixture = Fixture::new();
        let (store, settings) = fixture.applications().await;
        let root = fixture.0.join("Projects");
        fs::remove_dir(&root).expect("remove owned fixture root");
        let root = root.to_string_lossy().into_owned();
        let source = PlatformProjectDiscovery
            .entries(root.clone())
            .await
            .expect_err("missing root");
        assert_eq!(source.path(), Some(root.as_str()));
        assert_eq!(
            source
                .source()
                .expect("original cause")
                .downcast_ref::<std::io::Error>()
                .expect("I/O error")
                .kind(),
            std::io::ErrorKind::NotFound
        );
        let discovery = app::ProjectDiscoveryApplication::new(
            store,
            reader(),
            PlatformProjectDiscovery,
            settings,
        );
        for _ in 0..2 {
            let failure = discovery.scan().await.expect_err("failed scan");
            assert_eq!(failure.path(), Some(root.as_str()));
            assert!(failure.source().is_some());
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn discovery_registers_only_direct_projects_and_duplicate_scans_do_not_write_events() {
        let fixture = Fixture::new();
        let files = project(&fixture.0.join("Projects/Direct"));
        project(&fixture.0.join("Projects/Group/Nested"));
        fs::create_dir_all(fixture.0.join("Projects/Invalid/ProjectSettings"))
            .expect("invalid directory");
        fs::write(
            fixture
                .0
                .join("Projects/Invalid/ProjectSettings/ProjectVersion.txt"),
            b"invalid",
        )
        .expect("invalid project");
        let (store, settings) = fixture.applications().await;
        let discovery = app::ProjectDiscoveryApplication::new(
            store.clone(),
            reader(),
            PlatformProjectDiscovery,
            settings,
        );
        let first = discovery.scan().await.expect("first scan");
        assert_eq!(first.registered, 1);
        assert_eq!(first.rejected, 2);
        let before = store
            .list_projects(app::PrincipalId::local_owner(), None, 100)
            .await
            .expect("projects");
        assert_eq!(before.projects.len(), 1);
        let events_before = store
            .list_events(app::PrincipalId::local_owner(), 0, 100)
            .await
            .expect("events");
        let again = discovery.scan().await.expect("repeat scan");
        assert_eq!(again.registered, 0);
        assert_eq!(again.already_registered, 1);
        assert_eq!(
            store
                .list_projects(app::PrincipalId::local_owner(), None, 100)
                .await
                .expect("projects"),
            before
        );
        assert_eq!(
            store
                .list_events(app::PrincipalId::local_owner(), 0, 100)
                .await
                .expect("events"),
            events_before
        );
        for (path, bytes) in files {
            assert_eq!(fs::read(path).expect("unchanged project"), bytes);
        }
        let registered = &before.projects[0];
        store
            .unregister_project(
                app::PrincipalId::local_owner(),
                registered.project_id,
                registered.revision,
                key("remove-discovered-project"),
                100,
            )
            .await
            .expect("remove from registry");
        assert_eq!(
            discovery
                .scan()
                .await
                .expect("rediscover existing directory")
                .registered,
            1
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn discovery_waits_for_project_publication_and_keeps_its_registered_identity() {
        let fixture = Fixture::new();
        let path = fixture.0.join("Projects/Published");
        project(&path);
        let (store, settings) = fixture.applications().await;
        let entries = PlatformProjectDiscovery
            .entries(fixture.0.join("Projects").to_string_lossy().into_owned())
            .await
            .expect("enumerate fixture");
        let resource = PlatformProjectDiscovery
            .creation_resource(&entries.root, &entries.candidates[0])
            .expect("publication resource");
        let locks = Arc::new(ResourceLockCoordinator::default());
        let publication = locks.acquire(vec![resource]).await;
        let discovery = app::ProjectDiscoveryApplication::with_locks(
            store.clone(),
            reader(),
            PlatformProjectDiscovery,
            settings,
            locks,
        );
        let mut task = tokio::spawn(async move { discovery.scan().await });
        assert!(
            tokio::time::timeout(Duration::from_millis(30), &mut task)
                .await
                .is_err()
        );
        let observation = reader()
            .inspect_project(
                path.to_string_lossy().into_owned(),
                app::ProjectDiscoveryMode::ExactRoot,
            )
            .await
            .expect("published project observation");
        let published = store
            .register_project(
                app::PrincipalId::local_owner(),
                observation,
                key("project-publication"),
                100,
            )
            .await
            .expect("publish registry record while lock is held");
        drop(publication);
        let report = task.await.expect("scan task").expect("scan");
        assert_eq!(report.registered, 0);
        assert_eq!(report.already_registered, 1);
        let projects = store
            .list_projects(app::PrincipalId::local_owner(), None, 100)
            .await
            .expect("registered project");
        assert_eq!(projects.projects, [published.value]);
        let events = store
            .list_events(app::PrincipalId::local_owner(), 0, 100)
            .await
            .expect("committed registration")
            .events;
        assert_eq!(events.len(), 1);
        assert_eq!(
            events
                .iter()
                .filter(|event| event.kind == "project.registered")
                .count(),
            1
        );
    }

    #[derive(Clone)]
    struct HeldReader {
        reader: alcomd_vpm::VpmReader,
        entered: Arc<Notify>,
        release: Arc<Notify>,
        hold: Arc<AtomicBool>,
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn startup_recovery_reservation_defers_discovery_before_any_prepare_io() {
        let fixture = Fixture::new();
        let path = fixture.0.join("Projects/Recovering");
        project(&path);
        let (store, settings) = fixture.applications().await;
        let held = HeldReader {
            reader: reader(),
            entered: Arc::new(Notify::new()),
            release: Arc::new(Notify::new()),
            hold: Arc::new(AtomicBool::new(true)),
        };
        let locks = Arc::new(ResourceLockCoordinator::default());
        let recovery = locks.project_recovery_guard();
        let discovery = app::ProjectDiscoveryApplication::with_locks(
            store.clone(),
            held.clone(),
            PlatformProjectDiscovery,
            settings.clone(),
            locks.clone(),
        );
        let deferred = tokio::time::timeout(Duration::from_secs(2), discovery.scan())
            .await
            .expect("startup scan does not wait for recovery")
            .expect("deferred scan");
        assert!(deferred.startup_recovery_pending);
        assert_eq!(deferred.registered, 0);
        assert!(
            held.hold.load(Ordering::SeqCst),
            "discovery must not enter project preparation while startup recovery is reserved"
        );
        tokio::time::timeout(
            Duration::from_secs(2),
            settings.get_settings(&app::AccessContext::local_owner()),
        )
        .await
        .expect("settings remain responsive")
        .expect("read settings");
        assert!(
            tokio::time::timeout(
                Duration::from_secs(2),
                store.list_projects(app::PrincipalId::local_owner(), None, 100)
            )
            .await
            .expect("project query remains responsive")
            .expect("projects")
            .projects
            .is_empty()
        );
        let observation = reader()
            .inspect_project(
                path.to_string_lossy().into_owned(),
                app::ProjectDiscoveryMode::ExactRoot,
            )
            .await
            .expect("recovered project observation");
        let recovered = store
            .register_project(
                app::PrincipalId::local_owner(),
                observation,
                key("startup-recovered-project"),
                100,
            )
            .await
            .expect("commit recovery-owned identity");
        let changed = locks.project_recovery_notify();
        drop(recovery);
        tokio::time::timeout(Duration::from_secs(1), changed.notified())
            .await
            .expect("recovery release schedules discovery");
        held.release.notify_one();
        let report = tokio::time::timeout(Duration::from_secs(2), discovery.scan())
            .await
            .expect("post-recovery scan")
            .expect("scan");
        assert_eq!(report.registered, 0);
        assert_eq!(report.already_registered, 1);
        let projects = store
            .list_projects(app::PrincipalId::local_owner(), None, 100)
            .await
            .expect("recovered identity retained");
        assert_eq!(projects.projects, [recovered.value]);
        let events = store
            .list_events(app::PrincipalId::local_owner(), 0, 100)
            .await
            .expect("recovery committed registration")
            .events;
        assert_eq!(events.len(), 1);
        assert_eq!(
            events
                .iter()
                .filter(|event| event.kind == "project.registered")
                .count(),
            1
        );
    }

    impl M3ReadAdapter for HeldReader {
        async fn project_directory_state(
            &self,
            path: String,
        ) -> alcomd_application::ProjectDirectoryState {
            alcomd_vpm::read_project_directory_state(path).await
        }

        fn normalize_remote_repository_url(&self, url: &str) -> Result<String, app::M3Error> {
            self.reader.normalize_remote_repository_url(url)
        }

        async fn inspect_project(
            &self,
            path: String,
            mode: app::ProjectDiscoveryMode,
        ) -> Result<app::ProjectObservation, app::M3Error> {
            if self.hold.swap(false, Ordering::SeqCst) {
                self.entered.notify_one();
                self.release.notified().await;
            }
            self.reader.inspect_project(path, mode).await
        }

        async fn inspect_repository(
            &self,
            source: app::RepositorySource,
            validators: Option<app::RepositoryValidators>,
        ) -> Result<app::RepositoryReadOutcome, app::M3Error> {
            self.reader.inspect_repository(source, validators).await
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn configuration_changes_fence_old_scans_without_blocking_settings_or_list() {
        for disable in [true, false] {
            let fixture = Fixture::new();
            project(&fixture.0.join("Projects/Old"));
            project(&fixture.0.join("OtherProjects/New"));
            let (store, settings) = fixture.applications().await;
            let held = HeldReader {
                reader: reader(),
                entered: Arc::new(Notify::new()),
                release: Arc::new(Notify::new()),
                hold: Arc::new(AtomicBool::new(true)),
            };
            let discovery = app::ProjectDiscoveryApplication::new(
                store.clone(),
                held.clone(),
                PlatformProjectDiscovery,
                settings.clone(),
            );
            let task = tokio::spawn(async move { discovery.scan().await });
            tokio::time::timeout(Duration::from_secs(2), held.entered.notified())
                .await
                .expect("scan reached reader");
            let update = if disable {
                ConfigUpdate {
                    projects: Some(ConfigProjectUpdate {
                        auto_discover: false,
                    }),
                    ..empty_update()
                }
            } else {
                ConfigUpdate {
                    paths: Some(ConfigPathUpdate {
                        project_directory: ConfigNullableUpdate::Set(
                            fixture
                                .0
                                .join("OtherProjects")
                                .to_string_lossy()
                                .into_owned(),
                        ),
                        backup_directory: ConfigNullableUpdate::Unchanged,
                    }),
                    ..empty_update()
                }
            };
            tokio::time::timeout(
                Duration::from_secs(2),
                settings.update_settings(&app::AccessContext::local_owner(), 1, update),
            )
            .await
            .expect("settings are not blocked by scan IO")
            .expect("settings update");
            assert!(
                tokio::time::timeout(
                    Duration::from_secs(2),
                    store.list_projects(app::PrincipalId::local_owner(), None, 100)
                )
                .await
                .expect("list is not blocked")
                .expect("list")
                .projects
                .is_empty()
            );
            held.release.notify_one();
            let report = task.await.expect("scan task").expect("scan");
            assert_eq!(report.registered, 0);
            assert!(report.configuration_changed);
            assert!(
                store
                    .list_projects(app::PrincipalId::local_owner(), None, 100)
                    .await
                    .expect("no old registration")
                    .projects
                    .is_empty()
            );
            let current = app::ProjectDiscoveryApplication::new(
                store.clone(),
                reader(),
                PlatformProjectDiscovery,
                settings,
            );
            assert_eq!(
                current.scan().await.expect("next scan").registered,
                u32::from(!disable)
            );
            if !disable {
                let projects = store
                    .list_projects(app::PrincipalId::local_owner(), None, 100)
                    .await
                    .expect("new projects");
                assert_eq!(
                    projects.projects[0].observation.root_path,
                    fixture.0.join("OtherProjects/New").to_string_lossy()
                );
            }
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn unavailable_roots_preserve_the_registry_and_disabled_scans_do_not_touch_them() {
        let fixture = Fixture::new();
        project(&fixture.0.join("Projects/Existing"));
        let (store, settings) = fixture.applications().await;
        let discovery = app::ProjectDiscoveryApplication::new(
            store.clone(),
            reader(),
            PlatformProjectDiscovery,
            settings.clone(),
        );
        assert_eq!(discovery.scan().await.expect("initial scan").registered, 1);
        let before = store
            .list_projects(app::PrincipalId::local_owner(), None, 100)
            .await
            .expect("projects");
        fs::rename(
            fixture.0.join("Projects"),
            fixture.0.join("DetachedProjects"),
        )
        .expect("detach owned fixture root");
        assert_eq!(
            discovery.scan().await,
            Err(app::ProjectDiscoveryError::DirectoryUnavailable)
        );
        settings
            .update_settings(
                &app::AccessContext::local_owner(),
                1,
                ConfigUpdate {
                    projects: Some(ConfigProjectUpdate {
                        auto_discover: false,
                    }),
                    ..empty_update()
                },
            )
            .await
            .expect("disable discovery");
        assert_eq!(
            discovery.scan().await.expect("disabled scan"),
            app::ProjectDiscoveryReport::default()
        );
        assert_eq!(
            store
                .list_projects(app::PrincipalId::local_owner(), None, 100)
                .await
                .expect("retained registry"),
            before
        );
    }
}
