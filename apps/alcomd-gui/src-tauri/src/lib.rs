/// Starts the official ALCOMD GUI shell.
///
/// Business logic must remain in `alcomd`; this process is only a client and UI host.
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("failed to run alcomd-gui");
}
