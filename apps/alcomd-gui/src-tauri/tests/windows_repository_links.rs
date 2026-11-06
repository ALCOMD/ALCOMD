#![cfg(windows)]

// Include the private native adapter without making it part of the app's API.
#[allow(dead_code)]
#[path = "../src/repository_links.rs"]
mod repository_links;

use repository_links::{RepositoryLinks, forward_arguments};
use tauri::Manager;

#[test]
#[ignore = "requires the Windows native message loop and two fixture processes"]
fn windows_single_instance_forwards_repository_link() {
    use std::{
        fs,
        path::PathBuf,
        process::{Child, Command},
        thread,
        time::{Duration, Instant},
    };

    const ENV: &str = "ALCOMD_VCC_LINK_TEST_DIRECTORY";
    const TEST: &str = "windows_single_instance_forwards_repository_link";
    const COLD: &str = "vcc://vpm/addRepo?url=https%3A%2F%2Fcold.test%2Findex.json";
    const WARM: &str = "vcc://vpm/addRepo?url=https%3A%2F%2Fwarm.test%2Findex.json&headers%5B%5D=Authorization%3Afixture%7Cvalue";

    if let Some(directory) = std::env::var_os(ENV) {
        let directory = PathBuf::from(directory);
        let mut context = tauri::generate_context!();
        context.config_mut().identifier = format!(
            "com.vrclearn.alcomd.fixture.{}",
            directory
                .file_name()
                .expect("fixture identity")
                .to_string_lossy()
        );
        context.config_mut().app.windows.clear();
        tauri::Builder::default()
            .any_thread()
            .manage(RepositoryLinks::from_arguments())
            .plugin(tauri_plugin_single_instance::init(
                |app, mut arguments, _| {
                    // Strip the libtest options for this fixture. The shell
                    // delivers only the executable and URI in the real app.
                    let uri = arguments.pop().expect("forwarded fixture URI");
                    forward_arguments(app, vec![arguments.remove(0), uri]);
                },
            ))
            .setup(move |app| {
                let app = app.handle().clone();
                thread::spawn(move || {
                    let inbox = app.state::<RepositoryLinks>();
                    let first = inbox.take().unwrap_or_default();
                    fs::write(directory.join("ready"), first).expect("cold evidence");
                    let deadline = Instant::now() + Duration::from_secs(15);
                    while Instant::now() < deadline {
                        if let Some(link) = inbox.take() {
                            fs::write(directory.join("forwarded"), link).expect("warm evidence");
                            app.exit(0);
                            return;
                        }
                        thread::sleep(Duration::from_millis(20));
                    }
                    app.exit(1);
                });
                Ok(())
            })
            .build(context)
            .expect("isolated native app")
            .run(|_, _| {});
        return;
    }

    struct FixtureChild(Child);
    impl Drop for FixtureChild {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let directory = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
    fs::create_dir(&directory).expect("owned fixture directory");
    let launch = |uri: &str| {
        FixtureChild(
            Command::new(std::env::current_exe().expect("test executable"))
                .args([TEST, "--ignored", "--nocapture", uri])
                .env(ENV, &directory)
                .spawn()
                .expect("fixture process"),
        )
    };
    let wait_file = |name| {
        let deadline = Instant::now() + Duration::from_secs(15);
        while Instant::now() < deadline {
            if let Ok(value) = fs::read_to_string(directory.join(name))
                && !value.is_empty()
            {
                return value;
            }
            thread::sleep(Duration::from_millis(20));
        }
        panic!("fixture file {name} did not arrive");
    };
    let mut primary = launch(COLD);
    assert_eq!(wait_file("ready"), COLD);
    let mut secondary = launch(WARM);
    assert_eq!(wait_file("forwarded"), WARM);
    assert!(secondary.0.wait().expect("secondary exits").success());
    assert!(primary.0.wait().expect("primary exits").success());
    fs::remove_file(directory.join("ready")).expect("remove cold evidence");
    fs::remove_file(directory.join("forwarded")).expect("remove warm evidence");
    fs::remove_dir(directory).expect("remove fixture directory");
}
