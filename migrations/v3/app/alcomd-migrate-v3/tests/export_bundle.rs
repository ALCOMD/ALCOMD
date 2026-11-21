use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use flate2::{Compression, write::GzEncoder};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

const PAGE: usize = 8192;

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir()
            .canonicalize()
            .expect("absolute fixture parent")
            .join(format!("alcomd-v3-synthetic-{}", Uuid::new_v4()));
        fs::create_dir_all(root.join("source/config")).expect("synthetic source");
        Self(root)
    }

    fn source(&self) -> PathBuf {
        self.0.join("source")
    }
    fn bundle(&self) -> PathBuf {
        self.0.join("staged bundle")
    }
    fn write(&self, name: &str, value: &Value) {
        fs::write(
            self.source().join(name),
            serde_json::to_vec(value).expect("synthetic JSON"),
        )
        .expect("synthetic profile item");
    }
    fn run(&self, command: &str, output: Option<&Path>) -> Output {
        let mut child = Command::new(env!("CARGO_BIN_EXE_alcomd-migrate-v3"));
        child.args([command, "--source-root"]).arg(self.source());
        if let Some(output) = output {
            child.arg("--output").arg(output);
        }
        child.output().expect("execute isolated migration program")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn synthetic_v3_profile_normalizes_records_without_writing_the_source_or_printing_values() {
    let f = Fixture::new();
    let project = f.0.join("synthetic 项目");
    let project_path = project.to_str().expect("Unicode project path");
    let directory = f.0.join("synthetic Unity folders");
    let editor = f.0.join("synthetic editor");
    let editor_path = editor.to_str().expect("Unicode editor path");
    let local_package = f.0.join("package source");
    let secret = "synthetic-repo-secret";
    let old_mcp = "synthetic-mcp-token";
    f.write("settings.json", &json!({
        "userProjects":[project],
        "unityEditors":[editor],
        "defaultProjectPath":directory,
        "showPrereleasePackages":true,
        "userPackageFolders":[local_package],
        "userRepos":[{"localPath":f.0.join("synthetic cache"), "url":"HTTPS://EXAMPLE.INVALID/index.json",
                      "id":"local.example.repo", "name":"Fixture display", "headers":{"Authorization":format!("Bearer {secret}")}}]
    }));
    f.write("config/gui-config.json", &json!({
        "mcpHttpToken":old_mcp,
        "guiHiddenRepositories":["local.example.repo"],
        "language":"zh-CN", "defaultUnityArguments": [], "backupFormat":"zip-store",
        "fullscreen":true, "guiAnimation":false, "unityDiscordSharingEnabled":true,
        "unityDiscordDisplayOptions":{"projectName":false,"unityVersion":true,"editorCount":false,"customText":"synthetic presence"},
        "extensions":{"discord":{"enabled":false},"mcp":{"enabled":false}}
    }));
    f.write("config/theme-config.json", &json!({"theme":"dark"}));
    let db = fixture_db(project_path, editor_path);
    fs::write(f.source().join("vcc.liteDb"), &db).expect("synthetic LiteDB data");
    let settings_before = fs::read(f.source().join("settings.json")).expect("source before");
    let inventory = f.run("inventory", None);
    assert!(inventory.status.success(), "{:?}", inventory.stderr);
    let counts: Value = serde_json::from_slice(&inventory.stdout).expect("counts JSON");
    assert_eq!(counts["projects"], 1);
    assert_eq!(counts["unityInstallations"], 1);
    assert_eq!(counts["repositoryCredentials"], 1);
    assert_eq!(counts["extensions"], 2);
    let export = f.run("export", Some(&f.bundle()));
    assert!(export.status.success(), "{:?}", export.stderr);
    assert_eq!(export.stdout, inventory.stdout);
    let state_raw = fs::read_to_string(f.bundle().join("state.json")).expect("snapshot");
    for private_value in [secret, old_mcp] {
        assert!(!state_raw.contains(private_value));
        assert!(!String::from_utf8_lossy(&export.stdout).contains(private_value));
    }
    let state: Value = serde_json::from_str(&state_raw).expect("state JSON");
    assert_eq!(state["sourceVersion"], "3.4.0");
    assert_eq!(state["projects"][0]["path"], project_path);
    assert_eq!(state["projects"][0]["favorite"], true);
    assert_eq!(
        state["projects"][0]["registeredAtMs"],
        1_700_000_000_001_u64
    );
    assert_eq!(
        state["projects"][0]["unityLaunchArguments"],
        json!(["-logFile", "fixture.txt"])
    );
    assert_eq!(state["projects"][0]["unityExecutablePath"], editor_path);
    assert_eq!(state["unityInstallations"][0]["fromHub"], true);
    assert_eq!(
        state["repositories"][0]["source"]["url"],
        "https://example.invalid/index.json"
    );
    assert_eq!(state["repositories"][0]["hidden"], true);
    assert_eq!(state["repositories"][0]["hasCredentials"], true);
    assert_eq!(state["repositories"][0]["credentialRef"], Value::Null);
    assert_eq!(state["settings"]["projectDirectory"], json!(directory));
    assert_eq!(state["settings"]["locale"], "zh-CN");
    assert_eq!(state["settings"]["appearanceMode"], "dark");
    assert_eq!(state["settings"]["defaultUnityLaunchArguments"], json!([]));
    assert_eq!(state["settings"]["backupCompression"], "store");
    assert_eq!(
        state["discord"],
        json!({"enabled":false,"projectName":false,"unityVersion":true,"editorCount":false,"customText":"synthetic presence"})
    );
    assert_eq!(
        fs::read(f.source().join("settings.json")).expect("source after"),
        settings_before
    );
    assert_eq!(
        fs::read(f.source().join("vcc.liteDb")).expect("database after"),
        db
    );
    let manifest: Value =
        serde_json::from_slice(&fs::read(f.bundle().join("manifest.json")).expect("manifest"))
            .expect("manifest JSON");
    assert_eq!(manifest["records"], counts);
    check_file(&f.bundle(), &manifest["files"][0]);
    assert!(!f.run("export", Some(&f.bundle())).status.success());
    assert_eq!(
        fs::read_to_string(f.bundle().join("state.json")).expect("original state"),
        state_raw
    );
}

#[test]
fn synthetic_archived_template_becomes_a_native_zip_with_verified_manifest_entry() {
    let f = Fixture::new();
    f.write("settings.json", &json!({}));
    let folder = f.source().join("templates");
    fs::create_dir(&folder).expect("template source directory");
    let file = folder.join("synthetic.alcomtemplate");
    let mut source = serde_json::to_vec(&json!({"$type":"com.anatawa12.vrc-get.custom-template","formatVersion":"2.0",
        "displayName":"Fixture Template","id":"dev.example.template","templateKind":"projectArchive",
        "archive":{"format":"tar.gz","unityVersion":"2022.3.22f1"}})).expect("fixture metadata");
    source.push(0);
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    for (path, body) in [
        ("Assets/example.txt", b"fixture".as_slice()),
        (
            "Packages/manifest.json",
            b"{\"dependencies\":{}}".as_slice(),
        ),
        (
            "ProjectSettings/ProjectVersion.txt",
            b"m_EditorVersion: 2022.3.22f1\n".as_slice(),
        ),
    ] {
        append_tar(&mut encoder, path, body);
    }
    encoder.write_all(&[0; 1024]).expect("tar terminator");
    source.extend(encoder.finish().expect("archive bytes"));
    fs::write(&file, &source).expect("synthetic archive");
    let export = f.run("export", Some(&f.bundle()));
    assert!(export.status.success(), "{:?}", export.stderr);
    let manifest: Value =
        serde_json::from_slice(&fs::read(f.bundle().join("manifest.json")).expect("manifest"))
            .expect("manifest JSON");
    assert_eq!(manifest["records"]["extensions"], 0);
    let attachment = &manifest["files"][1];
    assert_eq!(attachment["path"], "templates/0001.alcomdtemplate");
    check_file(&f.bundle(), attachment);
    let input =
        fs::File::open(f.bundle().join("templates/0001.alcomdtemplate")).expect("converted ZIP");
    let mut zip = zip::ZipArchive::new(input).expect("native ZIP root");
    let template: Value =
        serde_json::from_reader(zip.by_name("package.json").expect("native metadata"))
            .expect("native metadata JSON");
    assert_eq!(
        template,
        json!({"name":"dev.example.template","displayName":"Fixture Template"})
    );
    let mut contents = String::new();
    zip.by_name("Assets/example.txt")
        .expect("user asset")
        .read_to_string(&mut contents)
        .expect("asset bytes");
    assert_eq!(contents, "fixture");
    assert_eq!(fs::read(file).expect("original archive"), source);
}

#[test]
fn explicitly_stored_discord_privacy_does_not_create_mcp_preferences_or_enable_sharing() {
    let f = Fixture::new();
    f.write("settings.json", &json!({}));
    f.write(
        "config/gui-config.json",
        &json!({
            "unityDiscordDisplayOptions": { "projectName": false },
            "unityDiscordSharingEnabled": false
        }),
    );
    let export = f.run("export", Some(&f.bundle()));
    assert!(export.status.success(), "{:?}", export.stderr);
    let count: Value = serde_json::from_slice(&export.stdout).expect("counts JSON");
    assert_eq!(count["extensions"], 1);
    let state: Value =
        serde_json::from_slice(&fs::read(f.bundle().join("state.json")).expect("native state"))
            .expect("native JSON");
    assert_eq!(
        state["extensions"],
        json!([{ "id": "discord", "enabled": true, "visible": true }])
    );
    assert_eq!(state["discord"]["enabled"], false);
    assert_eq!(state["discord"]["projectName"], false);
}

#[test]
fn bad_source_or_output_is_refused_before_publishing_or_logging_secrets() {
    let f = Fixture::new();
    f.write("settings.json", &json!({ "userRepos":[{
        "localPath": f.0.join("repo"), "url": "https://synthetic-user:synthetic-secret@example.invalid/repo.json"
    }]}));
    let attempted = f.run("export", Some(&f.bundle()));
    assert!(!attempted.status.success());
    assert!(!String::from_utf8_lossy(&attempted.stderr).contains("synthetic-secret"));
    assert!(!f.bundle().exists());
    f.write("settings.json", &json!({ "userRepos":[{
        "localPath": f.0.join("repo"), "url": "https://example.invalid/repo.json",
        "name":"public label contains synthetic-secret", "headers": {"Authorization":"Bearer synthetic-secret"}
    }]}));
    let attempted = f.run("export", Some(&f.bundle()));
    assert!(!attempted.status.success());
    assert!(!String::from_utf8_lossy(&attempted.stderr).contains("synthetic-secret"));
    assert!(!f.bundle().exists());
    f.write("settings.json", &json!({}));
    let db = f.source().join("vcc.liteDb");
    fs::write(&db, b"unknown persisted bytes").expect("unrecognized state");
    assert!(!f.run("export", Some(&f.bundle())).status.success());
    assert_eq!(
        fs::read(&db).expect("retained evidence"),
        b"unknown persisted bytes"
    );
    fs::remove_file(db).expect("remove synthetic invalid item");
    let inside = f.source().join("arbitrary new directory");
    assert!(!f.run("export", Some(&inside)).status.success());
    assert!(!inside.exists());
}

fn check_file(root: &Path, entry: &Value) {
    let bytes =
        fs::read(root.join(entry["path"].as_str().expect("manifest path"))).expect("manifest file");
    let digest = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(entry["sha256"], digest);
    assert_eq!(entry["size"], bytes.len());
}

fn fixture_db(project: &str, unity: &str) -> Vec<u8> {
    let mut bytes = vec![0; PAGE * 5];
    let header = &mut bytes[..PAGE];
    header[4] = 1;
    header[32..59].copy_from_slice(b"** This is a LiteDB file **");
    header[59] = 8;
    header[64..68].copy_from_slice(&4_u32.to_le_bytes());
    let collections = bson(&[
        (0x10, "projects", 1_u32.to_le_bytes().to_vec()),
        (0x10, "unityVersions", 2_u32.to_le_bytes().to_vec()),
    ]);
    header[192..192 + collections.len()].copy_from_slice(&collections);
    for id in 1_u32..=4 {
        let page = &mut bytes[id as usize * PAGE..(id as usize + 1) * PAGE];
        page[..4].copy_from_slice(&id.to_le_bytes());
        page[4] = if id <= 2 { 2 } else { 4 };
    }
    let args = bson(&[
        (0x02, "0", text("-logFile")),
        (0x02, "1", text("fixture.txt")),
    ]);
    let details = bson(&[
        (0x04, "custom_unity_args", args),
        (0x02, "unity_path", text(unity)),
    ]);
    let project = bson(&[
        (0x02, "Path", text(project)),
        (0x02, "UnityVersion", text("2022.3.22f1")),
        (0x08, "Favorite", vec![1]),
        (
            0x09,
            "CreatedAt",
            1_700_000_000_001_i64.to_le_bytes().to_vec(),
        ),
        (0x03, "vrc-get", details),
    ]);
    let editor = bson(&[
        (0x02, "Path", text(unity)),
        (0x02, "Version", text("2022.3.22f1")),
        (0x08, "LoadedFromHub", vec![1]),
    ]);
    add_document(&mut bytes[PAGE * 3..PAGE * 4], 1, &project);
    add_document(&mut bytes[PAGE * 4..PAGE * 5], 2, &editor);
    bytes
}

fn add_document(page: &mut [u8], collection: u32, document: &[u8]) {
    page[19..23].copy_from_slice(&collection.to_le_bytes());
    page[23] = 1;
    page[33..38].fill(u8::MAX);
    page[38..38 + document.len()].copy_from_slice(document);
    page[PAGE - 4..PAGE - 2].copy_from_slice(&((6 + document.len()) as u16).to_le_bytes());
    page[PAGE - 2..PAGE].copy_from_slice(&32_u16.to_le_bytes());
}

fn bson(elements: &[(u8, &str, Vec<u8>)]) -> Vec<u8> {
    let mut body = vec![0; 4];
    for (kind, name, contents) in elements {
        body.push(*kind);
        body.extend(name.as_bytes());
        body.push(0);
        body.extend(contents);
    }
    body.push(0);
    let count = body.len() as u32;
    body[..4].copy_from_slice(&count.to_le_bytes());
    body
}

fn text(value: &str) -> Vec<u8> {
    let mut buffer = ((value.len() + 1) as u32).to_le_bytes().to_vec();
    buffer.extend(value.as_bytes());
    buffer.push(0);
    buffer
}

fn append_tar(destination: &mut impl Write, path: &str, payload: &[u8]) {
    let mut header = [0; 512];
    header[..path.len()].copy_from_slice(path.as_bytes());
    header[124..136].copy_from_slice(format!("{:011o}\0", payload.len()).as_bytes());
    header[148..156].fill(b' ');
    header[156] = b'0';
    header[257..263].copy_from_slice(b"ustar\0");
    let checksum: u64 = header.iter().copied().map(u64::from).sum();
    header[148..156].copy_from_slice(format!("{checksum:06o}\0 ").as_bytes());
    destination.write_all(&header).expect("tar header");
    destination.write_all(payload).expect("tar data");
    destination
        .write_all(&vec![0; (512 - payload.len() % 512) % 512])
        .expect("tar padding");
}

#[test]
fn standalone_select_previews_only_confirmed_legacy_inputs() {
    let f = Fixture::new();
    let project = f.0.join("selected project");
    let editor = f.0.join("selected editor");
    f.write(
        "settings.json",
        &json!({"userProjects":[project], "unityEditors":[editor], "userRepos": false}),
    );
    fs::create_dir(f.source().join("vrc-get")).expect("legacy GUI directory");
    f.write(
        "vrc-get/gui-config.json",
        &json!({"theme":"system","mcpHttpToken":"synthetic-unselected-secret"}),
    );
    f.write("vrc-get/theme-config.json", &json!({"theme":"dark"}));
    let execute = |source, category: &str| {
        Command::new(env!("CARGO_BIN_EXE_alcomd-migrate-v3"))
            .args([
                "select",
                "--source",
                source,
                "--category",
                category,
                "--source-root",
            ])
            .arg(f.source())
            .output()
            .expect("isolated preview process")
    };
    let vcc = execute("vcc", "projects");
    assert!(vcc.status.success());
    let first: Value = serde_json::from_slice(&vcc.stdout).expect("selection plan");
    assert_eq!(first["projects"], 1);
    assert_eq!(first["unityInstallations"], 1);
    assert_eq!(first["planDigest"].as_str().expect("digest").len(), 64);
    assert_eq!(execute("vcc", "projects").stdout, vcc.stdout);
    assert!(!String::from_utf8_lossy(&vcc.stdout).contains("selected project"));
    let alcom = execute("alcom", "projects");
    assert!(alcom.status.success());
    let alcom_plan: Value = serde_json::from_slice(&alcom.stdout).expect("ALCOM plan");
    assert_ne!(alcom_plan["planDigest"], first["planDigest"]);
    assert!(execute("alcomd3-beta", "theme").status.success());
    let beta: Value =
        serde_json::from_slice(&execute("alcomd3-beta", "theme").stdout).expect("Beta theme plan");
    assert_eq!(beta["appearanceMode"], "dark");
    assert!(!execute("alcomd3-beta", "projects").status.success());
    assert!(!execute("vcc", "resources").status.success());
    assert!(!f.bundle().exists());
}
