use std::{
    collections::VecDeque,
    path::{Component, Path},
    sync::Mutex,
};

use tauri::{AppHandle, Emitter, Manager, State, Url};

const PRODUCT_SCHEME: &str = env!("ALCOMD_URI_SCHEME");
const MAX_LINKS: usize = 8;
const MAX_LINK_BYTES: usize = 16 * 1024;
const MAX_REPOSITORY_URL_BYTES: usize = 2048;
const MAX_EXTENSION_URL_BYTES: usize = 8192;

#[derive(PartialEq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum DesktopLink {
    VccRepository { link: String },
    Repository { url: String },
    Extension { url: String },
    RepositoryList { path: String },
    TemplateFile { path: String },
}

impl DesktopLink {
    fn parse(input: String) -> Option<Self> {
        if input.len() > MAX_LINK_BYTES || input.contains('|') {
            return None;
        }
        if let Some(file) = Self::parse_file(&input) {
            return Some(file);
        }
        let outer = Url::parse(&input).ok()?;
        if matches!(outer.scheme(), "vcc" | "alcom") {
            return (outer.host_str() == Some("vpm")
                && outer.path() == "/addRepo"
                && outer.username().is_empty()
                && outer.password().is_none()
                && outer.port().is_none())
            .then_some(Self::VccRepository { link: input });
        }
        if outer.scheme() != PRODUCT_SCHEME
            || outer.fragment().is_some()
            || input.chars().any(char::is_control)
        {
            return None;
        }
        let (_, address) = input.split_once("://")?;
        // Match the original route so URL normalization cannot turn an extra
        // path segment or a different authority into an accepted route.
        let (route, _) = address.split_once('?')?;
        let mut fields = outer.query_pairs();
        let (key, url) = fields.next()?;
        if key != "url" || fields.next().is_some() || url.len() > MAX_EXTENSION_URL_BYTES {
            return None;
        }
        if url
            .chars()
            .any(|part| part.is_whitespace() || part.is_control())
            || url.contains('\\')
            || !url
                .get(..8)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case("https://"))
        {
            return None;
        }
        let source = Url::parse(&url).ok()?;
        if source.host_str().is_none()
            || !source.username().is_empty()
            || source.password().is_some()
            || source.fragment().is_some()
        {
            return None;
        }
        // Both forms accept the original address. Keep the source query in
        // its original order and leave business validation to Core.
        match route {
            "repositories/add" if url.len() <= MAX_REPOSITORY_URL_BYTES => Some(Self::Repository {
                url: url.into_owned(),
            }),
            "extensions/add" => Some(Self::Extension {
                url: url.into_owned(),
            }),
            _ => None,
        }
    }

    fn parse_file(input: &str) -> Option<Self> {
        let from_url;
        let path = if input.starts_with("file:") {
            let url = Url::parse(input).ok()?;
            if !matches!(url.host_str(), None | Some("localhost"))
                || url.query().is_some()
                || url.fragment().is_some()
            {
                return None;
            }
            from_url = url.to_file_path().ok()?;
            &from_url
        } else {
            Path::new(input)
        };
        if !path.is_absolute()
            || path.components().any(|part| part == Component::ParentDir)
            || path.to_str()?.chars().any(char::is_control)
        {
            return None;
        }
        #[cfg(windows)]
        if !matches!(
            path.components().next(),
            Some(Component::Prefix(prefix))
                if matches!(prefix.kind(), std::path::Prefix::Disk(_) | std::path::Prefix::VerbatimDisk(_))
        ) {
            return None;
        }
        if path.extension()?.eq_ignore_ascii_case("alcomrepositories") {
            return Some(Self::RepositoryList {
                path: path.to_str()?.to_owned(),
            });
        }
        if path.extension()?.eq_ignore_ascii_case("alcomtemplate") {
            return Some(Self::TemplateFile {
                path: path.to_str()?.to_owned(),
            });
        }
        None
    }
}

#[derive(Default)]
pub(crate) struct DesktopLinks(Mutex<VecDeque<DesktopLink>>);

impl DesktopLinks {
    pub(crate) fn from_arguments() -> Self {
        let inbox = Self::default();
        for argument in std::env::args_os()
            .skip(1)
            .filter_map(|value| value.into_string().ok())
        {
            inbox.offer(argument);
        }
        inbox
    }

    fn offer(&self, value: String) -> bool {
        let Some(link) = DesktopLink::parse(value) else {
            return false;
        };
        let Ok(mut links) = self.0.lock() else {
            return false;
        };
        if links.len() == MAX_LINKS || links.contains(&link) {
            return false;
        }
        links.push_back(link);
        true
    }

    pub(crate) fn take(&self) -> Option<DesktopLink> {
        self.0.lock().ok()?.pop_front()
    }
}

pub(crate) fn forward_arguments(app: &AppHandle, mut arguments: Vec<String>) {
    if arguments.len() == 1 {
        let _ = focus_main(app);
        return;
    }
    // The single-instance adapter may split on a literal pipe. Never accept
    // partial activations from an invocation that carries multiple values.
    if arguments.len() == 2
        && let Some(link) = arguments.pop()
    {
        forward(app, [link]);
    }
}

fn focus_main(app: &AppHandle) -> Option<tauri::WebviewWindow> {
    let window = app.get_webview_window("main")?;
    let _ = window.unminimize();
    let _ = window.set_focus();
    Some(window)
}

pub(crate) fn forward(app: &AppHandle, links: impl IntoIterator<Item = String>) {
    let inbox = app.state::<DesktopLinks>();
    let mut added = false;
    for link in links {
        added |= inbox.offer(link);
    }
    if added && let Some(window) = focus_main(app) {
        let _ = window.emit_to("main", "alcomd-desktop-link-available", ());
    }
}

#[tauri::command]
pub(crate) fn gui_take_desktop_link(links: State<'_, DesktopLinks>) -> Option<DesktopLink> {
    links.take()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_urls_and_absolute_paths_dispatch_only_supported_alcom_formats() {
        let root = std::env::temp_dir();
        let list = root.join("ALCOM 仓库.alcomrepositories");
        let template = root.join("ALCOM template.ALCOMTEMPLATE");
        assert!(
            DesktopLink::parse(Url::from_file_path(&list).expect("file URL").to_string())
                == Some(DesktopLink::RepositoryList {
                    path: list.to_string_lossy().into_owned()
                })
        );
        assert!(
            DesktopLink::parse(template.to_string_lossy().into_owned())
                == Some(DesktopLink::TemplateFile {
                    path: template.to_string_lossy().into_owned()
                })
        );
        assert!(DesktopLink::parse("relative.alcomtemplate".into()).is_none());
        assert!(
            DesktopLink::parse("file://other.example/share/file.alcomtemplate".into()).is_none()
        );
        assert!(DesktopLink::parse("https://example.test/file.alcomrepositories".into()).is_none());
        assert!(
            DesktopLink::parse("alcom://vpm/removeRepo?url=https://example.test".into()).is_none()
        );
    }

    #[test]
    fn product_links_accept_only_add_routes_and_preserve_the_source_address() {
        let source = "https://example.test/index.json?b=2&a=1&a=3";
        let encoded = "https%3A%2F%2Fexample.test%2Findex.json%3Fb%3D2%26a%3D1%26a%3D3";
        for (route, expected) in [
            (
                "repositories/add",
                DesktopLink::Repository { url: source.into() },
            ),
            (
                "extensions/add",
                DesktopLink::Extension { url: source.into() },
            ),
        ] {
            assert!(
                DesktopLink::parse(format!("{PRODUCT_SCHEME}://{route}?url={encoded}"))
                    == Some(expected)
            );
        }

        for address in [
            "projects/add?url=https://example.test",
            "repositories/not-add/../add?url=https://example.test",
            "repositories/add/?url=https://example.test",
            "user@repositories/add?url=https://example.test",
            "repositories:123/add?url=https://example.test",
            "extensions/add",
            "extensions/add?url=",
            "extensions/add?url=https://example.test&url=https://another.test",
            "extensions/add?url=https://example.test&version=1",
            "extensions/add?url=https://example.test&headers%5B%5D=Authorization%3Afixture",
            "extensions/add?url=http://example.test",
            "extensions/add?url=file:///private",
            "extensions/add?url=https%3A%2F%2Fuser%3Apass%40example.test",
            "extensions/add?url=https%3A%2F%2Fexample.test%2Findex.json%23private",
            "extensions/add?url=https://example.test#outer",
            "extensions/add?url=https%3A%2F%2Fexample.test%2F%0A",
        ] {
            assert!(
                DesktopLink::parse(format!("{PRODUCT_SCHEME}://{address}")).is_none(),
                "{address}"
            );
        }
    }

    #[test]
    fn inbox_preserves_legacy_activations_and_drains_once_in_order() {
        let links = DesktopLinks::default();
        assert!(!links.offer("vcc://other/addRepo?url=https://example.test".into()));
        assert!(!links.offer("vcc://vpm/removeRepo?url=https://example.test".into()));
        assert!(!links.offer("vcc://vpm/addRepo?url=https://example.test|extra".into()));
        let vcc =
            "vcc://vpm/addRepo?url=http://legacy.test&headers%5B%5D=X-Token%3Afixture".to_owned();
        assert!(links.offer(vcc.clone()));

        let urls = (1..MAX_LINKS)
            .map(|id| format!("https://example.test/{id}"))
            .collect::<Vec<_>>();
        for url in &urls {
            let link = format!("{PRODUCT_SCHEME}://repositories/add?url={url}");
            assert!(links.offer(link.clone()));
            assert!(!links.offer(link));
        }
        assert!(!links.offer("vcc://vpm/addRepo?url=https://example.test/overflow".into()));
        assert!(links.take() == Some(DesktopLink::VccRepository { link: vcc }));
        for url in urls {
            assert!(links.take() == Some(DesktopLink::Repository { url }));
        }
        assert!(links.take().is_none());
    }
}
