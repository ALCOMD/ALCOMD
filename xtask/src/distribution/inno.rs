use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

use alcomd_updater::distribution::{
    Candidate, MANIFEST_NAME, SIGNATURE_NAME, Target, VerifiedRuntime, WINDOWS_CRT_FILE,
    WINDOWS_UPDATE_STAGE_PREFIX, format_digest,
};
use anyhow::{Context, ensure};

use super::{
    ProductConfig, publish_file,
    webview2::{BOOTSTRAPPER_NAME, STANDALONE_NAME, StagedDigests},
};

pub(super) fn export(
    layout: &Path,
    product: &ProductConfig,
    candidate: &Candidate,
    verified: &VerifiedRuntime,
    webview2: &StagedDigests,
) -> anyhow::Result<()> {
    ensure!(
        candidate.target == Target::Windows,
        "Inno requires a Windows runtime"
    );
    let version = candidate.version.to_string();
    for value in [
        &product.product.family_name,
        &product.identity.install_directory,
        &product.identity.windows_aumid,
        &product.binaries.bootstrap,
        &product.binaries.cli,
        &product.binaries.gui,
        &version,
    ] {
        ensure!(plain_segment(value), "invalid Inno product path component");
    }
    for value in [
        &product.product.display_name,
        &product.product.publisher_name,
    ] {
        ensure!(
            !value.is_empty()
                && value
                    .bytes()
                    .all(|byte| byte == b' ' || byte.is_ascii_alphanumeric()),
            "invalid Inno product display text"
        );
    }
    let app_id = &product.identity.windows_app_id;
    ensure!(
        app_id.len() == 38
            && app_id.bytes().enumerate().all(|(index, byte)| match index {
                0 => byte == b'{',
                9 | 14 | 19 | 24 => byte == b'-',
                37 => byte == b'}',
                _ => byte.is_ascii_hexdigit(),
            }),
        "invalid Inno AppId"
    );

    let mut files = String::new();
    writeln!(
        files,
        "Source: \"webview2\\{BOOTSTRAPPER_NAME}\"; Flags: dontcopy noencryption"
    )?;
    if webview2.standalone.is_some() {
        writeln!(
            files,
            "Source: \"webview2\\{STANDALONE_NAME}\"; Flags: dontcopy noencryption"
        )?;
    }
    let mut update_files = String::new();
    let mut uninstall_files = String::new();
    let mut runtime_directories = BTreeSet::new();
    let mut runtime_file_count = 0;
    for relative in verified.files().chain([MANIFEST_NAME, SIGNATURE_NAME]) {
        ensure!(
            relative.split('/').all(plain_segment),
            "invalid Inno runtime path"
        );
        let relative = relative.replace('/', "\\");
        let installed = format!("runtime\\{version}\\{relative}");
        add_file(&mut files, &installed)?;
        add_uninstall_file(&mut uninstall_files, &installed)?;
        writeln!(
            update_files,
            "  if not CopyRuntimeFile(Directory, '{relative}') then Exit;"
        )?;
        if let Some((directory, _)) = installed.rsplit_once('\\') {
            runtime_directories.insert(directory.to_owned());
        }
        runtime_file_count += 1;
    }
    for entry in [
        format!("{}.exe", product.binaries.cli),
        WINDOWS_CRT_FILE.into(),
    ] {
        add_file(&mut files, &format!("bin\\{entry}"))?;
    }
    for directory in runtime_directories.iter().rev() {
        writeln!(
            uninstall_files,
            "Type: dirifempty; Name: \"{{app}}\\{directory}\"; Check: IsUpdateInstallation"
        )?;
    }

    let bootstrap = format!("{}.exe", product.binaries.bootstrap);
    let cli = format!("{}.exe", product.binaries.cli);
    let bootstrap_digest = verified
        .file_sha256(&bootstrap)
        .context("Bootstrap missing from verified Inno runtime")?;
    let crt_digest = verified
        .file_sha256(WINDOWS_CRT_FILE)
        .context("CRT missing from verified Inno runtime")?;
    let webview2_code = include_str!("inno-webview2.iss")
        .replace("@WEBVIEW2_BOOTSTRAPPER_NAME@", BOOTSTRAPPER_NAME)
        .replace("@WEBVIEW2_BOOTSTRAPPER_DIGEST@", &webview2.bootstrapper)
        .replace("@WEBVIEW2_STANDALONE_NAME@", STANDALONE_NAME)
        .replace(
            "@WEBVIEW2_STANDALONE_DIGEST@",
            webview2.standalone.as_deref().unwrap_or(""),
        );
    let script = include_str!("inno.iss")
        .replace("@WEBVIEW2_CODE@", webview2_code.trim_end())
        .replace("@APP_ID@", app_id)
        .replace("@APP_NAME@", &product.product.display_name)
        .replace("@APP_AUMID@", &product.identity.windows_aumid)
        .replace("@APP_PUBLISHER@", &product.product.publisher_name)
        .replace("@APP_VERSION@", &version)
        .replace("@FAMILY_NAME@", &product.product.family_name)
        .replace("@INSTALL_DIRECTORY@", &product.identity.install_directory)
        .replace("@GUI_NAME@", &product.binaries.gui)
        .replace("@CLI_FILE@", &cli)
        .replace("@BOOTSTRAP_FILE@", &bootstrap)
        .replace("@BOOTSTRAP_DIGEST@", &format_digest(bootstrap_digest))
        .replace("@CRT_FILE@", WINDOWS_CRT_FILE)
        .replace("@CRT_DIGEST@", &format_digest(crt_digest))
        .replace("@SOURCE_SHA@", &candidate.source_commit)
        .replace("@MANIFEST_NAME@", MANIFEST_NAME)
        .replace("@UPDATE_STAGE_PREFIX@", WINDOWS_UPDATE_STAGE_PREFIX)
        .replace("@RUNTIME_FILE_COUNT@", &runtime_file_count.to_string())
        .replace(
            "@MANIFEST_DIGEST@",
            &format_digest(&verified.manifest_sha256),
        )
        .replace("@UPDATE_FILES@", update_files.trim_end())
        .replace("@UNINSTALL_FILES@", uninstall_files.trim_end())
        .replace("@FILES@", files.trim_end());
    publish_file(
        layout,
        "active-runtime-v1",
        format!("{version}\n").as_bytes(),
    )?;
    publish_file(layout, "alcomd.iss", script.as_bytes())
}

fn plain_segment(value: &str) -> bool {
    !value.is_empty()
        && !value.ends_with('.')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
}

fn add_file(list: &mut String, relative: &str) -> std::fmt::Result {
    let (parent, _) = relative
        .rsplit_once('\\')
        .expect("package file has a directory");
    writeln!(
        list,
        "Source: \"{relative}\"; DestDir: \"{{app}}\\{parent}\"; Flags: ignoreversion onlyifdoesntexist; Check: IsFreshInstallation"
    )
}

fn add_uninstall_file(list: &mut String, relative: &str) -> std::fmt::Result {
    writeln!(
        list,
        "Type: files; Name: \"{{app}}\\{relative}\"; Check: IsUpdateInstallation"
    )
}
