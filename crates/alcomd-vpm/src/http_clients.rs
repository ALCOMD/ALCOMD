use std::sync::Arc;

use alcomd_application::{ConfigProxy, M7OfficialApplication};
use reqwest::{Client, ClientBuilder, Proxy};
use tokio::sync::Mutex;

#[derive(Clone)]
pub(crate) struct ClientPair {
    pub anonymous: Client,
    pub authenticated: Client,
}

#[derive(Clone)]
pub(crate) struct HttpClients {
    settings: Option<M7OfficialApplication>,
    current: Arc<Mutex<(ConfigProxy, ClientPair)>>,
    build: fn(&ConfigProxy) -> Result<ClientPair, reqwest::Error>,
}

impl HttpClients {
    pub fn new(
        settings: Option<M7OfficialApplication>,
        build: fn(&ConfigProxy) -> Result<ClientPair, reqwest::Error>,
    ) -> Result<Self, reqwest::Error> {
        let proxy = ConfigProxy::System;
        let pair = build(&proxy)?;
        Ok(Self {
            settings,
            current: Arc::new(Mutex::new((proxy, pair))),
            build,
        })
    }

    pub fn settings(&self) -> Option<M7OfficialApplication> {
        self.settings.clone()
    }

    pub async fn for_request(&self) -> Result<ClientPair, ()> {
        let mut current = self.current.lock().await;
        if let Some(settings) = &self.settings {
            let proxy = settings.network_proxy().await.map_err(|_| ())?;
            if current.0 != proxy {
                let build = self.build;
                let (proxy, clients) = tokio::task::spawn_blocking(move || {
                    let clients = build(&proxy).map_err(|_| ())?;
                    Ok::<_, ()>((proxy, clients))
                })
                .await
                .map_err(|_| ())??;
                *current = (proxy, clients);
            }
        }
        Ok(current.1.clone())
    }

    #[cfg(all(test, windows))]
    pub async fn set_authenticated(&self, client: Client) {
        self.current.lock().await.1.authenticated = client;
    }
}

pub(crate) fn with_proxy(
    builder: ClientBuilder,
    setting: &ConfigProxy,
) -> Result<ClientBuilder, reqwest::Error> {
    Ok(match setting {
        ConfigProxy::System => builder,
        ConfigProxy::Direct => builder.no_proxy(),
        ConfigProxy::Custom { url } => builder.proxy(Proxy::all(url)?),
    })
}
