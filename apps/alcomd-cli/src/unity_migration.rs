use super::{
    CliError, ExecutionOptions, OutputMode, confirm_high_impact, show_copy_plan,
    show_unity_migration_plan, wait_for_operation,
};
use alcomd_client::{AlcomdClient, ClientConfig, ClientError};
use alcomd_protocol as rpc;
use serde_json::{Value, json};
use std::path::Path;

pub(super) async fn run(
    client: &mut AlcomdClient,
    config: &ClientConfig,
    input: rpc::ProjectsPlanUnityMigrationParams,
    in_place: bool,
    copy_name: Option<String>,
    apply_key: Option<String>,
    options: ExecutionOptions,
) -> Result<Value, CliError> {
    let revision = input.expected_project_revision;
    let reviewed = client.project_plan_unity_migration(input).await?;
    let rpc::ProjectsPlanUnityMigrationResult::Planned { plan, .. } = &reviewed else {
        return Ok(json!(reviewed));
    };
    if plan.target_sdk_version.is_none() {
        if copy_name.is_some() {
            return Err(CliError::Usage("--copy-name requires a Unity 6 migration"));
        }
        return finish(client, config, reviewed, apply_key, options).await;
    }

    let source = client.project_get(plan.project_id.clone()).await?.project;
    if in_place {
        if options.dry_run {
            let mut preview = json!(reviewed);
            preview["mode"] = json!("in_place");
            preview["sourcePath"] = json!(source.root_path);
            preview["destinationPath"] = json!(source.root_path);
            return Ok(preview);
        }
        if options.output_mode == OutputMode::Human {
            eprintln!(
                "In-place upgrade of {}. Back up the project before continuing.",
                source.root_path
            );
        }
        return finish(client, config, reviewed, apply_key, options).await;
    }

    let root = Path::new(&source.root_path);
    let parent = root
        .parent()
        .and_then(Path::to_str)
        .ok_or(CliError::Usage("Missing source parent directory"))?;
    let leaf = root
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or(CliError::Usage("Missing source project name"))?;
    let copy = client
        .project_plan_copy(rpc::ProjectsPlanCopyParams {
            source_project_id: plan.project_id.clone(),
            expected_revision: revision,
            target_parent_path: parent.to_owned(),
            target_leaf: copy_name.unwrap_or_else(|| format!("{leaf}-Unity6")),
            idempotency_key: plan.plan_id.clone(),
        })
        .await?
        .plan;
    if options.dry_run {
        let destination =
            Path::new(&copy.target_parent_canonical_path).join(&copy.normalized_target_leaf);
        return Ok(json!({ "mode": "copy", "sourcePath": source.root_path,
            "destinationPath": destination, "sourcePlan": plan, "copyPlan": copy }));
    }
    if options.output_mode == OutputMode::Human {
        show_copy_plan(&copy);
        show_unity_migration_plan(plan);
    }
    confirm_high_impact(options)?;
    let receipt = client
        .project_apply_copy(rpc::ProjectsApplyCopyParams {
            expected_revision: copy.source_project_revision,
            idempotency_key: copy.plan_id.clone(),
            plan_id: copy.plan_id,
        })
        .await?;
    follow(
        client,
        config,
        receipt.operation_id.clone(),
        options.output_mode,
    )
    .await?;
    if options.output_mode == OutputMode::Human {
        eprintln!(
            "Copy {} completed for project {}.",
            receipt.operation_id, receipt.target_project_id
        );
    }
    let result = async {
        let target = client.project_get(receipt.target_project_id.clone()).await?.project;
        if options.output_mode == OutputMode::Human {
            eprintln!("Migration destination: {}", target.root_path);
        }
        let reviewed = client.project_plan_unity_migration(rpc::ProjectsPlanUnityMigrationParams {
            project_id: receipt.target_project_id.clone(),
            expected_project_revision: target.revision.ok_or(ClientError::InvalidResponse)?,
            target_installation_id: plan.target_installation_id.clone(),
            idempotency_key: receipt.operation_id.clone(),
        }).await?;
        let migration = finish(client, config, reviewed, apply_key, options).await?;
        Ok(json!({
            "copy": { "operationId": receipt.operation_id, "targetProjectId": receipt.target_project_id,
                "destinationPath": target.root_path },
            "migration": migration
        }))
    }.await;
    result.map_err(|error| copy_completed(error, &receipt, options.output_mode))
}

pub(super) async fn apply_reviewed(
    client: &mut AlcomdClient,
    config: &ClientConfig,
    plan_id: String,
    in_place: bool,
    copy_operation_id: Option<String>,
    key: String,
    options: ExecutionOptions,
) -> Result<Value, CliError> {
    let rpc::PlansGetResult::ProjectUnityMigration { plan } = client
        .get_plan(rpc::PlansGetParams {
            kind: rpc::PlanKind::ProjectUnityMigration,
            plan_id,
        })
        .await?
    else {
        return Err(ClientError::InvalidResponse.into());
    };
    if plan.target_sdk_version.is_some() {
        if !in_place {
            let copy_id = copy_operation_id.ok_or(CliError::Usage(
                "For Unity 6, specify --copy-operation-id for a completed copy or --in-place for the original.",
            ))?;
            let copy = client.operation_get(copy_id).await?;
            if copy.kind != "projects.copy"
                || copy.state != rpc::OperationState::Succeeded
                || copy
                    .result
                    .as_ref()
                    .and_then(|r| r.get("targetProjectId"))
                    .and_then(Value::as_str)
                    != Some(&plan.project_id)
            {
                return Err(CliError::Usage(
                    "The completed copy must identify the reviewed project.",
                ));
            }
        }
        if options.output_mode == OutputMode::Human {
            let target = client.project_get(plan.project_id.clone()).await?.project;
            eprintln!("Migration destination: {}", target.root_path);
            if in_place {
                eprintln!("Back up the original project before continuing.");
            }
        }
    }
    let reviewed = rpc::ProjectsPlanUnityMigrationResult::Planned {
        plan: *plan,
        replayed: false,
    };
    finish(client, config, reviewed, Some(key), options).await
}

async fn finish(
    client: &mut AlcomdClient,
    config: &ClientConfig,
    reviewed: rpc::ProjectsPlanUnityMigrationResult,
    key: Option<String>,
    options: ExecutionOptions,
) -> Result<Value, CliError> {
    let rpc::ProjectsPlanUnityMigrationResult::Planned { plan, .. } = &reviewed else {
        return Ok(json!(reviewed));
    };
    if options.dry_run {
        return Ok(json!(reviewed));
    }
    if options.output_mode == OutputMode::Human {
        show_unity_migration_plan(plan);
    }
    confirm_high_impact(options)?;
    let accepted = client
        .project_apply_unity_migration(rpc::ProjectsApplyUnityMigrationParams {
            plan_id: plan.plan_id.clone(),
            idempotency_key: key.ok_or(CliError::Usage("Apply requires --idempotency-key"))?,
        })
        .await?;
    if options.no_wait {
        return Ok(json!(accepted));
    }
    let completed = follow(client, config, accepted.operation_id, options.output_mode).await?;
    if plan.target_sdk_version.is_some() && options.output_mode == OutputMode::Human {
        eprintln!(
            "Unity migration completed. Open the project and finish the VRChat SDK Upgrade Wizard."
        );
    }
    Ok(json!(completed))
}

// Recover observation of the accepted operation without resending its Apply request.
async fn follow(
    client: &mut AlcomdClient,
    config: &ClientConfig,
    operation_id: String,
    mode: OutputMode,
) -> Result<rpc::Operation, CliError> {
    for attempt in 0..3 {
        if attempt > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            match AlcomdClient::connect(config.clone().without_daemon_start()).await {
                Ok(reconnected) => *client = reconnected,
                Err(error) if error.is_retryable() => continue,
                Err(error) => return Err(error.into()),
            }
        }
        match wait_for_operation(client, operation_id.clone(), mode).await {
            Err(CliError::Client(error)) if error.is_retryable() => continue,
            result => return result,
        }
    }
    Err(ClientError::Remote(rpc::RpcError {
        code: "daemon_unavailable".into(),
        message: format!("The accepted operation could not be observed. Read it with operation get {operation_id}."),
        diagnostic_id: None,
        data: None,
    }).into())
}

fn copy_completed(
    error: CliError,
    copy: &rpc::ProjectsApplyCopyResult,
    mode: OutputMode,
) -> CliError {
    if mode != OutputMode::Json {
        return error;
    }
    let mut remote = match error {
        CliError::Client(ClientError::Remote(remote)) => remote,
        CliError::OperationFailed(op) => rpc::RpcError {
            code: op
                .error_code
                .unwrap_or_else(|| "operation_cancelled".into()),
            message: format!("Migration operation {} did not succeed.", op.operation_id),
            diagnostic_id: op.diagnostic_id,
            data: None,
        },
        other => return other,
    };
    remote.message.push_str(&format!(
        " Copy {} completed. The retained project ID is {}.",
        copy.operation_id, copy.target_project_id
    ));
    CliError::Client(ClientError::Remote(remote))
}
