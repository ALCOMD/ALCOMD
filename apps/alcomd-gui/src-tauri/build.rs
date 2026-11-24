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
    let uri_scheme = product["identity"]["uri_scheme"]
        .as_str()
        .expect("product URI scheme");
    println!("cargo:rustc-env=ALCOMD_URI_SCHEME={uri_scheme}");
    let desktop_id = product["identity"]["linux_desktop_id"]
        .as_str()
        .expect("product Linux desktop ID");
    println!("cargo:rustc-env=ALCOMD_LINUX_DESKTOP_ID={desktop_id}");
    for (field, variable) in [
        (
            "repository_list_extension",
            "ALCOMD_REPOSITORY_LIST_EXTENSION",
        ),
        ("template_extension", "ALCOMD_TEMPLATE_EXTENSION"),
        ("extension_extension", "ALCOMD_EXTENSION_EXTENSION"),
    ] {
        let value = product["formats"][field]
            .as_str()
            .expect("product file extension");
        println!("cargo:rustc-env={variable}={value}");
    }
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        // The headless native test links the Windows GUI runtime, which needs
        // Common Controls v6. Tauri supplies its own manifest for the app.
        println!("cargo:rustc-link-arg-tests=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-tests=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
        );
    }
    tauri_build::build();
}
