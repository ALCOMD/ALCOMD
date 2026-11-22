use super::{
    CliError, ExecutionOptions, absolute_path, apply_or_return_package_plan, confirm_high_impact,
    parse_cursor,
};
use alcomd_client::AlcomdClient;
use alcomd_protocol as rpc;
use clap::{Args, Subcommand};
use serde_json::Value;
use std::path::PathBuf;

fn parse_json<T: serde::de::DeserializeOwned>(input: &str) -> Result<T, String> {
    serde_json::from_str(input).map_err(|error| error.to_string())
}

#[derive(Debug, Args)]
pub(super) struct Candidates {
    project_id: String,
    #[arg(long)]
    expected_revision: u64,
    /// JSON summary or versions view. A summary can request bulk actions.
    #[arg(long, value_parser = parse_json::<rpc::CandidateView>)]
    view: rpc::CandidateView,
    #[arg(long)]
    limit: Option<u32>,
    #[arg(long)]
    expected_snapshot: Option<String>,
    /// The nextCursor object returned by Core.
    #[arg(long, value_parser = parse_cursor::<rpc::CandidateCursor>)]
    cursor: Option<rpc::CandidateCursor>,
}

impl From<Candidates> for rpc::ProjectCandidateRequest {
    fn from(query: Candidates) -> Self {
        Self {
            project_id: query.project_id,
            expected_revision: query.expected_revision,
            view: query.view,
            limit: query.limit,
            expected_snapshot: query.expected_snapshot,
            cursor: query.cursor,
        }
    }
}

#[derive(Debug, Args)]
pub(super) struct Bulk {
    project_id: String,
    #[arg(long)]
    expected_revision: u64,
    /// Repeat for each JSON install, upgrade, remove or reinstall intent, in request order.
    #[arg(long = "intent", required = true, value_parser = parse_json::<rpc::PackageBulkIntent>)]
    intents: Vec<rpc::PackageBulkIntent>,
    #[arg(long, required_unless_present = "dry_run")]
    idempotency_key: Option<String>,
}

impl Bulk {
    pub(super) async fn execute(
        self,
        client: &mut AlcomdClient,
        options: ExecutionOptions,
    ) -> Result<Value, CliError> {
        let plan = client
            .package_plan_bulk(rpc::PackagePlanBulkParams {
                project_id: self.project_id,
                expected_revision: self.expected_revision,
                intents: self.intents,
            })
            .await?;
        apply_or_return_package_plan(
            client,
            plan,
            self.expected_revision,
            self.idempotency_key,
            options,
        )
        .await
    }
}

#[derive(Debug, Args)]
pub(super) struct Reinstall {
    project_id: String,
    #[arg(
        long,
        conflicts_with = "package_ids",
        required_unless_present = "package_ids"
    )]
    all: bool,
    #[arg(long = "package-id")]
    package_ids: Vec<String>,
    /// Repeat for each explicit JSON packageId/source override.
    #[arg(long = "source", value_parser = parse_json::<rpc::PackageReinstallSource>)]
    sources: Vec<rpc::PackageReinstallSource>,
    #[arg(long)]
    expected_revision: u64,
    #[arg(long, required_unless_present = "dry_run")]
    idempotency_key: Option<String>,
}

impl Reinstall {
    pub(super) async fn execute(
        self,
        client: &mut AlcomdClient,
        options: ExecutionOptions,
    ) -> Result<Value, CliError> {
        let selection = if self.all {
            rpc::PackageReinstallSelection::All
        } else {
            rpc::PackageReinstallSelection::Packages {
                package_ids: self.package_ids,
            }
        };
        let plan = client
            .package_plan_reinstall(rpc::PackagePlanReinstallParams {
                project_id: self.project_id,
                expected_revision: self.expected_revision,
                selection,
                sources: self.sources,
            })
            .await?;
        apply_or_return_package_plan(
            client,
            plan,
            self.expected_revision,
            self.idempotency_key,
            options,
        )
        .await
    }
}

#[derive(Debug, Args)]
pub(super) struct Mutation {
    user_package_id: String,
    #[arg(long)]
    expected_revision: u64,
    #[arg(long)]
    idempotency_key: String,
}

impl From<Mutation> for rpc::UserPackageMutationParams {
    fn from(params: Mutation) -> Self {
        Self {
            user_package_id: params.user_package_id,
            expected_revision: params.expected_revision,
            idempotency_key: params.idempotency_key,
        }
    }
}

#[derive(Debug, Subcommand)]
pub(super) enum UserPackage {
    List {
        #[arg(long, value_parser = parse_cursor::<rpc::UserPackageCursor>)]
        cursor: Option<rpc::UserPackageCursor>,
        #[arg(long)]
        limit: Option<u32>,
    },
    Get {
        user_package_id: String,
    },
    Enroll {
        source_path: PathBuf,
        #[arg(long)]
        idempotency_key: String,
    },
    Refresh(Mutation),
    /// Remove the registration and keep the package directory.
    Remove(Mutation),
}

impl UserPackage {
    pub(super) fn name(&self) -> &'static str {
        match self {
            Self::List { .. } => "package user-package list",
            Self::Get { .. } => "package user-package get",
            Self::Enroll { .. } => "package user-package enroll",
            Self::Refresh(_) => "package user-package refresh",
            Self::Remove(_) => "package user-package remove",
        }
    }

    pub(super) async fn execute(
        self,
        client: &mut AlcomdClient,
        options: ExecutionOptions,
    ) -> Result<Value, CliError> {
        let result = match self {
            Self::List { cursor, limit } => serde_json::to_value(
                client
                    .user_packages_list(rpc::UserPackagesListParams { cursor, limit })
                    .await?,
            ),
            Self::Get { user_package_id } => serde_json::to_value(
                client
                    .user_package_get(rpc::UserPackageGetParams { user_package_id })
                    .await?,
            ),
            Self::Enroll {
                source_path,
                idempotency_key,
            } => serde_json::to_value(
                client
                    .user_package_enroll(rpc::UserPackageEnrollParams {
                        source_path: absolute_path(source_path).map_err(CliError::LocalIo)?,
                        idempotency_key,
                    })
                    .await?,
            ),
            Self::Refresh(params) => {
                serde_json::to_value(client.user_package_refresh(params.into()).await?)
            }
            Self::Remove(params) => {
                confirm_high_impact(options)?;
                serde_json::to_value(client.user_package_remove(params.into()).await?)
            }
        };
        result.map_err(|_| CliError::Client(alcomd_client::ClientError::InvalidResponse))
    }
}
