#[cfg(target_os = "linux")]
fn main() -> anyhow::Result<()> {
    alcomd_bootstrap::appimage_bridge::run()
}

#[cfg(not(target_os = "linux"))]
fn main() -> anyhow::Result<()> {
    anyhow::bail!("the AppImage bridge runs on Linux")
}
