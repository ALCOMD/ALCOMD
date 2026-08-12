use indexmap::IndexMap;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use url::Url;
use vrc_get_vpm::environment::DownloadedRemoteRepository;

const DOWNLOAD_EXPIRATION: Duration = Duration::from_secs(30 * 60);

#[derive(Clone)]
pub struct PendingRepositoryDownload {
    pub url: Url,
    pub headers: IndexMap<Box<str>, Box<str>>,
    pub repository: DownloadedRemoteRepository,
}

struct PendingRepositoryDownloadEntry {
    download: PendingRepositoryDownload,
    created_at: Instant,
}

pub struct RepositoryDownloadsState {
    downloads: Mutex<HashMap<String, PendingRepositoryDownloadEntry>>,
}

impl RepositoryDownloadsState {
    pub fn new() -> Self {
        Self {
            downloads: Mutex::new(HashMap::new()),
        }
    }

    pub fn insert(&self, download: PendingRepositoryDownload) -> String {
        let mut downloads = self.downloads.lock().unwrap();
        downloads.retain(|_, entry| entry.created_at.elapsed() < DOWNLOAD_EXPIRATION);
        let id = uuid::Uuid::new_v4().to_string();
        downloads.insert(
            id.clone(),
            PendingRepositoryDownloadEntry {
                download,
                created_at: Instant::now(),
            },
        );
        id
    }

    pub fn get(&self, id: &str) -> Option<PendingRepositoryDownload> {
        let mut downloads = self.downloads.lock().unwrap();
        if downloads
            .get(id)
            .is_some_and(|entry| entry.created_at.elapsed() >= DOWNLOAD_EXPIRATION)
        {
            downloads.remove(id);
            return None;
        }
        downloads.get(id).map(|entry| entry.download.clone())
    }

    pub fn remove(&self, id: &str) {
        self.downloads.lock().unwrap().remove(id);
    }

    pub fn remove_many<'a>(&self, ids: impl IntoIterator<Item = &'a str>) {
        let mut downloads = self.downloads.lock().unwrap();
        for id in ids {
            downloads.remove(id);
        }
    }
}
