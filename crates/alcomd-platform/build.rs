use std::path::Path;

fn main() {
    let manifest = Path::new("../../alcomd.product.toml");
    println!("cargo:rerun-if-changed={}", manifest.display());
    let text = std::fs::read_to_string(manifest).expect("product metadata");
    let mut identity = false;
    let mut directory = None;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            identity = line == "[identity]";
        } else if identity
            && let Some(("data_directory", value)) = line
                .split_once('=')
                .map(|(key, value)| (key.trim(), value.trim()))
        {
            assert!(directory.is_none(), "duplicate product directory");
            let value = value
                .strip_prefix('"')
                .and_then(|value| value.strip_suffix('"'))
                .expect("quoted product directory");
            assert!(
                !value.is_empty()
                    && value
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')),
                "product directory must be one plain path component"
            );
            directory = Some(value);
        }
    }
    println!(
        "cargo:rustc-env=ALCOMD_PRODUCT_DIRECTORY={}",
        directory.expect("product identity.data_directory")
    );
}
