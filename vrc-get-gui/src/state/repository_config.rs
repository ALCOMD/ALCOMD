use arc_swap::ArcSwap;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{Mutex, MutexGuard};
use vrc_get_vpm::io::{DefaultEnvironmentIo, IoTrait};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryConfig {
    #[serde(default)]
    pub display_names: BTreeMap<String, String>,
}

struct RepositoryConfigStateInner {
    config: RepositoryConfig,
    path: PathBuf,
}

pub struct RepositoryConfigState {
    inner: ArcSwap<RepositoryConfigStateInner>,
    io: DefaultEnvironmentIo,
    mut_lock: Mutex<()>,
}

impl RepositoryConfigState {
    pub async fn new_load(io: &DefaultEnvironmentIo) -> io::Result<Self> {
        let path = io.resolve(crate::storage::REPOSITORY_CONFIG_PATH.as_ref());
        let config = match load_config(&path).await {
            Ok(config) => config,
            Err(error) => {
                log::error!("Failed to load repository configuration, using defaults: {error}");
                if let Err(backup_error) = backup_invalid_config(&path).await {
                    log::error!("Failed to back up repository configuration: {backup_error}");
                }
                RepositoryConfig::default()
            }
        };
        Ok(Self {
            inner: ArcSwap::new(Arc::new(RepositoryConfigStateInner { config, path })),
            io: io.clone(),
            mut_lock: Mutex::new(()),
        })
    }

    pub fn get(&self) -> RepositoryConfigRef {
        RepositoryConfigRef {
            state: self.inner.load_full(),
        }
    }

    pub async fn load_mut(&self) -> RepositoryConfigMutRef<'_> {
        let lock = self.mut_lock.lock().await;
        let loaded = self.inner.load_full();
        RepositoryConfigMutRef {
            config: loaded.config.clone(),
            path: loaded.path.clone(),
            io: &self.io,
            _mut_lock_guard: lock,
            cache: &self.inner,
        }
    }
}

pub struct RepositoryConfigRef {
    state: Arc<RepositoryConfigStateInner>,
}

impl Deref for RepositoryConfigRef {
    type Target = RepositoryConfig;

    fn deref(&self) -> &Self::Target {
        &self.state.config
    }
}

pub struct RepositoryConfigMutRef<'state> {
    config: RepositoryConfig,
    path: PathBuf,
    io: &'state DefaultEnvironmentIo,
    _mut_lock_guard: MutexGuard<'state, ()>,
    cache: &'state ArcSwap<RepositoryConfigStateInner>,
}

impl RepositoryConfigMutRef<'_> {
    pub async fn save(self) -> io::Result<()> {
        let json = serde_json::to_string_pretty(&self.config)?;
        tokio::fs::create_dir_all(self.path.parent().unwrap()).await?;
        self.io.write_atomic(&self.path, json.as_bytes()).await?;
        self.cache.swap(Arc::new(RepositoryConfigStateInner {
            config: self.config,
            path: self.path,
        }));
        Ok(())
    }
}

impl Deref for RepositoryConfigMutRef<'_> {
    type Target = RepositoryConfig;

    fn deref(&self) -> &Self::Target {
        &self.config
    }
}

impl DerefMut for RepositoryConfigMutRef<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.config
    }
}

async fn load_config(path: &Path) -> io::Result<RepositoryConfig> {
    match tokio::fs::read(path).await {
        Ok(buffer) if buffer.is_empty() => Ok(RepositoryConfig::default()),
        Ok(buffer) => serde_json::from_slice(&buffer).map_err(Into::into),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(RepositoryConfig::default()),
        Err(error) => Err(error),
    }
}

async fn backup_invalid_config(path: &Path) -> io::Result<()> {
    let mut index = 0;
    loop {
        let backup_path = path.with_extension(format!("json.bak.{index}"));
        match tokio::fs::rename(path, backup_path).await {
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => index += 1,
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repository_display_names_default_to_empty_and_round_trip() {
        let config: RepositoryConfig = serde_json::from_str("{}").unwrap();
        assert!(config.display_names.is_empty());

        let config: RepositoryConfig = serde_json::from_str(
            r#"{"displayNames":{"https://example.com/index.json":"Example"}}"#,
        )
        .unwrap();
        assert_eq!(
            config
                .display_names
                .get("https://example.com/index.json")
                .map(String::as_str),
            Some("Example")
        );
        assert_eq!(
            serde_json::to_value(config).unwrap()["displayNames"]["https://example.com/index.json"],
            "Example"
        );
    }

    #[test]
    fn invalid_repository_config_is_backed_up_and_replaced_with_defaults() {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(invalid_repository_config_is_backed_up_and_replaced_with_defaults_inner());
    }

    async fn invalid_repository_config_is_backed_up_and_replaced_with_defaults_inner() {
        let temp = tempfile::tempdir().unwrap();
        let io = DefaultEnvironmentIo::new(temp.path().into());
        let path = io.resolve(crate::storage::REPOSITORY_CONFIG_PATH.as_ref());
        tokio::fs::create_dir_all(path.parent().unwrap())
            .await
            .unwrap();
        tokio::fs::write(&path, b"not json").await.unwrap();

        let state = RepositoryConfigState::new_load(&io).await.unwrap();

        assert!(state.get().display_names.is_empty());
        assert!(!path.exists());
        assert!(path.with_extension("json.bak.0").exists());
    }
}
