use alcomd_client::AlcomdClient;
use alcomd_protocol as rpc;
use rmcp::{
    ErrorData,
    model::{CallToolRequestParams, CallToolResult},
};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Map, Value};

use super::{Failure, canonical_id, failure, invalid, read, revision, snake, success, write};

pub(super) fn recognized(name: &str) -> bool {
    matches!(
        name,
        "alcomd_list_package_sources"
            | "alcomd_search_packages"
            | "alcomd_get_package"
            | "alcomd_plan_package_install"
            | "alcomd_plan_package_reinstall"
    )
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Source {
    Repository { repository_id: String },
    UserPackage { user_package_id: String },
}

enum Request {
    Query(rpc::McpPackageQuery),
    Plan(rpc::McpPackagePlanRequest),
}

struct Args(Map<String, Value>);

impl Args {
    fn optional<T: DeserializeOwned>(&mut self, name: &str) -> Result<Option<T>, ErrorData> {
        self.0
            .remove(name)
            .map(|value| serde_json::from_value(value).map_err(|_| invalid()))
            .transpose()
    }

    fn required<T: DeserializeOwned>(&mut self, name: &str) -> Result<T, ErrorData> {
        self.optional(name)?.ok_or_else(invalid)
    }

    fn package_id(&mut self) -> Result<String, ErrorData> {
        let id: String = self.required("package_id")?;
        if id.len() > 128
            || !id.as_bytes().first().is_some_and(u8::is_ascii_alphanumeric)
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
        {
            return Err(invalid());
        }
        Ok(id)
    }

    fn project(&mut self) -> Result<Option<rpc::McpPackageProject>, ErrorData> {
        match (
            self.optional::<String>("project_id")?,
            self.optional::<u64>("expected_revision")?,
        ) {
            (Some(project_id), Some(expected_revision))
                if canonical_id(&project_id) && revision(expected_revision) =>
            {
                Ok(Some(rpc::McpPackageProject {
                    project_id,
                    expected_revision,
                }))
            }
            (None, None) => Ok(None),
            _ => Err(invalid()),
        }
    }

    fn source(&mut self) -> Result<Option<rpc::PackageSourceSelector>, ErrorData> {
        self.optional("source")?
            .map(|source| match source {
                Source::Repository { repository_id } if canonical_id(&repository_id) => {
                    Ok(rpc::PackageSourceSelector::Repository { repository_id })
                }
                Source::UserPackage { user_package_id } if canonical_id(&user_package_id) => {
                    Ok(rpc::PackageSourceSelector::UserPackage { user_package_id })
                }
                _ => Err(invalid()),
            })
            .transpose()
    }

    fn query(&mut self, view: rpc::McpPackageView) -> Result<Request, ErrorData> {
        let project = if matches!(view, rpc::McpPackageView::Sources) {
            None
        } else {
            self.project()?
        };
        let limit: Option<u32> = self.optional("limit")?;
        let cursor: Option<String> = self.optional("cursor")?;
        if limit.is_some_and(|limit| !(1..=200).contains(&limit))
            || cursor.as_ref().is_some_and(|cursor| cursor.len() > 80)
        {
            return Err(invalid());
        }
        Ok(Request::Query(rpc::McpPackageQuery {
            view,
            project,
            cursor,
            limit,
        }))
    }
}

fn parse(params: CallToolRequestParams) -> Result<Request, ErrorData> {
    if params.input_responses.is_some() || params.request_state.is_some() {
        return Err(invalid());
    }
    let mut args = Args(params.arguments.unwrap_or_default());
    if args.0.values().any(Value::is_null) {
        return Err(invalid());
    }
    let request = match params.name.as_ref() {
        "alcomd_list_package_sources" => args.query(rpc::McpPackageView::Sources)?,
        "alcomd_search_packages" => {
            let query: Option<String> = args.optional("query")?;
            let repository_id: Option<String> = args.optional("repository_id")?;
            if query
                .as_ref()
                .is_some_and(|query| query.len() > 256 || query.chars().any(char::is_control))
                || repository_id.as_ref().is_some_and(|id| !canonical_id(id))
            {
                return Err(invalid());
            }
            args.query(rpc::McpPackageView::Summary {
                query,
                repository_id,
            })?
        }
        "alcomd_get_package" => {
            let package_id = args.package_id()?;
            let version = args.optional("version")?;
            let source = args.source()?;
            args.query(rpc::McpPackageView::Versions {
                package_id,
                version,
                source,
            })?
        }
        "alcomd_plan_package_install" | "alcomd_plan_package_reinstall" => {
            let intent = if params.name == "alcomd_plan_package_install" {
                rpc::McpPackageIntent::Install {
                    version_selector: args.required("version_selector")?,
                }
            } else {
                rpc::McpPackageIntent::Reinstall
            };
            Request::Plan(rpc::McpPackagePlanRequest {
                intent,
                project: args.project()?.ok_or_else(invalid)?,
                package_id: args.package_id()?,
                source: args.source()?,
            })
        }
        _ => return Err(invalid()),
    };
    if !args.0.is_empty() {
        return Err(invalid());
    }
    Ok(request)
}

pub(super) async fn execute(
    client: &mut AlcomdClient,
    params: CallToolRequestParams,
) -> Result<CallToolResult, ErrorData> {
    let request = parse(params)?;
    let result = match request {
        Request::Query(query) => client.mcp_package_query(query).await.map_err(read).and_then(|page| {
            let mut data = serde_json::to_value(&page).map_err(|_| Failure::InvalidCore)?;
            let mut dependencies = Vec::new();
            if let Some(rows) = data["items"].as_array_mut() {
                for (index, row) in rows.iter_mut().enumerate() {
                    if let Some(value) = row.as_object_mut().and_then(|object| object.remove("dependencies")) {
                        dependencies.push((index, value));
                    }
                }
            }
            let mut data = snake(data);
            for (index, value) in dependencies {
                data["items"][index]["dependencies"] = value;
            }
            Ok(success(data, "Authorized package catalog results.".into(), false))
        }),
        Request::Plan(plan) => client.mcp_package_plan(plan).await.map_err(write).and_then(|response| {
            if response.plan.change_set.mutations.iter().any(|mutation| mutation.source.is_some()) {
                return Err(Failure::InvalidCore);
            }
            let message = format!("Review package plan {} and confirm before applying. The project has not been changed.", response.plan.plan_id);
            let data = serde_json::to_value(response).map_err(|_| Failure::InvalidCore)?;
            Ok(success(snake(data), message, true))
        }),
    };
    Ok(result.unwrap_or_else(failure))
}
