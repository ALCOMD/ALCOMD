use alcomd_application::{AccessContext, PlanId};
use alcomd_protocol::{self as rpc, PlanKind, PlansGetResult};

use super::{
    Applications, ConnectionState, DispatchAction, invalid, m4_rpc, m7_copy_rpc, m7_delete_rpc,
    m7_unity_migration_rpc, require_capability, success_action,
};

pub(super) async fn get(
    request: rpc::RequestEnvelope,
    state: &ConnectionState,
    applications: &Applications,
    access: &AccessContext,
) -> DispatchAction {
    let id = request.id;
    let Ok(params) = serde_json::from_value::<rpc::PlansGetParams>(request.params) else {
        return invalid(id);
    };
    let capability = match params.kind {
        PlanKind::ExtensionInstall | PlanKind::ExtensionUninstall => {
            rpc::CAPABILITY_EXTENSIONS_LIFECYCLE_V1
        }
        PlanKind::Package => rpc::CAPABILITY_PACKAGES_PLAN_V1,
        PlanKind::ProjectCopy => rpc::CAPABILITY_PROJECTS_COPY_V1,
        PlanKind::ProjectDeleteDirectory => rpc::CAPABILITY_PROJECTS_DELETE_V1,
        PlanKind::ProjectUnityMigration => rpc::CAPABILITY_PROJECTS_UNITY_MIGRATION_V1,
    };
    if let Some(action) = require_capability(&id, state, capability) {
        return action;
    }
    let Ok(plan_id) = PlanId::parse(&params.plan_id) else {
        return invalid(id);
    };
    let result = match params.kind {
        PlanKind::ExtensionInstall | PlanKind::ExtensionUninstall => {
            let install = params.kind == PlanKind::ExtensionInstall;
            let action = if install { "install" } else { "uninstall" };
            let plan = match applications.m6.get_plan(access, plan_id, action).await {
                Ok(plan) => Box::new(super::m6_rpc::plan(plan)),
                Err(error) => return super::m6_rpc::extension_error(id, error),
            };
            if install {
                PlansGetResult::ExtensionInstall { plan }
            } else {
                PlansGetResult::ExtensionUninstall { plan }
            }
        }
        PlanKind::Package => match applications.m4.get_plan(access, plan_id).await {
            Ok(plan) => PlansGetResult::Package {
                plan: Box::new(m4_rpc::plan_to_rpc(plan)),
            },
            Err(error) => return m4_rpc::m4_error(id, error),
        },
        PlanKind::ProjectCopy => match applications.project_copy.get_plan(access, plan_id).await {
            Ok(plan) => PlansGetResult::ProjectCopy {
                plan: Box::new(m7_copy_rpc::plan(plan)),
            },
            Err(error) => return m7_copy_rpc::copy_error(id, error),
        },
        PlanKind::ProjectDeleteDirectory => {
            match applications.project_delete.get_plan(access, plan_id).await {
                Ok(plan) => PlansGetResult::ProjectDeleteDirectory {
                    plan: Box::new(m7_delete_rpc::plan(plan)),
                },
                Err(error) => return m7_delete_rpc::delete_error(id, error),
            }
        }
        PlanKind::ProjectUnityMigration => {
            match applications.unity_migration.get_plan(access, plan_id).await {
                Ok(plan) => PlansGetResult::ProjectUnityMigration {
                    plan: Box::new(m7_unity_migration_rpc::migration_plan(plan)),
                },
                Err(error) => return m7_unity_migration_rpc::migration_error(id, error),
            }
        }
    };
    success_action(id, result, None)
}
