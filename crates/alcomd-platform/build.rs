use std::path::Path;

fn main() {
    let manifest = Path::new("../../alcomd.product.toml");
    println!("cargo:rerun-if-changed={}", manifest.display());
    let text = std::fs::read_to_string(manifest).expect("product metadata");
    let mut section = "";
    let mut directory = None;
    let mut installation = None;
    let mut daemon = None;
    let mut bundle = None;
    let mut gui = None;
    let mut cli = None;
    let mut mcp = None;
    let mut bootstrap = None;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            section = line;
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let field = match (section, key.trim()) {
            ("[identity]", "data_directory") => &mut directory,
            ("[identity]", "install_directory") => &mut installation,
            ("[binaries]", "daemon") => &mut daemon,
            ("[identity]", "bundle_identifier") => &mut bundle,
            ("[binaries]", "gui") => &mut gui,
            ("[binaries]", "cli") => &mut cli,
            ("[binaries]", "mcp") => &mut mcp,
            ("[binaries]", "bootstrap") => &mut bootstrap,
            _ => continue,
        };
        assert!(field.is_none(), "duplicate product metadata key");
        let value = value
            .trim()
            .strip_prefix('"')
            .and_then(|value| value.strip_suffix('"'))
            .expect("quoted product path component");
        assert!(
            !value.is_empty()
                && value.bytes().all(|byte| byte.is_ascii_alphanumeric()
                    || matches!(byte, b'-' | b'_')
                    || key.trim() == "bundle_identifier" && byte == b'.'),
            "product path component must be a plain name"
        );
        *field = Some(value);
    }
    println!(
        "cargo:rustc-env=ALCOMD_PRODUCT_DIRECTORY={}",
        directory.expect("product identity.data_directory")
    );
    println!(
        "cargo:rustc-env=ALCOMD_DAEMON_NAME={}",
        daemon.expect("product binaries.daemon")
    );
    for (key, value) in [
        ("INSTALL_DIRECTORY", installation),
        ("BUNDLE_ID", bundle),
        ("GUI_NAME", gui),
        ("CLI_NAME", cli),
        ("MCP_NAME", mcp),
        ("BOOTSTRAP_NAME", bootstrap),
    ] {
        println!(
            "cargo:rustc-env=ALCOMD_{key}={}",
            value.expect("product client identity")
        );
    }
}
