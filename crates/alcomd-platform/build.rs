use std::path::Path;

fn main() {
    let manifest = Path::new("../../alcomd.product.toml");
    println!("cargo:rerun-if-changed={}", manifest.display());
    let text = std::fs::read_to_string(manifest).expect("product metadata");
    let mut section = "";
    let mut directory = None;
    let mut daemon = None;
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
            ("[binaries]", "daemon") => &mut daemon,
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
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')),
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
}
