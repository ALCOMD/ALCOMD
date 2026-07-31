use crate::commands::RustError;
use crate::templates::{self, ProjectTemplateInfo};
use serde::Serialize;
use vrc_get_vpm::environment::VccDatabaseConnection;
use vrc_get_vpm::io::DefaultEnvironmentIo;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectTemplateSummary {
    pub(crate) display_name: String,
    pub(crate) id: String,
    pub(crate) unity_versions: Vec<String>,
    pub(crate) update_date: Option<String>,
    pub(crate) has_unity_packages: bool,
    pub(crate) has_project_archive: bool,
    pub(crate) available: bool,
}

pub(crate) async fn load_project_templates(
    io: &DefaultEnvironmentIo,
) -> Result<Vec<ProjectTemplateInfo>, RustError> {
    let connection = VccDatabaseConnection::connect(io).await?;
    let unity_versions = connection
        .get_unity_installations()
        .iter()
        .filter_map(|unity| unity.version())
        .collect::<Vec<_>>();

    Ok(templates::load_resolve_all_templates(io, &unity_versions).await?)
}

pub(crate) fn project_template_summary(template: &ProjectTemplateInfo) -> ProjectTemplateSummary {
    let mut unity_versions = template.unity_versions.clone();
    unity_versions.sort_unstable_by(|left, right| right.cmp(left));
    unity_versions.dedup();
    let unity_versions = unity_versions
        .into_iter()
        .map(|version| version.to_string())
        .collect();

    ProjectTemplateSummary {
        display_name: template.display_name.clone(),
        id: template.id.clone(),
        unity_versions,
        update_date: template.update_date.map(|date| date.to_rfc3339()),
        has_unity_packages: template
            .alcom_template
            .as_ref()
            .is_some_and(|value| !value.unity_packages.is_empty()),
        has_project_archive: template
            .alcom_template
            .as_ref()
            .is_some_and(|value| value.is_project_archive()),
        available: template.available,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use vrc_get_vpm::version::UnityVersion;

    #[test]
    fn project_template_summary_is_discovery_focused() {
        let template = ProjectTemplateInfo {
            display_name: "Example".to_string(),
            id: "com.example.template".to_string(),
            unity_versions: vec![
                UnityVersion::new_f1(2022, 3, 6),
                UnityVersion::new_f1(2022, 3, 22),
                UnityVersion::new_f1(2022, 3, 6),
            ],
            update_date: None,
            alcom_template: None,
            source_path: Some("Templates/example.alcomtemplate".into()),
            available: true,
        };

        let summary = serde_json::to_value(project_template_summary(&template)).unwrap();

        assert_eq!(summary["displayName"], "Example");
        assert_eq!(summary["id"], "com.example.template");
        assert_eq!(
            summary["unityVersions"],
            serde_json::json!(["2022.3.22f1", "2022.3.6f1"])
        );
        assert_eq!(summary["available"], true);
        assert_eq!(summary["hasUnityPackages"], false);
        assert_eq!(summary["hasProjectArchive"], false);
        assert_eq!(summary.get("sourcePath"), None);
        assert_eq!(summary["updateDate"], Value::Null);
    }
}
