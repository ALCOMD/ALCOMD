//! Private Node SDK subprocess protocol.

use std::collections::BTreeMap;
use std::io;
use std::time::Duration;

use alcomd_protocol::{EventNotification, EventsSubscribeParams, MAX_REQUEST_ID_BYTES};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt, Stdin, Stdout};

use crate::{AlcomdClient, ClientConfig, ClientError, EventSubscription};

const CATALOG: &str = include_str!("../../../specs/rpc/native-sdk-v1.methods.json");
const FRAME_DEADLINE: Duration = Duration::from_secs(10);

#[derive(Deserialize)]
struct Catalog {
    methods: BTreeMap<String, Method>,
}

#[derive(Deserialize)]
struct Method {
    kind: MethodKind,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum MethodKind {
    Read,
    Write,
    Stream,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    id: String,
    method: String,
    params: Value,
}

/// Serves the bundled SDK helper. This interface is not an RPC listening endpoint.
pub async fn run() -> io::Result<()> {
    let catalog: Catalog = serde_json::from_str(CATALOG).map_err(io::Error::other)?;
    let mut input = tokio::io::stdin();
    let mut output = tokio::io::stdout();
    let Some(request) = read_request(&mut input).await? else {
        return Ok(());
    };
    if request.method != "connect" || request.params.as_object().is_none_or(|p| !p.is_empty()) {
        return invalid_request(&mut output, &request.id).await;
    }
    let mut client = match AlcomdClient::connect(
        ClientConfig::for_installed_core().with_client_name("alcomd-sdk"),
    )
    .await
    {
        Ok(client) => client,
        Err(error) => return client_error(&mut output, &request.id, error, false).await,
    };
    write_frame(
        &mut output,
        &json!({"id": request.id, "result": client.hello_result()}),
    )
    .await?;

    while let Some(request) = read_request(&mut input).await? {
        let Some(method) = catalog.methods.get(&request.method) else {
            invalid_request(&mut output, &request.id).await?;
            continue;
        };
        match method.kind {
            MethodKind::Read | MethodKind::Write => {
                match client
                    .call::<_, Value>(&request.method, request.params)
                    .await
                {
                    Ok(result) => {
                        write_frame(&mut output, &json!({"id": request.id, "result": result}))
                            .await?;
                    }
                    Err(error) => {
                        let unknown = matches!(method.kind, MethodKind::Write)
                            && !matches!(error, ClientError::Remote(_));
                        client_error(&mut output, &request.id, error, unknown).await?;
                        if !client.is_usable() {
                            return Ok(());
                        }
                    }
                }
            }
            MethodKind::Stream => {
                return serve_events(&mut input, &mut output, client, request).await;
            }
        }
    }
    Ok(())
}

async fn serve_events(
    input: &mut Stdin,
    output: &mut Stdout,
    client: AlcomdClient,
    request: Request,
) -> io::Result<()> {
    let params: EventsSubscribeParams = match serde_json::from_value(request.params) {
        Ok(params) => params,
        Err(_) => return invalid_request(output, &request.id).await,
    };
    let mut stream = match EventSubscription::from_client(client, params).await {
        Ok(stream) => stream,
        Err(error) => return client_error(output, &request.id, error, false).await,
    };
    write_frame(output, &json!({"id": request.id, "result": stream.ready()})).await?;
    loop {
        let mut stop = [0];
        let received = tokio::select! {
            _ = input.read(&mut stop) => return Ok(()),
            notification = stream.next_notification() => notification,
        };
        let notification = received.map_err(io::Error::other)?;
        write_frame(output, &notification).await?;
        if matches!(notification, EventNotification::Closed(_)) {
            return Ok(());
        }
    }
}

async fn read_request(input: &mut Stdin) -> io::Result<Option<Request>> {
    let mut length = [0; 4];
    if input.read(&mut length[..1]).await? == 0 {
        return Ok(None);
    }
    tokio::time::timeout(FRAME_DEADLINE, async {
        input.read_exact(&mut length[1..]).await?;
        let size = alcomd_protocol::decode_frame_length(length).map_err(|_| invalid_frame())?;
        let mut bytes = vec![0; size];
        input.read_exact(&mut bytes).await?;
        let request: Request = serde_json::from_slice(&bytes).map_err(|_| invalid_frame())?;
        if request.id.is_empty()
            || request.id.len() > MAX_REQUEST_ID_BYTES
            || !request.params.is_object()
        {
            return Err(invalid_frame());
        }
        Ok(Some(request))
    })
    .await
    .unwrap_or_else(|_| Err(io::ErrorKind::TimedOut.into()))
}

async fn write_frame(output: &mut Stdout, value: &impl Serialize) -> io::Result<()> {
    let bytes = serde_json::to_vec(value).map_err(|_| invalid_frame())?;
    let frame = alcomd_protocol::encode_frame(&bytes).map_err(|_| invalid_frame())?;
    tokio::time::timeout(FRAME_DEADLINE, async {
        output.write_all(&frame).await?;
        output.flush().await
    })
    .await
    .unwrap_or_else(|_| Err(io::ErrorKind::TimedOut.into()))
}

fn invalid_frame() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "invalid SDK helper frame")
}

async fn invalid_request(output: &mut Stdout, id: &str) -> io::Result<()> {
    let error = json!({"kind": "invalid_request", "message": "Unsupported SDK request"});
    write_frame(output, &json!({"id": id, "error": error})).await
}

async fn client_error(
    output: &mut Stdout,
    id: &str,
    error: ClientError,
    outcome_unknown: bool,
) -> io::Result<()> {
    let kind = match &error {
        ClientError::Remote(_) => "remote",
        ClientError::InvalidResponse => "protocol",
        ClientError::StartTimeout | ClientError::ReadTimeout => "timeout",
        ClientError::StartDaemon(_) | ClientError::DaemonPathUnavailable => "unavailable",
        ClientError::Transport(_) => "transport",
    };
    let mut view =
        json!({"kind": kind, "message": error.to_string(), "outcomeUnknown": outcome_unknown});
    if let ClientError::Remote(error) = error {
        view["code"] = json!(error.code);
        view["message"] = json!(error.message);
        if let Some(diagnostic_id) = error.diagnostic_id {
            view["diagnosticId"] = json!(diagnostic_id);
        }
        if let Some(data) = error.data {
            view["data"] = data;
        }
    }
    write_frame(output, &json!({"id": id, "error": view})).await
}
