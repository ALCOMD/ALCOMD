//! Explicit rejected execution attempts; queries and previews never write activity.
use super::{DispatchAction, M7OfficialGuiApplication};
use alcomd_application as app;
use alcomd_protocol as rpc;
use serde_json::{Value, json};
pub(super) struct Attempt {
    action: &'static str,
    permission: app::Permission,
    subject: app::OfficialActivitySubject,
    parameters: Value,
}
pub(super) fn prepare(request: &rpc::RequestEnvelope) -> Option<Attempt> {
    use app::Permission as P;
    let (action, permission, kind) = match request.method.as_str() {
        "projects.register" => ("project.add", P::ProjectsManage, "project"),
        "projects.refresh" => ("project.refresh", P::ProjectsManage, "project"),
        "projects.unregister" => ("project.remove", P::ProjectsManage, "project"),
        "projects.setFavorite" => ("project.favorite", P::ProjectsManage, "project"),
        "repositories.register" => ("repository.add", P::RepositoriesManage, "repository"),
        "repositories.refresh" => ("repository.fetch", P::RepositoriesManage, "repository"),
        "repositories.unregister" => ("repository.remove", P::RepositoriesManage, "repository"),
        "unity.installations.register" => (
            "unity.installation.add",
            P::UnityManage,
            "unity-installation",
        ),
        "unity.installations.remove" => (
            "unity.installation.remove",
            P::UnityManage,
            "unity-installation",
        ),
        "unity.launch" => ("unity.launch", P::UnityLaunch, "project"),
        "unity.projectLaunchConfig.set" | "unity.projectLaunchConfig.clear" => {
            ("unity.launch-config.change", P::UnityManage, "project")
        }
        "extensions.setGrant" | "extensions.revokeGrant" => (
            "extensions.permission.change",
            P::ExtensionsPermissionsManage,
            "extension",
        ),
        "packages.applyPlan" => ("packages.apply", P::PackagesManage, "project"),
        "projects.applyCopy" => ("projects.copy", P::ProjectsCreate, "project"),
        "projects.applyDeleteDirectory" => {
            ("projects.delete-directory", P::ProjectsDelete, "project")
        }
        "projects.applyUnityMigration" => (
            "projects.unity-migration",
            P::ProjectsUnityMigrate,
            "project",
        ),
        "backups.create" => ("backups.create", P::BackupsManage, "backup"),
        "backups.applyRestore" => ("backups.restore", P::ProjectsCreate, "backup"),
        "templates.applyImport" => ("templates.import", P::TemplatesManage, "template"),
        "templates.applyDerive" => ("templates.derive", P::TemplatesManage, "template"),
        "templates.applyCreateProject" => {
            ("templates.create-project", P::ProjectsCreate, "template")
        }
        "templates.remove" => ("template.remove", P::TemplatesManage, "template"),
        "templates.setFavorite" => ("template.favorite", P::TemplatesManage, "template"),
        "packages.userPackages.enroll" => ("user-package.add", P::PackagesManage, "user-package"),
        "packages.userPackages.refresh" => {
            ("user-package.refresh", P::PackagesManage, "user-package")
        }
        "packages.userPackages.remove" => {
            ("user-package.remove", P::PackagesManage, "user-package")
        }
        "extensions.applyInstall" => ("extensions.install", P::ExtensionsManage, "extension"),
        "extensions.applyUninstall" => ("extensions.uninstall", P::ExtensionsManage, "extension"),
        "extensions.enable" => ("extensions.enable", P::ExtensionsManage, "extension"),
        "extensions.disable" => ("extensions.disable", P::ExtensionsManage, "extension"),
        _ => return None,
    };
    let id = [
        "projectId",
        "repositoryId",
        "installationId",
        "templateId",
        "userPackageId",
        "extensionId",
        "backupId",
    ]
    .into_iter()
    .find_map(|k| request.params.get(k).and_then(Value::as_str))
    .map(str::to_owned);
    let name = request
        .params
        .get("path")
        .and_then(Value::as_str)
        .and_then(|p| std::path::Path::new(p).file_name())
        .and_then(|p| p.to_str())
        .map(str::to_owned);
    let mut parameters = serde_json::Map::new();
    for k in ["planId", "favorite"] {
        if let Some(v) = request.params.get(k) {
            parameters.insert(k.into(), v.clone());
        }
    }
    Some(Attempt {
        action,
        permission,
        subject: app::OfficialActivitySubject {
            kind: kind.into(),
            id,
            name,
        },
        parameters: Value::Object(parameters),
    })
}
pub(super) async fn record_rejection(
    attempt: Option<Attempt>,
    action: &mut DispatchAction,
    application: &M7OfficialGuiApplication,
    access: &app::AccessContext,
) {
    let Some(attempt) = attempt else {
        return;
    };
    let Some(error) = action.response.get("error") else {
        return;
    };
    let Some(code) = error.get("code").and_then(Value::as_str) else {
        return;
    };
    // These do not establish an authorized, unaccepted execution failure. In
    // particular an accepted Unity launch already owns its activity and result.
    if matches!(
        code,
        "invalid_request"
            | "invalid_params"
            | "permission_denied"
            | "method_not_found"
            | "capability_required"
            | "capability_not_negotiated"
            | "unsupported_capability"
            | "store_unavailable"
            | "internal_error"
            | "unity_launch_failed"
    ) || code.contains("recovery")
        || code.ends_with("permission_denied")
        || code.ends_with("scope_denied")
    {
        return;
    }
    if access.require(attempt.permission).is_err() {
        return;
    }
    let diagnostic_id = error
        .get("diagnosticId")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let activity = app::NewFailedActivity {
        activity_id: app::OperationId::new().to_string(),
        action: attempt.action.into(),
        subject: attempt.subject,
        parameters: attempt.parameters,
        diagnostic_id,
        occurred_at_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|t| t.as_millis().min(9_007_199_254_740_991) as u64)
            .unwrap_or(0),
        error_code: code.into(),
    };
    if application
        .record_failed_activity(access, attempt.permission, activity)
        .await
        .is_err()
        && let Some(error) = action
            .response
            .get_mut("error")
            .and_then(Value::as_object_mut)
    {
        let data = error.entry("data").or_insert_with(|| json!({}));
        if let Some(data) = data.as_object_mut() {
            data.insert("activityRecordingUnavailable".into(), json!(true));
        }
    }
}
