#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
#[path = "../windows_bridge/mod.rs"]
mod windows_bridge;

fn main() -> std::process::ExitCode {
    #[cfg(windows)]
    return windows_bridge::run();
    #[cfg(not(windows))]
    std::process::ExitCode::FAILURE
}
