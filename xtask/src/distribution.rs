use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use alcomd_updater::Channel;
use alcomd_updater::distribution::{
    Candidate, MANIFEST_NAME, SIGNATURE_DOMAIN, SIGNATURE_NAME, Target, WindowsCrtDigests,
    format_digest, prepare_fixture, prepare_release, verify_fixture, verify_release,
};
use anyhow::{Context, ensure};
use clap::{ArgGroup, Parser};
use ed25519_dalek::{Signer, SigningKey};

use crate::{ProductConfig, read_toml};

mod bridge;
mod build;
mod inno;
mod package;
mod webview2;

const FIXTURE_KEY: [u8; 32] = [37; 32];

#[derive(Debug, Parser)]
#[command(
    bin_name = "cargo xtask dist",
    about = "Prepare and verify an offline v4 runtime candidate",
    group(ArgGroup::new("action").args(["build", "prepare", "check", "package_layout", "prepare_bridge"]).required(true))
)]
struct Arguments {
    #[arg(long, value_enum)]
    target: Target,
    /// Compile and stage an unsigned runtime from a clean source checkout.
    #[arg(long, requires_all = ["discord_package", "discord_sha256"], conflicts_with = "fixture")]
    build: bool,
    /// Write the exact manifest bytes for external signing.
    #[arg(long, requires = "staging")]
    prepare: bool,
    /// Verify the manifest signature and the entire staged runtime.
    #[arg(long)]
    check: bool,
    /// Export signed product packaging inputs or development fixtures.
    #[arg(long, requires_all = ["staging", "format", "output"])]
    package_layout: bool,
    /// Prepare a v3 migration payload from the verified, complete release runtime.
    #[arg(long, requires_all = ["staging", "bridge_output", "bridge_channel"], conflicts_with = "fixture")]
    prepare_bridge: bool,
    /// New, independent migration staging directory and signature message.
    #[arg(long, requires = "prepare_bridge")]
    bridge_output: Option<PathBuf>,
    /// Channel of the existing v1 bridge payload.
    #[arg(long, value_enum, requires = "prepare_bridge")]
    bridge_channel: Option<Channel>,
    #[arg(long, value_enum, requires = "package_layout")]
    format: Option<package::Format>,
    #[arg(long, requires = "package_layout")]
    output: Option<PathBuf>,
    /// Use development-only trust for a staged or disposable fixture.
    #[arg(long, conflicts_with = "source_sha")]
    fixture: bool,
    /// The runtime staging directory (new for --build, existing otherwise).
    #[arg(long, required_unless_present = "fixture")]
    staging: Option<PathBuf>,
    /// The full source commit supplied by the build pipeline.
    #[arg(long, required_unless_present = "fixture")]
    source_sha: Option<String>,
    /// Signed first-party extension archive for the staged runtime.
    #[arg(long, requires = "build")]
    discord_package: Option<PathBuf>,
    /// Pinned SHA-256 of the first-party extension archive.
    #[arg(long, conflicts_with_all = ["fixture", "check", "package_layout", "prepare_bridge"])]
    discord_sha256: Option<String>,
    /// The x64 vcruntime140.dll from a licensed Visual Studio redistributable directory.
    #[arg(long, requires_all = ["build", "windows_crt_sha256"])]
    windows_crt: Option<PathBuf>,
    /// Pinned SHA-256 of the exact app-local Windows VC runtime DLL.
    #[arg(long, conflicts_with_all = ["fixture", "check", "package_layout", "prepare_bridge"])]
    windows_crt_sha256: Option<String>,
    /// The x64 vcruntime140_1.dll from the same Visual Studio redistributable directory.
    #[arg(long, requires_all = ["build", "windows_crt_companion_sha256"])]
    windows_crt_companion: Option<PathBuf>,
    /// Pinned SHA-256 of the exact app-local Windows VC runtime companion DLL.
    #[arg(long, conflicts_with_all = ["fixture", "check", "package_layout", "prepare_bridge"])]
    windows_crt_companion_sha256: Option<String>,
    /// Microsoft's online Evergreen Bootstrapper for a Windows release Inno layout.
    #[arg(long, requires_all = ["package_layout", "webview2_bootstrapper_sha256"], conflicts_with = "fixture")]
    webview2_bootstrapper: Option<PathBuf>,
    /// Pinned SHA-256 of the exact WebView2 Bootstrapper.
    #[arg(long, requires = "webview2_bootstrapper")]
    webview2_bootstrapper_sha256: Option<String>,
    /// Microsoft's x64 Evergreen Standalone installer for offline deployment.
    #[arg(long, requires_all = ["webview2_bootstrapper", "webview2_standalone_sha256"])]
    webview2_standalone: Option<PathBuf>,
    /// Pinned SHA-256 of the exact x64 WebView2 Standalone installer.
    #[arg(long, requires = "webview2_standalone")]
    webview2_standalone_sha256: Option<String>,
}

pub fn run(root: &Path) -> anyhow::Result<()> {
    let args = Arguments::parse_from(std::env::args_os().skip(1));
    let product: ProductConfig =
        read_toml(&root.join("alcomd.product.toml")).map_err(|error| anyhow::anyhow!("{error}"))?;
    let candidate = Candidate {
        target: args.target,
        version: product.product.version.parse()?,
        source_commit: args.source_sha.unwrap_or_else(|| "0".repeat(40)),
    };
    if args.fixture && args.staging.is_none() {
        let fixture = tempfile::tempdir()?;
        create_fixture(&fixture.path().canonicalize()?, &product, &candidate)?;
        println!(
            "Verified synthetic {} runtime for {:?}",
            candidate.version, args.target
        );
        return Ok(());
    }
    let path = std::path::absolute(args.staging.context("staging directory is required")?)?;
    let crt_digests = match (
        args.windows_crt_sha256.as_deref(),
        args.windows_crt_companion_sha256.as_deref(),
    ) {
        (Some(primary), Some(companion)) => Some(WindowsCrtDigests { primary, companion }),
        (None, None) => None,
        _ => anyhow::bail!("supply both --windows-crt-sha256 and --windows-crt-companion-sha256"),
    };
    if args.prepare_bridge {
        let output = std::path::absolute(args.bridge_output.context("bridge output is required")?)?;
        bridge::export(
            root,
            &path,
            &output,
            &product,
            &candidate,
            args.bridge_channel.context("bridge channel is required")?,
        )?;
    } else if args.package_layout {
        let output = std::path::absolute(args.output.context("package output is required")?)?;
        let webview2 = args
            .webview2_bootstrapper
            .zip(args.webview2_bootstrapper_sha256)
            .map(|(path, sha256)| webview2::Inputs {
                bootstrapper: webview2::PinnedInstaller { path, sha256 },
                standalone: args
                    .webview2_standalone
                    .zip(args.webview2_standalone_sha256)
                    .map(|(path, sha256)| webview2::PinnedInstaller { path, sha256 }),
            });
        let variant = if args.fixture {
            package::Variant::Fixture
        } else {
            package::Variant::Release(webview2.as_ref())
        };
        package::export(
            root,
            &path,
            &output,
            &product,
            &candidate,
            args.format.context("package format is required")?,
            variant,
        )?;
    } else if args.build {
        let extension = std::path::absolute(
            args.discord_package
                .context("Discord package is required")?,
        )?;
        let crt_primary = args.windows_crt.map(std::path::absolute).transpose()?;
        let crt_companion = args
            .windows_crt_companion
            .map(std::path::absolute)
            .transpose()?;
        let crt = match (
            crt_primary.as_deref(),
            crt_companion.as_deref(),
            crt_digests,
        ) {
            (Some(primary), Some(companion), Some(digests)) => Some(build::WindowsCrtInputs {
                primary,
                companion,
                digests,
            }),
            (None, None, None) => None,
            _ => anyhow::bail!(
                "supply both --windows-crt and --windows-crt-companion and their SHA-256 pins"
            ),
        };
        build::runtime(
            root,
            &path,
            &product,
            &candidate,
            &extension,
            args.discord_sha256
                .as_deref()
                .context("Discord digest is required")?,
            crt,
        )?;
    } else if args.fixture {
        let key = SigningKey::from_bytes(&FIXTURE_KEY);
        if args.prepare {
            let (staging, _) = alcomd_platform::resolve_plain_storage_directory(&path)?;
            sign_fixture(&staging, &prepare_fixture(&staging, &candidate)?, &key)?;
        }
        let digest = verify_fixture(&path, &candidate, &key.verifying_key())?;
        println!(
            "Verified fixture runtime manifest {}",
            format_digest(&digest.manifest_sha256)
        );
    } else if args.prepare {
        let digest = args
            .discord_sha256
            .as_deref()
            .context("Discord digest is required for release preparation")?;
        let (staging, identity) = alcomd_platform::resolve_plain_storage_directory(&path)?;
        let bytes = prepare_release(&staging, &candidate, digest, crt_digests)?;
        build::verify_discord_package(
            root,
            &staging.join("extensions/discord.alcomdextension"),
            digest,
        )?;
        ensure!(
            alcomd_platform::resolve_plain_storage_directory(&staging)?.1 == identity,
            "staging directory changed during manifest preparation"
        );
        publish_manifest(&staging, &bytes)?;
        println!("Prepared {MANIFEST_NAME} for {}", candidate.version);
    } else {
        let verified = verify_release(&path, &candidate)?;
        println!(
            "Verified runtime manifest {}",
            format_digest(&verified.manifest_sha256)
        );
    }
    Ok(())
}

fn publish_manifest(staging: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    publish_file(staging, MANIFEST_NAME, bytes)
}

fn publish_file(staging: &Path, name: &str, bytes: &[u8]) -> anyhow::Result<()> {
    let mut temp = tempfile::NamedTempFile::new_in(staging)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    temp.persist_noclobber(staging.join(name))?;
    alcomd_platform::sync_directory(staging)?;
    Ok(())
}

fn runtime_binaries(product: &ProductConfig) -> [&str; 7] {
    let binaries = &product.binaries;
    [
        &binaries.daemon,
        &binaries.gui,
        &binaries.cli,
        &binaries.mcp,
        &binaries.extension_host,
        &binaries.bootstrap,
        &binaries.updater,
    ]
}

fn create_fixture(
    root: &Path,
    product: &ProductConfig,
    candidate: &Candidate,
) -> anyhow::Result<()> {
    for base in runtime_binaries(product) {
        let path = root.join(format!("{base}{}", candidate.target.executable_suffix()));
        fs::write(&path, synthetic_executable(candidate.target))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
        }
    }
    fs::write(root.join("LICENSE"), b"Synthetic fixture license")?;
    fs::write(
        root.join("THIRD_PARTY_NOTICES.md"),
        b"Synthetic fixture notices",
    )?;
    fs::create_dir(root.join("extensions"))?;
    fs::write(
        root.join("extensions/discord.alcomdextension"),
        b"Synthetic fixture package",
    )?;
    let bytes = prepare_fixture(root, candidate)?;
    let signing = SigningKey::from_bytes(&FIXTURE_KEY);
    sign_fixture(root, &bytes, &signing)?;
    verify_fixture(root, candidate, &signing.verifying_key())?;
    Ok(())
}

fn sign_fixture(root: &Path, bytes: &[u8], signing: &SigningKey) -> anyhow::Result<()> {
    let signature = fixture_signature(bytes, signing);
    publish_file(root, SIGNATURE_NAME, signature.as_bytes())?;
    publish_manifest(root, bytes)
}

fn fixture_signature(bytes: &[u8], signing: &SigningKey) -> String {
    let mut message = SIGNATURE_DOMAIN.to_vec();
    message.extend_from_slice(bytes);
    format_digest(&signing.sign(&message).to_bytes())
}

fn synthetic_executable(target: Target) -> [u8; 90] {
    let mut bytes = [0; 90];
    match target {
        Target::Windows => {
            bytes[..2].copy_from_slice(b"MZ");
            bytes[60] = 64;
            bytes[64..70].copy_from_slice(b"PE\0\0\x64\x86");
            bytes[88..90].copy_from_slice(&[0x0b, 0x02]);
        }
        Target::Linux => {
            bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
            bytes[18] = 62;
        }
        Target::MacOs => bytes[..8].copy_from_slice(&[0xcf, 0xfa, 0xed, 0xfe, 12, 0, 0, 1]),
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use alcomd_updater::distribution::{WINDOWS_CRT_COMPANION_FILE, WINDOWS_CRT_FILE};

    fn fixture(target: Target) -> anyhow::Result<(tempfile::TempDir, Candidate)> {
        let product: ProductConfig = toml::from_str(include_str!("../../alcomd.product.toml"))?;
        let root = tempfile::tempdir_in(std::env::temp_dir().canonicalize()?)?;
        let candidate = Candidate {
            target,
            version: product.product.version.parse()?,
            source_commit: "0".repeat(40),
        };
        create_fixture(root.path(), &product, &candidate)?;
        Ok((root, candidate))
    }

    #[test]
    fn bridge_manifest_covers_verified_runtime_files_and_the_migration_reader() -> anyhow::Result<()>
    {
        use alcomd_updater::bridge::prepare_manifest;

        let (runtime, candidate) = fixture(Target::Windows)?;
        let key = SigningKey::from_bytes(&FIXTURE_KEY).verifying_key();
        let verified = verify_fixture(runtime.path(), &candidate, &key)?;
        let stage = tempfile::tempdir_in(std::env::temp_dir().canonicalize()?)?;
        bridge::copy_runtime(runtime.path(), stage.path(), &verified)?;
        let product: ProductConfig = toml::from_str(include_str!("../../alcomd.product.toml"))?;
        let reader = format!("{}.exe", product.binaries.migration_v3);
        let reader_path = stage.path().join(&reader);
        fs::write(&reader_path, synthetic_executable(candidate.target))?;
        let bytes = prepare_manifest(stage.path(), &candidate, &verified, Channel::Beta)?;
        assert_eq!(
            bytes,
            prepare_manifest(stage.path(), &candidate, &verified, Channel::Beta)?
        );
        let manifest: serde_json::Value = serde_json::from_slice(&bytes)?;
        assert_eq!(manifest["schema"], 1);
        assert_eq!(manifest["target"], "windows-x86_64");
        assert_eq!(manifest["channel"], "beta");
        let files = manifest["files"].as_array().context("bridge files")?;
        assert_eq!(files.len(), verified.files().count() + 1);
        assert!(files.iter().any(|file| file["path"] == reader));
        assert!(!stage.path().join(MANIFEST_NAME).exists());
        assert!(!stage.path().join(SIGNATURE_NAME).exists());

        let foreign_manifest = stage.path().join(MANIFEST_NAME);
        fs::write(&foreign_manifest, b"unexpected release metadata")?;
        assert!(prepare_manifest(stage.path(), &candidate, &verified, Channel::Beta).is_err());
        fs::remove_file(foreign_manifest)?;
        fs::write(stage.path().join("LICENSE"), b"changed signed file")?;
        assert!(prepare_manifest(stage.path(), &candidate, &verified, Channel::Beta).is_err());
        Ok(())
    }

    #[test]
    fn package_layout_binds_the_copied_runtime_and_preserves_existing_outputs() -> anyhow::Result<()>
    {
        let root = crate::workspace_root();
        let product: ProductConfig = toml::from_str(include_str!("../../alcomd.product.toml"))?;
        let output = tempfile::tempdir()?;
        let output_root = output.path().canonicalize()?;
        let (stage, candidate) = fixture(Target::Linux)?;
        let stage = stage.path().canonicalize()?;
        let deb = output_root.join("deb");
        let appimage = output_root.join("appimage");
        let inno = output_root.join("inno");
        assert!(
            package::export(
                &root,
                &stage,
                &inno,
                &product,
                &candidate,
                package::Format::Inno,
                package::Variant::Fixture,
            )
            .is_err()
        );
        assert!(!inno.exists());
        for (name, format) in [
            (&deb, package::Format::Deb),
            (&appimage, package::Format::Appimage),
        ] {
            package::export(
                &root,
                &stage,
                name,
                &product,
                &candidate,
                format,
                package::Variant::Fixture,
            )?;
            assert!(
                package::export(
                    &root,
                    &stage,
                    name,
                    &product,
                    &candidate,
                    format,
                    package::Variant::Fixture
                )
                .is_err()
            );
        }
        assert!(deb.join("deb-root/DEBIAN/control").is_file());
        assert!(appimage.join("AppDir/AppRun").is_file());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;

            let fixture = format!("{}-dist-fixture", product.product.technical_name);
            let runtime_path = format!("usr/lib/{fixture}/runtime/{}", candidate.version);
            for tree in [deb.join("deb-root"), appimage.join("AppDir")] {
                let runtime = tree.join(&runtime_path);
                for (file, mode) in [
                    (runtime.clone(), 0o755),
                    (runtime.join(&product.binaries.cli), 0o755),
                    (runtime.join(MANIFEST_NAME), 0o644),
                    (runtime.join(SIGNATURE_NAME), 0o644),
                ] {
                    assert_eq!(file.metadata()?.permissions().mode() & 0o7777, mode);
                }
            }
            let app_dir = appimage.join("AppDir");
            for (file, mode) in [
                (app_dir.join("AppRun"), 0o755),
                (app_dir.join(format!("{fixture}.desktop")), 0o644),
                (deb.join("deb-root/DEBIAN/control"), 0o644),
            ] {
                assert_eq!(file.metadata()?.permissions().mode() & 0o7777, mode);
            }
        }
        fs::write(stage.join("LICENSE"), b"modified fixture")?;
        let output = output_root.join("tampered");
        assert!(
            package::export(
                &root,
                &stage,
                &output,
                &product,
                &candidate,
                package::Format::Deb,
                package::Variant::Fixture,
            )
            .is_err()
        );
        assert!(!output.exists());
        Ok(())
    }

    #[test]
    fn verifies_three_target_fixtures() -> anyhow::Result<()> {
        for target in [Target::Windows, Target::Linux, Target::MacOs] {
            fixture(target)?;
        }
        Ok(())
    }

    #[test]
    fn rejects_wrong_trust_candidate_and_changed_file() -> anyhow::Result<()> {
        let (root, mut candidate) = fixture(Target::Windows)?;
        let correct = SigningKey::from_bytes(&[37; 32]).verifying_key();
        let wrong = SigningKey::from_bytes(&[38; 32]).verifying_key();
        let verified = verify_fixture(root.path(), &candidate, &correct)?;
        verified.verify_copy("LICENSE", &root.path().join("LICENSE"))?;
        assert!(
            verified
                .verify_copy("LICENSE", &root.path().join("THIRD_PARTY_NOTICES.md"))
                .is_err()
        );
        assert!(
            verified
                .verify_copy("ABSENT", &root.path().join("LICENSE"))
                .is_err()
        );
        assert!(verify_fixture(root.path(), &candidate, &wrong).is_err());
        candidate.version.patch += 1;
        assert!(verify_fixture(root.path(), &candidate, &correct).is_err());
        candidate.version.patch -= 1;
        candidate.target = Target::Linux;
        assert!(verify_fixture(root.path(), &candidate, &correct).is_err());
        candidate.target = Target::Windows;
        fs::write(root.path().join("LICENSE"), b"Synthetic fixture licensE")?;
        assert!(verify_fixture(root.path(), &candidate, &correct).is_err());
        Ok(())
    }

    #[test]
    fn rejects_duplicate_or_unapproved_contents_even_when_signed() -> anyhow::Result<()> {
        let (root, candidate) = fixture(Target::Windows)?;
        let signing = SigningKey::from_bytes(&[37; 32]);
        let key = signing.verifying_key();
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(root.path().join(MANIFEST_NAME))?)?;
        let files = manifest["files"].as_array_mut().context("fixture files")?;
        files.push(files[0].clone());
        let bytes = serde_json::to_vec(&manifest)?;
        fs::write(root.path().join(MANIFEST_NAME), &bytes)?;
        fs::write(
            root.path().join(SIGNATURE_NAME),
            fixture_signature(&bytes, &signing),
        )?;
        assert!(verify_fixture(root.path(), &candidate, &key).is_err());

        let (root, candidate) = fixture(Target::Windows)?;
        fs::write(root.path().join("alcomd-migrate-v3.exe"), b"synthetic")?;
        assert!(verify_fixture(root.path(), &candidate, &key).is_err());
        fs::remove_file(root.path().join("alcomd-migrate-v3.exe"))?;
        fs::create_dir(root.path().join("unowned"))?;
        assert!(verify_fixture(root.path(), &candidate, &key).is_err());
        Ok(())
    }

    #[test]
    fn release_preflight_requires_matching_binary_headers_and_complete_layout() -> anyhow::Result<()>
    {
        let (root, mut candidate) = fixture(Target::Windows)?;
        fs::remove_file(root.path().join(MANIFEST_NAME))?;
        fs::remove_file(root.path().join(SIGNATURE_NAME))?;
        let archive = fs::File::open(root.path().join("extensions/discord.alcomdextension"))?;
        let size = archive.metadata()?.len();
        let hash = format_digest(&alcomd_updater::hash_exact(archive, size)?);
        let mut dll = synthetic_executable(Target::Windows);
        dll[87] = 0x20;
        let crt = root.path().join(WINDOWS_CRT_FILE);
        fs::write(&crt, dll)?;
        let crt_hash = format_digest(&alcomd_updater::hash_exact(fs::File::open(&crt)?, 90)?);
        let mut companion_dll = dll;
        companion_dll[2] = 1;
        let companion = root.path().join(WINDOWS_CRT_COMPANION_FILE);
        fs::write(&companion, companion_dll)?;
        let companion_hash = format_digest(&alcomd_updater::hash_exact(
            fs::File::open(&companion)?,
            90,
        )?);
        let pins = WindowsCrtDigests {
            primary: &crt_hash,
            companion: &companion_hash,
        };
        assert!(prepare_release(root.path(), &candidate, &hash, Some(pins)).is_err());
        candidate.source_commit = "1".repeat(40);
        let manifest: serde_json::Value = serde_json::from_slice(&prepare_release(
            root.path(),
            &candidate,
            &hash,
            Some(pins),
        )?)?;
        let files = manifest["files"].as_array().context("release files")?;
        for (name, digest) in [
            (WINDOWS_CRT_FILE, &crt_hash),
            (WINDOWS_CRT_COMPANION_FILE, &companion_hash),
        ] {
            assert!(
                files
                    .iter()
                    .any(|entry| entry["path"] == name && entry["sha256"] == digest.as_str())
            );
        }
        assert!(prepare_release(root.path(), &candidate, &hash, None).is_err());
        let wrong_hash = "1".repeat(64);
        for wrong_pins in [
            WindowsCrtDigests {
                primary: &wrong_hash,
                ..pins
            },
            WindowsCrtDigests {
                companion: &wrong_hash,
                ..pins
            },
        ] {
            assert!(prepare_release(root.path(), &candidate, &hash, Some(wrong_pins)).is_err());
        }
        fs::write(&companion, dll)?;
        let identical_pins = WindowsCrtDigests {
            companion: &crt_hash,
            ..pins
        };
        assert!(prepare_release(root.path(), &candidate, &hash, Some(identical_pins)).is_err());
        fs::write(&companion, companion_dll)?;
        assert!(prepare_release(root.path(), &candidate, &wrong_hash, Some(pins)).is_err());
        for (path, valid, primary) in [(&crt, dll, true), (&companion, companion_dll, false)] {
            fs::remove_file(path)?;
            assert!(prepare_release(root.path(), &candidate, &hash, Some(pins)).is_err());

            let mut wrong_architecture = valid;
            wrong_architecture[68..70].copy_from_slice(&[0x64, 0xaa]);
            let mut not_a_dll = valid;
            not_a_dll[87] = 0;
            for malformed in [wrong_architecture, not_a_dll] {
                fs::write(path, malformed)?;
                let malformed_hash =
                    format_digest(&alcomd_updater::hash_exact(fs::File::open(path)?, 90)?);
                let malformed_pins = if primary {
                    WindowsCrtDigests {
                        primary: &malformed_hash,
                        ..pins
                    }
                } else {
                    WindowsCrtDigests {
                        companion: &malformed_hash,
                        ..pins
                    }
                };
                assert!(
                    prepare_release(root.path(), &candidate, &hash, Some(malformed_pins)).is_err()
                );
            }
            fs::write(path, valid)?;
        }
        let cli = root.path().join("alcomd-cli.exe");
        fs::write(&cli, synthetic_executable(Target::Linux))?;
        assert!(prepare_release(root.path(), &candidate, &hash, Some(pins)).is_err());
        fs::remove_file(cli)?;
        assert!(prepare_release(root.path(), &candidate, &hash, Some(pins)).is_err());
        Ok(())
    }
}
