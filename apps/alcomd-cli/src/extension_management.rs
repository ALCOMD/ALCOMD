use crate::{CliError, ExecutionOptions, confirm_high_impact};
use alcomd_client::AlcomdClient;
use alcomd_protocol as rpc;
use clap::{Args, Subcommand, ValueEnum};
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Args)]
pub(crate) struct Page {
    #[arg(long)]
    cursor: Option<String>,
    #[arg(long)]
    limit: Option<u32>,
}

#[derive(Debug, Args)]
pub(crate) struct Lifecycle {
    extension_id: String,
    #[arg(long)]
    expected_revision: u64,
    #[arg(long)]
    idempotency_key: String,
}

#[derive(Debug, Args)]
pub(crate) struct Grant {
    extension_id: String,
    permission: String,
    #[arg(long)]
    resource_kind: String,
    #[arg(long)]
    resource_id: String,
    #[arg(long)]
    expected_grant_revision: u64,
    #[arg(long)]
    idempotency_key: String,
}

impl From<Grant> for rpc::ExtensionGrantParams {
    fn from(value: Grant) -> Self {
        Self {
            extension_id: value.extension_id,
            permission: value.permission,
            resource_kind: value.resource_kind,
            resource_id: value.resource_id,
            expected_grant_revision: value.expected_grant_revision,
            idempotency_key: value.idempotency_key,
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(crate) enum DataDisposition {
    RetainData,
    DeleteData,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// List installed extensions. Continue with the cursor from the previous page.
    List(Page),
    /// Read the installed identity, lifecycle status and signed declarations.
    Get { extension_id: String },
    /// Read available permissions and current grants.
    Permissions {
        extension_id: String,
        #[command(flatten)]
        page: Page,
    },
    /// Inspect an extension source and obtain its version list and fingerprint.
    InspectSource { url: String },
    /// Review a signed package and its installation plan.
    PlanInstall {
        package_path: String,
        #[arg(long, default_value_t = 0)]
        expected_revision: u64,
    },
    /// Review a selected source version against its inspected fingerprint.
    PlanInstallFromSource {
        url: String,
        version: String,
        #[arg(long)]
        expected_source_fingerprint: String,
    },
    /// Apply the reviewed plan with its exact consent fingerprint.
    ApplyInstall {
        plan_id: String,
        #[arg(long)]
        plan_fingerprint: String,
        #[arg(long)]
        accept_terms: bool,
        #[arg(long)]
        accept_required_permissions: bool,
        #[arg(long)]
        idempotency_key: String,
    },
    /// Enable the installed package at the reviewed revision.
    Enable {
        #[command(flatten)]
        target: Lifecycle,
        #[arg(long)]
        activation_package_digest: Option<String>,
    },
    /// Disable the installed package at the reviewed revision.
    Disable(Lifecycle),
    /// Review removal and choose what happens to extension-owned data.
    PlanUninstall {
        extension_id: String,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long, value_enum)]
        data_disposition: DataDisposition,
    },
    /// Apply the original reviewed removal plan.
    ApplyUninstall {
        plan_id: String,
        #[arg(long)]
        idempotency_key: String,
    },
    /// Grant the permission on the selected resource.
    Grant(Grant),
    /// Revoke the permission on the selected resource.
    Revoke(Grant),
}

impl Command {
    pub(super) fn name(&self) -> &'static str {
        match self {
            Self::List(_) => "extension list",
            Self::Get { .. } => "extension get",
            Self::Permissions { .. } => "extension permissions",
            Self::InspectSource { .. } => "extension inspect-source",
            Self::PlanInstall { .. } => "extension plan-install",
            Self::PlanInstallFromSource { .. } => "extension plan-install-from-source",
            Self::ApplyInstall { .. } => "extension apply-install",
            Self::Enable { .. } => "extension enable",
            Self::Disable(_) => "extension disable",
            Self::PlanUninstall { .. } => "extension plan-uninstall",
            Self::ApplyUninstall { .. } => "extension apply-uninstall",
            Self::Grant(_) => "extension grant",
            Self::Revoke(_) => "extension revoke",
        }
    }
    pub(super) fn supports_preview(&self) -> bool {
        matches!(
            self,
            Self::PlanInstall { .. }
                | Self::PlanInstallFromSource { .. }
                | Self::PlanUninstall { .. }
        )
    }
}

fn output<T: Serialize>(value: T) -> Result<Value, CliError> {
    serde_json::to_value(value)
        .map_err(|_| CliError::Client(alcomd_client::ClientError::InvalidResponse))
}

pub(super) async fn execute(
    client: &mut AlcomdClient,
    command: Command,
    options: ExecutionOptions,
) -> Result<Value, CliError> {
    use Command::*;
    match command {
        List(page) => output(
            client
                .extensions_list(rpc::ExtensionsListParams {
                    cursor: page.cursor,
                    limit: page.limit,
                })
                .await?,
        ),
        Get { extension_id } => output(client.extension_get(extension_id).await?),
        Permissions { extension_id, page } => output(
            client
                .extension_permissions_list(rpc::ExtensionPermissionsListParams {
                    extension_id,
                    cursor: page.cursor,
                    limit: page.limit,
                })
                .await?,
        ),
        InspectSource { url } => output(
            client
                .extension_inspect_source(rpc::ExtensionInspectSourceParams { url })
                .await?,
        ),
        PlanInstall {
            package_path,
            expected_revision,
        } => output(
            client
                .extension_plan_install(rpc::ExtensionPlanInstallParams {
                    package_path,
                    expected_revision,
                    source_kind: rpc::ExtensionSourceKind::LocalOwnerSelected,
                })
                .await?,
        ),
        PlanInstallFromSource {
            url,
            version,
            expected_source_fingerprint,
        } => output(
            client
                .extension_plan_install_from_source(rpc::ExtensionPlanInstallFromSourceParams {
                    url,
                    version,
                    expected_source_fingerprint,
                })
                .await?,
        ),
        ApplyInstall {
            plan_id,
            plan_fingerprint,
            accept_terms,
            accept_required_permissions,
            idempotency_key,
        } => {
            crate::confirm_frozen_plan(
                client,
                rpc::PlanKind::ExtensionInstall,
                &plan_id,
                None,
                options,
            )
            .await?;
            output(
                client
                    .extension_apply_install(rpc::ExtensionInstallApplyParams {
                        plan_id,
                        idempotency_key,
                        consent: rpc::ExtensionInstallConsent {
                            plan_fingerprint,
                            installation_terms_accepted: accept_terms,
                            required_permissions_accepted: accept_required_permissions,
                        },
                    })
                    .await?,
            )
        }
        Enable {
            target,
            activation_package_digest,
        } => {
            confirm_high_impact(options)?;
            output(
                client
                    .extension_enable_with_consent(rpc::ExtensionEnableParams {
                        extension_id: target.extension_id,
                        expected_revision: target.expected_revision,
                        idempotency_key: target.idempotency_key,
                        activation_package_digest,
                    })
                    .await?,
            )
        }
        Disable(target) => {
            confirm_high_impact(options)?;
            output(
                client
                    .extension_disable(rpc::ExtensionLifecycleParams {
                        extension_id: target.extension_id,
                        expected_revision: target.expected_revision,
                        idempotency_key: target.idempotency_key,
                    })
                    .await?,
            )
        }
        PlanUninstall {
            extension_id,
            expected_revision,
            data_disposition,
        } => output(
            client
                .extension_plan_uninstall(rpc::ExtensionPlanUninstallParams {
                    extension_id,
                    expected_revision,
                    data_disposition: match data_disposition {
                        DataDisposition::RetainData => rpc::ExtensionDataDisposition::RetainData,
                        DataDisposition::DeleteData => rpc::ExtensionDataDisposition::DeleteData,
                    },
                })
                .await?,
        ),
        ApplyUninstall {
            plan_id,
            idempotency_key,
        } => {
            crate::confirm_frozen_plan(
                client,
                rpc::PlanKind::ExtensionUninstall,
                &plan_id,
                None,
                options,
            )
            .await?;
            output(
                client
                    .extension_apply_uninstall(rpc::ExtensionApplyParams {
                        plan_id,
                        idempotency_key,
                    })
                    .await?,
            )
        }
        Grant(grant) => {
            confirm_high_impact(options)?;
            output(client.extension_set_grant(grant.into()).await?)
        }
        Revoke(grant) => {
            confirm_high_impact(options)?;
            output(client.extension_revoke_grant(grant.into()).await?)
        }
    }
}
