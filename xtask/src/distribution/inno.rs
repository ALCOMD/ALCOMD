use std::fmt::Write as _;
use std::path::Path;

use alcomd_updater::distribution::{
    Candidate, MANIFEST_NAME, SIGNATURE_NAME, Target, VerifiedRuntime, WINDOWS_CRT_FILE,
    format_digest,
};
use anyhow::{Context, ensure};

use super::{ProductConfig, publish_file};

pub(super) fn export(
    layout: &Path,
    product: &ProductConfig,
    candidate: &Candidate,
    verified: &VerifiedRuntime,
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
    for relative in verified.files().chain([MANIFEST_NAME, SIGNATURE_NAME]) {
        ensure!(
            relative.split('/').all(plain_segment),
            "invalid Inno runtime path"
        );
        add_file(
            &mut files,
            &format!("runtime\\{version}\\{}", relative.replace('/', "\\")),
        )?;
    }
    for entry in [
        format!("{}.exe", product.binaries.cli),
        WINDOWS_CRT_FILE.into(),
    ] {
        add_file(&mut files, &format!("bin\\{entry}"))?;
    }

    let bootstrap = format!("{}.exe", product.binaries.bootstrap);
    let bootstrap_digest = verified
        .file_sha256(&bootstrap)
        .context("Bootstrap missing from verified Inno runtime")?;
    let crt_digest = verified
        .file_sha256(WINDOWS_CRT_FILE)
        .context("CRT missing from verified Inno runtime")?;
    let script = include_str!("inno.iss")
        .replace("@APP_ID@", app_id)
        .replace("@APP_NAME@", &product.product.display_name)
        .replace("@APP_AUMID@", &product.identity.windows_aumid)
        .replace("@APP_PUBLISHER@", &product.product.publisher_name)
        .replace("@APP_VERSION@", &version)
        .replace("@FAMILY_NAME@", &product.product.family_name)
        .replace("@INSTALL_DIRECTORY@", &product.identity.install_directory)
        .replace("@GUI_NAME@", &product.binaries.gui)
        .replace("@BOOTSTRAP_FILE@", &bootstrap)
        .replace("@BOOTSTRAP_DIGEST@", &format_digest(bootstrap_digest))
        .replace("@CRT_FILE@", WINDOWS_CRT_FILE)
        .replace("@CRT_DIGEST@", &format_digest(crt_digest))
        .replace("@SOURCE_SHA@", &candidate.source_commit)
        .replace("@MANIFEST_NAME@", MANIFEST_NAME)
        .replace(
            "@MANIFEST_DIGEST@",
            &format_digest(&verified.manifest_sha256),
        )
        .replace("@FILES@", files.trim_end());
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
        "Source: \"{relative}\"; DestDir: \"{{app}}\\{parent}\"; Flags: ignoreversion"
    )
}
