//! Minimal stdio MCP transport over the same deterministic capability service.
//! It never requests model sampling, opens a network listener, or launches agents.
use crate::capabilities::{self, catalog};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{
    io::{BufRead, Write},
    path::{Path, PathBuf},
};
const MAX_INPUT: usize = 2 * 1024 * 1024;
const MAX_OUTPUT: usize = 8 * 1024 * 1024;
pub struct Session {
    root: PathBuf,
    initialized: bool,
    ready: bool,
    all_tools: bool,
}
fn error(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
}
fn name(id: &str) -> String {
    format!("sa_{}", id.replace(['.', '-'], "_"))
}
fn tools(all: bool) -> Vec<Value> {
    let mut list:Vec<_>=catalog::all().into_iter().filter(|c|all||c.primary).map(|c|json!({"name":name(&c.id),"description":c.description,"inputSchema":c.input_schema,"outputSchema":c.output_schema,"annotations":{"readOnlyHint":!c.mutates,"destructiveHint":c.mutates,"openWorldHint":false}})).collect();
    list.push(json!({"name":"sa_tools","description":"Discover granular capabilities or invoke one through its structured schema.","inputSchema":{"type":"object","additionalProperties":false,"required":["operation"],"properties":{"operation":{"type":"string","enum":["list","call"]},"capability":{"type":"string"},"arguments":{"type":"object"}}},"annotations":{"readOnlyHint":false}}));
    list
}
impl Session {
    pub fn new(root: &Path, all_tools: bool) -> Result<Self> {
        Ok(Self {
            root: root.canonicalize()?,
            initialized: false,
            ready: false,
            all_tools,
        })
    }
    pub fn handle(&mut self, request: Value) -> Option<Value> {
        let id = request.get("id").cloned();
        if request["jsonrpc"] != "2.0"
            || !request["method"].is_string()
            || id
                .as_ref()
                .is_some_and(|id| !id.is_string() && !id.is_number())
        {
            return Some(error(
                id.unwrap_or(Value::Null),
                -32600,
                "Invalid JSON-RPC request",
            ));
        }
        let method = request["method"].as_str().unwrap();
        let params = request.get("params").cloned().unwrap_or(json!({}));
        if id.is_none() {
            if method == "notifications/initialized" && self.initialized {
                self.ready = true;
            }
            return None;
        }
        let id = id.unwrap();
        if method == "initialize" {
            if self.initialized {
                return Some(error(id, -32600, "Already initialized"));
            }
            if !params["protocolVersion"].is_string()
                || !params["capabilities"].is_object()
                || !params["clientInfo"].is_object()
            {
                return Some(error(id, -32602, "Invalid initialize parameters"));
            }
            self.initialized = true;
            let requested = params["protocolVersion"].as_str().unwrap();
            let version =
                if ["2024-11-05", "2025-03-26", "2025-06-18", "2025-11-25"].contains(&requested) {
                    requested
                } else {
                    "2025-11-25"
                };
            return Some(
                json!({"jsonrpc":"2.0","id":id,"result":{"protocolVersion":version,"capabilities":{"tools":{"listChanged":false},"resources":{"subscribe":false,"listChanged":false}},"serverInfo":{"name":"spec-autonomous","version":env!("CARGO_PKG_VERSION")},"instructions":"Seven complete capabilities are exposed by default. Use sa_tools to discover granular operations. The host owns every agent session; prepare returns work and apply-result accepts receipts."}}),
            );
        }
        if method == "ping" {
            return Some(json!({"jsonrpc":"2.0","id":id,"result":{}}));
        }
        if !self.initialized || !self.ready {
            return Some(error(
                id,
                -32002,
                "Initialize and send notifications/initialized first",
            ));
        }
        let result: Result<Value> = (|| match method {
            "tools/list" => {
                if params.get("cursor").is_some() {
                    bail!("invalid_cursor: tool list has a single page");
                }
                Ok(json!({"tools":tools(self.all_tools)}))
            }
            "tools/call" => {
                let tool = params["name"]
                    .as_str()
                    .context("invalid_params: missing tool name")?;
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                let called = if tool == "sa_tools" {
                    match args["operation"].as_str() {
                        Some("list") => Ok(catalog::list(true)),
                        Some("call") => capabilities::invoke(
                            &self.root,
                            args["capability"]
                                .as_str()
                                .context("invalid_params: capability required")?,
                            args.get("arguments").unwrap_or(&json!({})),
                        ),
                        _ => bail!("invalid_params: sa_tools operation must be list or call"),
                    }
                } else {
                    let cap = catalog::all()
                        .into_iter()
                        .find(|c| name(&c.id) == tool && (self.all_tools || c.primary))
                        .context("unknown_tool")?;
                    capabilities::invoke(&self.root, &cap.id, &args)
                };
                match called {
                    Ok(data) => {
                        let envelope = json!({"schema_version":1,"data":data});
                        let text = serde_json::to_string(&envelope)?;
                        if text.len() > MAX_OUTPUT {
                            bail!("response_too_large: select fields or use a bounded resource");
                        }
                        Ok(
                            json!({"content":[{"type":"text","text":text}],"structuredContent":envelope,"isError":false}),
                        )
                    }
                    Err(e) => {
                        let message = format!("{e:#}");
                        Ok(json!({"content":[{"type":"text","text":message}],"isError":true}))
                    }
                }
            }
            "resources/list" => Ok(
                json!({"resources":[{"uri":"spec-autonomous://capabilities","name":"capabilities","mimeType":"application/json"},{"uri":"spec-autonomous://progress","name":"progress","mimeType":"application/json"}]}),
            ),
            "resources/templates/list" => Ok(
                json!({"resourceTemplates":[{"uriTemplate":"spec-autonomous://runs/{run_id}","name":"run","mimeType":"application/json"},{"uriTemplate":"spec-autonomous://work/{run_id}/{request_id}","name":"work-context","mimeType":"application/json"}]}),
            ),
            "resources/read" => {
                let uri = params["uri"]
                    .as_str()
                    .context("invalid_params: uri required")?;
                let data = if uri == "spec-autonomous://capabilities" {
                    catalog::list(true)
                } else if uri == "spec-autonomous://progress" {
                    capabilities::invoke(&self.root, "progress", &json!({}))?
                } else if let Some(id) = uri.strip_prefix("spec-autonomous://runs/") {
                    crate::paths::valid_id(id)?;
                    capabilities::invoke(&self.root, "state.get", &json!({"run_id":id}))?
                } else if let Some(pair) = uri.strip_prefix("spec-autonomous://work/") {
                    let (run, request) = pair.split_once('/').context("invalid_resource")?;
                    crate::paths::valid_id(run)?;
                    crate::paths::valid_id(request)?;
                    capabilities::invoke(
                        &self.root,
                        "work.context",
                        &json!({"run_id":run,"request_id":request,"view":"full"}),
                    )?
                } else {
                    bail!("unknown_resource");
                };
                let text = serde_json::to_string(&data)?;
                if text.len() > MAX_OUTPUT {
                    bail!("response_too_large");
                }
                Ok(json!({"contents":[{"uri":uri,"mimeType":"application/json","text":text}]}))
            }
            _ => bail!("method_not_found"),
        })();
        Some(match result {
            Ok(result) => json!({"jsonrpc":"2.0","id":id,"result":result}),
            Err(e) => {
                let message = e.to_string();
                error(
                    id,
                    if message == "method_not_found" {
                        -32601
                    } else {
                        -32602
                    },
                    &message,
                )
            }
        })
    }
}
fn line(reader: &mut impl BufRead) -> Result<Option<Vec<u8>>> {
    let mut line = vec![];
    loop {
        let buffer = reader.fill_buf()?;
        if buffer.is_empty() {
            return Ok(if line.is_empty() { None } else { Some(line) });
        }
        let length = buffer
            .iter()
            .position(|b| *b == b'\n')
            .map_or(buffer.len(), |i| i + 1);
        if line.len() + length > MAX_INPUT {
            bail!("request_too_large");
        }
        let done = buffer[length - 1] == b'\n';
        line.extend_from_slice(&buffer[..length]);
        reader.consume(length);
        if done {
            return Ok(Some(line));
        }
    }
}
pub fn serve(
    root: &Path,
    all: bool,
    mut input: impl BufRead + Send + 'static,
    mut output: impl Write,
) -> Result<()> {
    let mut session = Session::new(root, all)?;
    let (send, receive) = std::sync::mpsc::sync_channel(2);
    std::thread::spawn(move || {
        loop {
            let next = line(&mut input);
            let stop = !matches!(next, Ok(Some(_)));
            if send.send(next).is_err() || stop {
                break;
            }
        }
    });
    loop {
        if crate::process::interrupted() {
            break;
        }
        let message = match receive.recv_timeout(std::time::Duration::from_millis(50)) {
            Ok(m) => m,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(_) => break,
        };
        let bytes = match message {
            Ok(Some(bytes)) => bytes,
            Ok(None) => break,
            Err(error_) => {
                writeln!(
                    output,
                    "{}",
                    error(Value::Null, -32600, &error_.to_string())
                )?;
                output.flush()?;
                break;
            }
        };
        if bytes.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let response = match serde_json::from_slice(&bytes) {
            Ok(request) => session.handle(request),
            Err(_) => Some(error(Value::Null, -32700, "Invalid JSON")),
        };
        if let Some(response) = response {
            writeln!(output, "{response}")?;
            output.flush()?;
        }
    }
    Ok(())
}
