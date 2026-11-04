fn main() {
    let product_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../alcomd.product.toml");
    println!("cargo:rerun-if-changed={}", product_path.display());
    let source = std::fs::read_to_string(&product_path).expect("read product metadata");
    let product: toml::Value = toml::from_str(&source).expect("parse product metadata");
    let windows_aumid = product["identity"]["windows_aumid"]
        .as_str()
        .expect("product Windows AUMID");
    println!("cargo:rustc-env=ALCOMD_WINDOWS_AUMID={windows_aumid}");
    tauri_build::build();
}
