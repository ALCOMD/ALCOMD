//! Offline import of preferences for the distribution's signed Discord extension.

use std::path::{Path, PathBuf};

use alcomd_application::{
    ExtensionDesiredState, ExtensionInstallConsent, ExtensionInstallPlanDraft,
    ExtensionPermissionGrant, ExtensionQuarantineState, ExtensionRuntimeState, ExtensionSourceKind,
    IdempotencyKey, LOCAL_IPC_PERMISSION, LOCAL_IPC_RESOURCE_KIND, M6ErrorCode, M6PackageAdapter,
    M6Store, PrincipalId, UNITY_SESSION_PERMISSION, VerifiedExtensionActivation, oauth_hex,
};
use alcomd_extensions::{
    ExtensionEngine, VerifiedExtensionPackage, inspect_extension_directory,
    inspect_extension_package_with_archive_digest,
};
use alcomd_import::DiscordPreference;
use alcomd_platform::resolve_plain_storage_directory;
use alcomd_store::StateStoreHandle;
use anyhow::{Context, Result, ensure};
use serde::Serialize;

const EXTENSION_ID: &str = "com.vrclearn.alcomd-discord-presence";
const TARGET_ID: &str = "discord";
const SETTINGS_KEY: &str = "presence-settings";
const PERMISSIONS: [&str; 4] = [
    "background.run",
    LOCAL_IPC_PERMISSION,
    "projects.read",
    UNITY_SESSION_PERMISSION,
];

/// Verified distribution evidence and user consent supplied by the native migration bridge.
#[derive(Clone, Copy)]
pub struct DiscordImportOptions<'a> {
    pub package: &'a Path,
    pub package_sha256: &'a str,
    pub publish_data_dir: &'a Path,
    pub consent_digest: Option<&'a str>,
}

#[derive(Serialize)]
struct PresenceSettings<'a> {
    show_project: bool,
    show_version: bool,
    show_count: bool,
    custom_text: &'a str,
}

fn settings(preference: &DiscordPreference) -> Result<Vec<u8>> {
    ensure!(
        preference.custom_text.chars().count() <= 128
            && !preference.custom_text.chars().any(char::is_control),
        "invalid Discord presence text"
    );
    Ok(serde_json::to_vec(&PresenceSettings {
        show_project: preference.project_name,
        show_version: preference.unity_version,
        show_count: preference.editor_count,
        custom_text: &preference.custom_text,
    })?)
}

fn needs_install(preference: &DiscordPreference, explicitly_present: bool) -> bool {
    explicitly_present
        || preference.enabled
        || !preference.project_name
        || !preference.unity_version
        || !preference.editor_count
        || !preference.custom_text.is_empty()
}

async fn verified_package(
    options: DiscordImportOptions<'_>,
    enabled: bool,
) -> Result<(VerifiedExtensionPackage, String)> {
    let parent = options
        .package
        .parent()
        .context("Discord package parent is missing")?;
    resolve_plain_storage_directory(parent)?;
    ensure!(
        options.package.is_absolute()
            && options
                .package
                .extension()
                .is_some_and(|extension| extension == "alcomdextension"),
        "Discord package must be an absolute extension archive"
    );
    let metadata = std::fs::symlink_metadata(options.package)?;
    ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "Discord package must be a plain file"
    );
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        ensure!(
            metadata.file_attributes() & 0x400 == 0,
            "Discord package is a reparse point"
        );
    }

    let path = options.package.to_owned();
    let archive_sha256 = options.package_sha256.to_owned();
    let package = tokio::task::spawn_blocking(move || {
        inspect_extension_package_with_archive_digest(&path, &archive_sha256)
    })
    .await?
    .context("Discord distribution digest or extension signature did not verify")?;
    let manifest = &package.manifest;
    ensure!(
        manifest.id == EXTENSION_ID
            && manifest.api == 1
            && package.mcp_tools.is_empty()
            && manifest.mcp.is_none()
            && manifest.installation_terms.is_none()
            && manifest.permissions.required.is_empty()
            && manifest.permissions.optional.len() == PERMISSIONS.len()
            && PERMISSIONS.iter().all(|permission| manifest
                .permissions
                .optional
                .iter()
                .any(|value| value == permission)),
        "signed package does not match the Discord migration contract"
    );
    let integration = manifest.background_integration();
    ensure!(
        integration.enabled && integration.local_ipc_targets.len() == 1,
        "signed Discord package has unexpected activation targets"
    );
    let target_scope = integration
        .local_ipc_targets
        .get(TARGET_ID)
        .context("signed package is missing its Discord activation target")?
        .scope_key(TARGET_ID);
    if enabled {
        ensure!(
            options.consent_digest == Some(oauth_hex(&package.package_digest).as_str()),
            "enabled Discord presence requires consent for the verified extension package digest"
        );
    }
    Ok((package, target_scope))
}

fn committed_root(path: &Path) -> Result<PathBuf> {
    let parent = path
        .parent()
        .context("committed data directory parent is missing")?;
    let name = path
        .file_name()
        .context("committed data directory name is missing")?;
    let (parent, _) = resolve_plain_storage_directory(parent)?;
    Ok(parent.join(name))
}

pub(super) async fn stage(
    store: &StateStoreHandle,
    stage_root: &Path,
    preference: &DiscordPreference,
    explicitly_present: bool,
    options: Option<DiscordImportOptions<'_>>,
    now_ms: u64,
) -> Result<()> {
    if !needs_install(preference, explicitly_present) {
        return Ok(());
    }
    let bytes = settings(preference)?;
    let options =
        options.context("Discord preferences require the signed distribution extension")?;
    let (package, target_scope) = verified_package(options, preference.enabled).await?;
    let final_root = committed_root(options.publish_data_dir)?;
    let (stage_root, _) = resolve_plain_storage_directory(stage_root)?;
    let engine = ExtensionEngine::new(store.clone(), stage_root.join("extensions"))?
        .with_tools_validator(
            super::m6_runtime::PlatformExtensionRuntime::new(store.clone())?.host_executable(),
        );
    let evidence = engine
        .inspect(
            ExtensionSourceKind::FirstPartyPackaged,
            options
                .package
                .to_str()
                .context("Discord package path must be Unicode")?
                .into(),
        )
        .await?;
    ensure!(
        evidence.extension_id == EXTENSION_ID
            && evidence.package_digest == package.package_digest
            && evidence.manifest_digest == package.manifest_digest
            && evidence.component_digest == package.component_digest
            && evidence.publisher_fingerprint == package.publisher_fingerprint,
        "verified Discord distribution package changed"
    );
    let owner = PrincipalId::local_owner();
    let plan = store
        .create_install_plan(
            owner.clone(),
            ExtensionInstallPlanDraft {
                plan_fingerprint: evidence.package_digest,
                evidence,
                expected_revision: None,
            },
            now_ms,
        )
        .await?;
    let accepted = store
        .accept_plan(
            owner.clone(),
            plan.plan_id,
            Some(ExtensionInstallConsent {
                plan_fingerprint: oauth_hex(&plan.plan_fingerprint),
                // The verified manifest contains neither terms nor required permissions.
                installation_terms_accepted: false,
                required_permissions_accepted: false,
            }),
            IdempotencyKey::parse("migration:discord:install")?,
            now_ms,
        )
        .await?;
    let operation = accepted.operation_id;
    let pending = store.begin_apply(operation, now_ms).await?;
    let locator = engine.install(operation, pending.clone()).await?;
    let relative = Path::new(&locator).strip_prefix(&stage_root)?;
    ensure!(
        relative.starts_with("extensions/packages"),
        "invalid staged Discord package path"
    );
    let final_locator = final_root.join(relative);
    store
        .finish_install(
            operation,
            final_locator
                .to_str()
                .context("committed Discord package path must be Unicode")?
                .into(),
            now_ms,
        )
        .await?;
    engine.cleanup(operation, pending).await?;
    store.complete_operation(operation, now_ms).await?;
    store
        .migration_extension_value(
            EXTENSION_ID.into(),
            package.package_digest,
            SETTINGS_KEY.into(),
            Some((bytes, now_ms)),
        )
        .await?;
    if preference.enabled {
        let record = store
            .get_extension(owner.clone(), EXTENSION_ID.into())
            .await?;
        store
            .enable_consented(
                owner,
                EXTENSION_ID.into(),
                record.revision,
                IdempotencyKey::parse("migration:discord:enable")?,
                now_ms,
                VerifiedExtensionActivation {
                    package_digest: package.package_digest,
                    target_scopes: vec![target_scope],
                },
            )
            .await?;
    }
    Ok(())
}

pub(super) async fn check(
    store: &StateStoreHandle,
    stage_root: &Path,
    preference: &DiscordPreference,
    explicitly_present: bool,
    options: Option<DiscordImportOptions<'_>>,
) -> Result<()> {
    let owner = PrincipalId::local_owner();
    let installed = store
        .get_extension(owner.clone(), EXTENSION_ID.into())
        .await;
    if !needs_install(preference, explicitly_present) {
        ensure!(
            matches!(installed, Err(error) if error.code() == M6ErrorCode::NotInstalled),
            "staged Discord installation does not match source preferences"
        );
        return Ok(());
    }
    let options =
        options.context("Discord preferences require the signed distribution extension")?;
    let (package, target_scope) = verified_package(options, preference.enabled).await?;
    let record = installed?;
    let desired = if preference.enabled {
        ExtensionDesiredState::Enabled
    } else {
        ExtensionDesiredState::InstalledDisabled
    };
    ensure!(
        record.desired_state == desired
            && record.quarantine_state == ExtensionQuarantineState::Clear
            && record.runtime_state == ExtensionRuntimeState::Stopped
            && record.package_digest == package.package_digest
            && record.publisher_fingerprint == package.publisher_fingerprint,
        "staged Discord identity or activation differs from the source preferences"
    );
    let locator = store
        .live_package_locator(owner.clone(), EXTENSION_ID.into())
        .await?;
    let final_root = committed_root(options.publish_data_dir)?;
    let relative = Path::new(&locator).strip_prefix(final_root)?;
    ensure!(
        relative.starts_with("extensions/packages"),
        "invalid committed Discord package path"
    );
    let (staged, _) = resolve_plain_storage_directory(&stage_root.join(relative))?;
    let installed_package =
        tokio::task::spawn_blocking(move || inspect_extension_directory(&staged)).await??;
    ensure!(
        installed_package == package,
        "staged Discord package differs from the verified distribution"
    );
    let saved = store
        .migration_extension_value(
            EXTENSION_ID.into(),
            package.package_digest,
            SETTINGS_KEY.into(),
            None,
        )
        .await?;
    ensure!(
        saved == Some(settings(preference)?),
        "staged Discord privacy preferences differ"
    );
    let mut permissions = store
        .list_permissions(owner, EXTENSION_ID.into(), None, 8)
        .await?;
    let mut expected_grants = Vec::new();
    if preference.enabled {
        for (permission, resource_kind, resource_id) in [
            ("background.run", "Extension", EXTENSION_ID),
            (
                LOCAL_IPC_PERMISSION,
                LOCAL_IPC_RESOURCE_KIND,
                target_scope.as_str(),
            ),
            ("projects.read", "Global", "*"),
            (UNITY_SESSION_PERMISSION, "Global", "*"),
        ] {
            expected_grants.push(ExtensionPermissionGrant {
                permission: permission.into(),
                resource_kind: resource_kind.into(),
                resource_id: resource_id.into(),
            });
        }
    }
    permissions
        .grants
        .sort_by(|left, right| left.permission.cmp(&right.permission));
    expected_grants.sort_by(|left, right| left.permission.cmp(&right.permission));
    ensure!(
        permissions.next_cursor.is_none() && permissions.grants == expected_grants,
        "staged Discord grants differ from the consented activation"
    );
    Ok(())
}
