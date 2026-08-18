use std::collections::BTreeSet;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

const LICENSE: &str = "AGPL-3.0-only";
const FORBIDDEN_PRODUCTION_TOKENS: &[&str] = &[
    "com.cqmhv.alcomd3",
    "CQMHV.ALCOMD3",
    "alcomd3-mcp",
    "ALCOMD3.exe",
];
const PRODUCTION_ROOTS: &[&str] = &["apps", "crates", "extensions", "packages", "sdk"];
const REQUIRED_PATHS: &[&str] = &[
    "AGENTS.md",
    "LICENSE",
    "Cargo.lock",
    "package-lock.json",
    "alcomd.product.toml",
    "feature-parity.toml",
    "docs/architecture/ALCOMD-V4.md",
    "docs/exec-plans/M0-bootstrap.md",
    "apps/alcomd",
    "apps/alcomd-gui",
    "apps/alcomd-cli",
    "apps/alcomd-mcp",
    "apps/alcomd-updater",
    "extensions/first-party/alcomd-extension-mcp",
    "extensions/first-party/alcomd-extension-discord",
];

type Member = (&'static str, &'static str, &'static [&'static str]);

const MEMBERS: &[Member] = &[
    (
        "apps/alcomd",
        "alcomd",
        &[
            "alcomd-application",
            "alcomd-platform",
            "alcomd-protocol",
            "alcomd-store",
        ],
    ),
    (
        "apps/alcomd-cli",
        "alcomd-cli",
        &["alcomd-client", "alcomd-protocol"],
    ),
    ("apps/alcomd-mcp", "alcomd-mcp", &["alcomd-client"]),
    ("apps/alcomd-api", "alcomd-api", &["alcomd-client"]),
    (
        "apps/alcomd-extension-host",
        "alcomd-extension-host",
        &["alcomd-client", "alcomd-extensions"],
    ),
    (
        "apps/alcomd-bootstrap",
        "alcomd-bootstrap",
        &["alcomd-updater"],
    ),
    ("apps/alcomd-updater", "alcomd-updater", &[]),
    (
        "apps/alcomd-gui/src-tauri",
        "alcomd-gui",
        &["alcomd-client", "alcomd-protocol"],
    ),
    ("crates/alcomd-domain", "alcomd-domain", &[]),
    (
        "crates/alcomd-application",
        "alcomd-application",
        &["alcomd-domain"],
    ),
    ("crates/alcomd-protocol", "alcomd-protocol", &[]),
    (
        "crates/alcomd-client",
        "alcomd-client",
        &["alcomd-platform", "alcomd-protocol"],
    ),
    (
        "crates/alcomd-store",
        "alcomd-store",
        &["alcomd-application"],
    ),
    ("crates/alcomd-platform", "alcomd-platform", &[]),
    ("crates/alcomd-vpm", "alcomd-vpm", &[]),
    ("crates/alcomd-extensions", "alcomd-extensions", &[]),
    ("crates/alcomd-import", "alcomd-import", &[]),
    ("crates/alcomd-testing", "alcomd-testing", &[]),
    (
        "migrations/v3/app/alcomd-migrate-v3",
        "alcomd-migrate-v3",
        &[],
    ),
    ("xtask", "xtask", &[]),
];

#[derive(Deserialize)]
struct ProductConfig {
    product: Product,
    identity: Identity,
    binaries: Binaries,
}

#[derive(Deserialize)]
struct Product {
    family_name: String,
    technical_name: String,
    display_name: String,
    publisher_name: String,
    version: String,
}

#[derive(Deserialize)]
struct Identity {
    bundle_identifier: String,
    windows_aumid: String,
    uri_scheme: String,
    data_directory: String,
}

#[derive(Deserialize)]
struct Binaries {
    daemon: String,
    gui: String,
    cli: String,
    mcp: String,
    api: String,
    extension_host: String,
    bootstrap: String,
    updater: String,
    migration_v3: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let command = env::args().nth(1).unwrap_or_else(|| "check".to_owned());
    let root = workspace_root();

    match command.as_str() {
        "check" => check(&root),
        "tree" => print_tree(&root),
        other => Err(format!("unknown xtask command: {other}").into()),
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask must be directly under the workspace root")
        .to_path_buf()
}

fn check(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut errors = Vec::new();
    for relative in REQUIRED_PATHS {
        if !root.join(relative).exists() {
            errors.push(format!("missing required path: {relative}"));
        }
    }

    let product: ProductConfig = read_toml(&root.join("alcomd.product.toml"))?;
    check_product(&product, &mut errors);
    check_workspace(root, &mut errors)?;
    check_derived_identity(root, &product, &mut errors)?;
    check_unsafe_boundary(root, &mut errors)?;

    for relative in PRODUCTION_ROOTS {
        scan_forbidden_tokens(&root.join(relative), &mut errors)?;
    }

    if errors.is_empty() {
        println!("ALCOMD repository checks passed.");
        return Ok(());
    }
    for error in &errors {
        eprintln!("error: {error}");
    }
    Err(format!("{} repository check(s) failed", errors.len()).into())
}

fn check_unsafe_boundary(
    root: &Path,
    errors: &mut Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    const APPROVED: [&str; 2] = [
        "crates/alcomd-platform/src/windows_security.rs",
        "crates/alcomd-platform/src/windows_known_folder.rs",
    ];
    let unsafe_word = ["un", "safe"].concat();
    let allowance = format!("allow({unsafe_word}_code)");
    for approved in APPROVED {
        let approved_path = root.join(approved);
        let approved_content = fs::read_to_string(&approved_path)?;
        if !approved_content.contains(&format!("#![{allowance}]")) {
            errors.push(format!(
                "{approved} must contain the approved crate-local unsafe allowance"
            ));
        }
    }

    scan_rust_sources(root, root, &APPROVED, errors)?;

    let platform_manifest: toml::Value =
        read_toml(&root.join("crates/alcomd-platform/Cargo.toml"))?;
    if platform_manifest["lints"]["rust"]["unsafe_code"].as_str() != Some("deny") {
        errors.push("alcomd-platform must set unsafe_code = \"deny\"".to_owned());
    }
    if platform_manifest["lints"]["clippy"]["undocumented_unsafe_blocks"].as_str() != Some("deny") {
        errors.push("alcomd-platform must deny undocumented unsafe blocks".to_owned());
    }
    Ok(())
}

fn scan_rust_sources(
    root: &Path,
    path: &Path,
    approved: &[&str],
    errors: &mut Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let child = entry?.path();
            let name = child
                .file_name()
                .and_then(OsStr::to_str)
                .unwrap_or_default();
            if !matches!(name, "target" | "node_modules" | ".git") {
                scan_rust_sources(root, &child, approved, errors)?;
            }
        }
        return Ok(());
    }
    if path.extension().and_then(OsStr::to_str) != Some("rs") {
        return Ok(());
    }

    let relative = path
        .strip_prefix(root)?
        .to_string_lossy()
        .replace('\\', "/");
    let content = fs::read_to_string(path)?;
    let unsafe_word = ["un", "safe"].concat();
    let contains_allow = content.contains(&format!("allow({unsafe_word}_code)"));
    let unsafe_tokens =
        ["{", "fn ", "impl ", "trait "].map(|suffix| format!("{unsafe_word} {suffix}"));
    let contains_boundary_token = unsafe_tokens.iter().any(|token| content.contains(token));
    if !approved.contains(&relative.as_str()) && contains_allow {
        errors.push(format!(
            "unsafe_code allowance outside approved file: {relative}"
        ));
    }
    if !approved.contains(&relative.as_str()) && contains_boundary_token {
        errors.push(format!("unsafe Rust outside approved file: {relative}"));
    }
    Ok(())
}

fn check_product(product: &ProductConfig, errors: &mut Vec<String>) {
    let values = [
        (
            "family_name",
            product.product.family_name.as_str(),
            "ALCOMD",
        ),
        (
            "technical_name",
            product.product.technical_name.as_str(),
            "alcomd",
        ),
        (
            "display_name",
            product.product.display_name.as_str(),
            "ALCOMD3",
        ),
        (
            "publisher_name",
            product.product.publisher_name.as_str(),
            "CQMHV",
        ),
        (
            "bundle_identifier",
            product.identity.bundle_identifier.as_str(),
            "com.cqmhv.alcomd",
        ),
        (
            "windows_aumid",
            product.identity.windows_aumid.as_str(),
            "CQMHV.ALCOMD",
        ),
        ("uri_scheme", product.identity.uri_scheme.as_str(), "alcomd"),
        (
            "data_directory",
            product.identity.data_directory.as_str(),
            "ALCOMD",
        ),
        ("daemon binary", product.binaries.daemon.as_str(), "alcomd"),
        ("GUI binary", product.binaries.gui.as_str(), "alcomd-gui"),
        ("CLI binary", product.binaries.cli.as_str(), "alcomd-cli"),
        ("MCP binary", product.binaries.mcp.as_str(), "alcomd-mcp"),
        ("API binary", product.binaries.api.as_str(), "alcomd-api"),
        (
            "extension host binary",
            product.binaries.extension_host.as_str(),
            "alcomd-extension-host",
        ),
        (
            "bootstrap binary",
            product.binaries.bootstrap.as_str(),
            "alcomd-bootstrap",
        ),
        (
            "updater binary",
            product.binaries.updater.as_str(),
            "alcomd-updater",
        ),
        (
            "migration binary",
            product.binaries.migration_v3.as_str(),
            "alcomd-migrate-v3",
        ),
    ];
    for (label, actual, expected) in values {
        if actual != expected {
            errors.push(format!(
                "product {label} differs: expected {expected:?}, found {actual:?}"
            ));
        }
    }
}

fn check_workspace(
    root: &Path,
    errors: &mut Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let cargo: toml::Value = read_toml(&root.join("Cargo.toml"))?;
    let actual_members = cargo["workspace"]["members"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(toml::Value::as_str)
        .collect::<BTreeSet<_>>();
    let expected_members = MEMBERS
        .iter()
        .map(|(path, _, _)| *path)
        .collect::<BTreeSet<_>>();
    if actual_members != expected_members {
        errors.push("Cargo workspace members do not match the M0 skeleton".to_owned());
    }

    for (path, expected_name, expected_local_dependencies) in MEMBERS {
        let manifest: toml::Value = read_toml(&root.join(path).join("Cargo.toml"))?;
        let actual_name = manifest["package"]["name"].as_str();
        if actual_name != Some(expected_name) {
            errors.push(format!(
                "{path} package name differs: expected {expected_name:?}, found {actual_name:?}"
            ));
        }

        let actual_local_dependencies = manifest
            .get("dependencies")
            .and_then(toml::Value::as_table)
            .map(|dependencies| {
                dependencies
                    .keys()
                    .filter(|name| name.starts_with("alcomd-"))
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>()
            })
            .unwrap_or_default();
        let expected = expected_local_dependencies
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if actual_local_dependencies != expected {
            errors.push(format!(
                "{path} local dependencies differ: expected {expected:?}, found {actual_local_dependencies:?}"
            ));
        }

        if *path == "crates/alcomd-domain" {
            let actual_dependencies = manifest
                .get("dependencies")
                .and_then(toml::Value::as_table)
                .map(|dependencies| {
                    dependencies
                        .keys()
                        .map(String::as_str)
                        .collect::<BTreeSet<_>>()
                })
                .unwrap_or_default();
            let allowed_dependencies = ["serde", "uuid"].into_iter().collect::<BTreeSet<_>>();
            if actual_dependencies != allowed_dependencies {
                errors.push(format!(
                    "{path} dependencies differ: expected {allowed_dependencies:?}, found {actual_dependencies:?}"
                ));
            }
        }
    }
    Ok(())
}

fn check_derived_identity(
    root: &Path,
    product: &ProductConfig,
    errors: &mut Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let tauri: serde_json::Value =
        read_json(&root.join("apps/alcomd-gui/src-tauri/tauri.conf.json"))?;
    let tauri_values = [
        ("/productName", product.product.display_name.as_str()),
        ("/version", product.product.version.as_str()),
        ("/identifier", product.identity.bundle_identifier.as_str()),
        ("/mainBinaryName", product.binaries.gui.as_str()),
        ("/bundle/publisher", product.product.publisher_name.as_str()),
    ];
    for (pointer, expected) in tauri_values {
        if tauri.pointer(pointer).and_then(serde_json::Value::as_str) != Some(expected) {
            errors.push(format!(
                "Tauri {pointer} does not match alcomd.product.toml"
            ));
        }
    }

    let root_package: serde_json::Value = read_json(&root.join("package.json"))?;
    let gui_package: serde_json::Value = read_json(&root.join("apps/alcomd-gui/package.json"))?;
    let npm_values = [
        (
            root_package.get("name").and_then(serde_json::Value::as_str),
            format!("{}-workspace", product.product.technical_name),
            "root npm package name",
        ),
        (
            root_package
                .get("version")
                .and_then(serde_json::Value::as_str),
            product.product.version.clone(),
            "root npm package version",
        ),
        (
            gui_package.get("name").and_then(serde_json::Value::as_str),
            format!("@{}/gui", product.product.technical_name),
            "GUI npm package name",
        ),
        (
            gui_package
                .get("version")
                .and_then(serde_json::Value::as_str),
            product.product.version.clone(),
            "GUI npm package version",
        ),
    ];
    for (actual, expected, label) in npm_values {
        if actual != Some(expected.as_str()) {
            errors.push(format!("{label} does not match alcomd.product.toml"));
        }
    }

    for extension in ["mcp", "discord"] {
        let relative =
            format!("extensions/first-party/alcomd-extension-{extension}/alcomd-extension.toml");
        let manifest: toml::Value = read_toml(&root.join(&relative))?;
        let expected_id = format!(
            "{}.extension.{extension}",
            product.identity.bundle_identifier
        );
        if manifest["id"].as_str() != Some(&expected_id) {
            errors.push(format!("{relative} ID does not match alcomd.product.toml"));
        }
        if manifest["publisher"].as_str() != Some(product.product.publisher_name.as_str()) {
            errors.push(format!(
                "{relative} publisher does not match alcomd.product.toml"
            ));
        }
        if manifest["license"].as_str() != Some(LICENSE) {
            errors.push(format!("{relative} must use {LICENSE}"));
        }
    }

    let identity_tokens = [
        (
            "packages/alcomd-ui/src/index.ts",
            format!("productFamily = \"{}\"", product.product.family_name),
        ),
        (
            "packages/alcomd-ui/src/index.ts",
            format!("technicalName = \"{}\"", product.product.technical_name),
        ),
        (
            "crates/alcomd-protocol/src/lib.rs",
            format!("PRODUCT_FAMILY: &str = \"{}\"", product.product.family_name),
        ),
        (
            "crates/alcomd-protocol/src/lib.rs",
            format!(
                "TECHNICAL_NAME: &str = \"{}\"",
                product.product.technical_name
            ),
        ),
    ];
    for (relative, token) in identity_tokens {
        if !fs::read_to_string(root.join(relative))?.contains(&token) {
            errors.push(format!(
                "{relative} does not contain derived identity {token:?}"
            ));
        }
    }
    Ok(())
}

fn read_toml<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, Box<dyn std::error::Error>> {
    Ok(toml::from_str(&fs::read_to_string(path)?)?)
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, Box<dyn std::error::Error>> {
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

fn scan_forbidden_tokens(
    path: &Path,
    errors: &mut Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let child = entry?.path();
            let name = child
                .file_name()
                .and_then(OsStr::to_str)
                .unwrap_or_default();
            if !matches!(name, "target" | "node_modules" | "dist" | ".git") {
                scan_forbidden_tokens(&child, errors)?;
            }
        }
    } else if is_text_file(path) {
        let content = fs::read_to_string(path)?;
        for token in FORBIDDEN_PRODUCTION_TOKENS {
            if content.contains(token) {
                errors.push(format!(
                    "forbidden legacy production token `{token}` in {}",
                    path.display()
                ));
            }
        }
    }
    Ok(())
}

fn is_text_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(OsStr::to_str),
        Some(
            "rs" | "toml"
                | "json"
                | "ts"
                | "tsx"
                | "js"
                | "jsx"
                | "css"
                | "html"
                | "md"
                | "yml"
                | "yaml"
        )
    )
}

fn print_tree(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", root.display());
    for relative in REQUIRED_PATHS {
        let marker = if root.join(relative).exists() {
            "ok"
        } else {
            "missing"
        };
        println!("  [{marker}] {relative}");
    }
    Ok(())
}
