fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
        && std::env::var_os("CARGO_FEATURE_WINDOWS_BRIDGE").is_some()
    {
        println!("cargo:rustc-link-arg-bin=alcomd-v3-windows-bridge=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-bin=alcomd-v3-windows-bridge=/MANIFESTUAC:level='asInvoker' uiAccess='false'"
        );
    }
}
