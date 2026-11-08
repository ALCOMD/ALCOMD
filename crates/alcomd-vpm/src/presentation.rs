use std::path::Path;

use alcomd_application::{
    CandidateLinks, DependencyIdentity, InstalledPackagePresentation, MAX_PRESENTATION_BYTES,
    MAX_PRESENTATION_PACKAGES, PresentationState, ProjectPackagePresentations,
    normalize_package_display_metadata as metadata,
};
use serde_json::Value;
use sha2::{Digest, Sha256};

fn text(manifest: &Value, key: &str, limit: usize) -> Option<String> {
    let value = manifest.get(key)?.as_str()?;
    (!value.is_empty()
        && value.len() <= limit
        && !value
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t'))
    .then(|| value.to_owned())
}

pub(crate) async fn inspect(
    root: &Path,
    locked: &[DependencyIdentity],
) -> ProjectPackagePresentations {
    let mut result = ProjectPackagePresentations {
        state: PresentationState::Complete,
        ..Default::default()
    };
    let mut total = 0;
    for package in locked {
        if result.packages.len() >= MAX_PRESENTATION_PACKAGES
            || !alcomd_application::valid_presentation_package_id(&package.package_id)
        {
            result.state = PresentationState::Partial;
            break;
        }
        let bytes = match crate::read_component(
            root,
            &["Packages", &package.package_id, "package.json"],
            crate::PROJECT_MANIFEST_LIMIT,
        )
        .await
        {
            Ok(Some(bytes)) => bytes,
            _ => {
                result.state = PresentationState::Partial;
                continue;
            }
        };
        total += bytes.len();
        if total > MAX_PRESENTATION_BYTES {
            result.state = PresentationState::Partial;
            break;
        }
        let value = match crate::parse_bounded_json(
            &bytes,
            alcomd_application::M3ErrorCode::ProjectManifestInvalid,
        ) {
            Ok(value) => value,
            Err(_) => {
                result.state = PresentationState::Partial;
                continue;
            }
        };
        if value.get("name").and_then(Value::as_str) != Some(&package.package_id)
            || value.get("version").and_then(Value::as_str) != Some(&package.value)
        {
            result.state = PresentationState::Partial;
            continue;
        }
        let Some(metadata) = metadata(&value) else {
            result.state = PresentationState::Partial;
            continue;
        };
        result.packages.push(InstalledPackagePresentation {
            package_id: package.package_id.clone(),
            version: package.value.clone(),
            display_name: text(&value, "displayName", 4_096),
            description: text(&value, "description", 65_536),
            yanked: metadata.yanked,
            metadata,
            links: CandidateLinks {
                documentation: crate::sanitized_package_link(value.get("documentationUrl")),
                changelog: crate::sanitized_package_link(value.get("changelogUrl")),
            },
            manifest_sha256: Sha256::digest(&bytes)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
        });
    }
    // Include status: a partial scan must not alias a previously complete one.
    let encoded = serde_json::to_vec(&(result.state, &result.packages))
        .expect("presentation primitives serialize");
    result.fingerprint = Sha256::digest(encoded)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn installed_metadata_matches_lock_and_missing_or_changed_manifests_are_partial() {
        let root = std::env::temp_dir().join(format!(
            "alcomd-presentation-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("presentation fixture or query succeeds")
                .as_nanos()
        ));
        let folder = root.join("Packages/com.example.package");
        tokio::fs::create_dir_all(&folder)
            .await
            .expect("presentation fixture or query succeeds");
        let manifest = br#"{"name":"com.example.package","version":"1.0.0","displayName":"Installed title","description":"Installed description","keywords":["needle"],"vrc-get":{"yanked":true},"documentationUrl":"https://example.invalid/docs","changelogUrl":"file:///secret"}"#;
        tokio::fs::write(folder.join("package.json"), manifest)
            .await
            .expect("presentation fixture or query succeeds");
        let mut locked = vec![DependencyIdentity {
            package_id: "com.example.package".into(),
            value: "1.0.0".into(),
        }];
        let observed = inspect(&root, &locked).await;
        assert_eq!(observed.state, PresentationState::Complete);
        assert_eq!(
            observed.packages[0].display_name.as_deref(),
            Some("Installed title")
        );
        assert!(observed.packages[0].yanked);
        assert_eq!(observed.packages[0].metadata.keywords, ["needle"]);
        assert!(observed.packages[0].links.changelog.is_none());
        locked[0].value = "2.0.0".into();
        let mismatch = inspect(&root, &locked).await;
        assert_eq!(mismatch.state, PresentationState::Partial);
        assert!(mismatch.packages.is_empty());
        assert_ne!(mismatch.fingerprint, observed.fingerprint);
        tokio::fs::remove_file(folder.join("package.json"))
            .await
            .expect("presentation fixture or query succeeds");
        assert_eq!(
            inspect(&root, &locked).await.state,
            PresentationState::Partial
        );
        locked[0].package_id = "../outside".into();
        assert!(inspect(&root, &locked).await.packages.is_empty());
        assert_eq!(inspect(&root, &[]).await.state, PresentationState::Complete);
        tokio::fs::remove_dir_all(root)
            .await
            .expect("presentation fixture or query succeeds");
    }
}
