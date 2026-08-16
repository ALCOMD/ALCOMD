use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

const REQUIRED_PRODUCT_TOKENS: &[&str] = &[
    r#"family_name = "ALCOMD""#,
    r#"technical_name = "alcomd""#,
    r#"display_name = "ALCOMD3""#,
    r#"bundle_identifier = "com.cqmhv.alcomd""#,
    r#"windows_aumid = "CQMHV.ALCOMD""#,
    r#"data_directory = "ALCOMD""#,
    r#"daemon = "alcomd""#,
    r#"gui = "alcomd-gui""#,
    r#"cli = "alcomd-cli""#,
    r#"mcp = "alcomd-mcp""#,
];

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
    "LICENSE-DECISION.md",
    "PLANS.md",
    "alcomd.product.toml",
    "feature-parity.toml",
    "docs/architecture/ALCOMD-V4.md",
    "docs/exec-plans/M-1-audit.md",
    "apps/alcomd",
    "apps/alcomd-gui",
    "apps/alcomd-cli",
    "apps/alcomd-mcp",
    "extensions/first-party/alcomd-extension-mcp",
    "extensions/first-party/alcomd-extension-discord",
];

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
        .expect("xtask must be located directly under the workspace root")
        .to_path_buf()
}

fn check(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut errors = Vec::new();

    for relative in REQUIRED_PATHS {
        if !root.join(relative).exists() {
            errors.push(format!("missing required path: {relative}"));
        }
    }

    let product_path = root.join("alcomd.product.toml");
    let product = fs::read_to_string(&product_path)?;
    for token in REQUIRED_PRODUCT_TOKENS {
        if !product.contains(token) {
            errors.push(format!(
                "{} is missing required token: {token}",
                product_path.display()
            ));
        }
    }

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

fn scan_forbidden_tokens(
    path: &Path,
    errors: &mut Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    if !path.exists() {
        return Ok(());
    }

    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let child = entry.path();
            let name = child
                .file_name()
                .and_then(OsStr::to_str)
                .unwrap_or_default();

            if matches!(name, "target" | "node_modules" | "dist" | ".git") {
                continue;
            }

            scan_forbidden_tokens(&child, errors)?;
        }
        return Ok(());
    }

    if !is_text_file(path) {
        return Ok(());
    }

    let content = fs::read_to_string(path)?;
    for token in FORBIDDEN_PRODUCTION_TOKENS {
        if content.contains(token) {
            errors.push(format!(
                "forbidden legacy production token `{token}` in {}",
                path.display()
            ));
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
