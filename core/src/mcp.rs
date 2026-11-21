//! Minimal MCP server over stdio.
//!
//! The transport is JSON-RPC 2.0, one object per line. This implements the
//! subset the skills use: `initialize`, `ping`, `tools/list` and `tools/call`.
//!
//! Two behaviours matter more than the rest:
//!
//! - A tool that fails returns `{ "isError": true }` inside a successful
//!   `tools/call` result. Returning a JSON-RPC error instead makes clients
//!   drop the call (and sometimes the session), so upstream outages would look
//!   like a broken server.
//! - The server never builds a Tokio runtime of its own. It is an `async fn`
//!   that the caller awaits, so a `#[tokio::main]` binary cannot panic with
//!   "Cannot start a runtime from within a runtime".
//!
//! Not implemented, on purpose: batch requests, prompts, resources, sampling,
//! pagination and cancellation. A server that needs resources should use an
//! SDK (the airport, MTR and Ma Wan skills use `rmcp`).

use std::future::Future;

use anyhow::Result;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// Revision this server prefers.
pub const PROTOCOL_VERSION: &str = "2025-06-18";

/// Revisions this server can speak, newest first.
pub const SUPPORTED_VERSIONS: [&str; 2] = ["2025-06-18", "2024-11-05"];

/// One tool advertised by `tools/list`.
#[derive(Debug, Clone)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

impl Tool {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        input_schema: Value,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema,
        }
    }

    /// The object placed in the `tools` array of `tools/list`.
    pub fn to_json(&self) -> Value {
        json!({
            "name": self.name,
            "description": self.description,
            "inputSchema": self.input_schema
        })
    }
}

/// Pick the protocol revision to answer with.
///
/// A client that asks for a supported revision keeps it. Anything else gets
/// the server's preferred revision, which is what the specification asks for.
pub fn negotiate(requested: Option<&str>) -> &'static str {
    requested
        .filter(|value| SUPPORTED_VERSIONS.contains(value))
        .and_then(|value| {
            SUPPORTED_VERSIONS
                .iter()
                .find(|supported| **supported == value)
                .copied()
        })
        .unwrap_or(PROTOCOL_VERSION)
}

/// Build the `initialize` result.
pub fn initialize_result(requested: Option<&str>, server_name: &str, version: &str) -> Value {
    json!({
        "protocolVersion": negotiate(requested),
        "capabilities": { "tools": {} },
        "serverInfo": {
            "name": server_name,
            "version": version
        }
    })
}

/// Build the `tools/list` result.
pub fn tools_result(tools: &[Tool]) -> Value {
    json!({ "tools": tools.iter().map(Tool::to_json).collect::<Vec<_>>() })
}

/// A successful JSON-RPC response.
pub fn success(id: Option<Value>, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

/// A JSON-RPC protocol error.
pub fn error(id: Option<Value>, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message }
    })
}

/// Wrap a tool payload as a successful `tools/call` result.
///
/// String payloads are sent as text only; everything else is pretty-printed
/// and also exposed as `structuredContent`.
pub fn tool_result(value: Value) -> Value {
    let (text, structured) = match &value {
        Value::String(text) => (text.clone(), None),
        other => (
            serde_json::to_string_pretty(other).unwrap_or_default(),
            Some(other.clone()),
        ),
    };
    let mut result = json!({
        "content": [{ "type": "text", "text": text }],
        "isError": false
    });
    if let Some(structured) = structured {
        result["structuredContent"] = structured;
    }
    result
}

/// Wrap a tool failure as an `isError: true` result.
pub fn tool_error(message: &str) -> Value {
    json!({
        "content": [{ "type": "text", "text": message }],
        "isError": true
    })
}

/// Serve one MCP session over stdin/stdout.
///
/// `handler` receives the tool name and its arguments and returns either a
/// payload ([`tool_result`]) or a human-readable failure ([`tool_error`]).
/// Unknown tool names are rejected here, before the handler runs.
pub async fn serve_stdio<F, Fut>(
    server_name: &str,
    server_version: &str,
    tools: Vec<Tool>,
    handler: F,
) -> Result<()>
where
    F: Fn(String, Value) -> Fut,
    Fut: Future<Output = std::result::Result<Value, String>>,
{
    let names: Vec<String> = tools.iter().map(|tool| tool.name.clone()).collect();
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();

    while let Some(line) = lines.next_line().await? {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let request: Value = match serde_json::from_str(line) {
            Ok(value) => value,
            Err(cause) => {
                write(
                    &mut stdout,
                    error(None, -32700, &format!("parse error: {cause}")),
                )
                .await?;
                continue;
            }
        };

        if request.is_array() {
            write(
                &mut stdout,
                error(None, -32600, "batch requests are not supported"),
            )
            .await?;
            continue;
        }

        // No id means a notification (for example notifications/initialized).
        let Some(id) = request.get("id").cloned() else {
            continue;
        };

        let method = request
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();

        let response = match method {
            "initialize" => {
                let requested = request
                    .get("params")
                    .and_then(|params| params.get("protocolVersion"))
                    .and_then(Value::as_str);
                success(
                    Some(id),
                    initialize_result(requested, server_name, server_version),
                )
            }
            "ping" => success(Some(id), json!({})),
            "tools/list" => success(Some(id), tools_result(&tools)),
            "tools/call" => {
                let params = request.get("params");
                let name = params
                    .and_then(|params| params.get("name"))
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                if !names.iter().any(|known| known == name) {
                    error(Some(id), -32602, &format!("unknown tool: {name}"))
                } else {
                    let arguments = params
                        .and_then(|params| params.get("arguments"))
                        .cloned()
                        .unwrap_or_else(|| json!({}));
                    match handler(name.to_string(), arguments).await {
                        Ok(value) => success(Some(id), tool_result(value)),
                        Err(message) => success(Some(id), tool_error(&message)),
                    }
                }
            }
            other => error(Some(id), -32601, &format!("method not found: {other}")),
        };

        write(&mut stdout, response).await?;
    }

    Ok(())
}

async fn write(stdout: &mut tokio::io::Stdout, response: Value) -> Result<()> {
    stdout
        .write_all(serde_json::to_string(&response)?.as_bytes())
        .await?;
    stdout.write_all(b"\n").await?;
    stdout.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tools() -> Vec<Tool> {
        vec![Tool::new(
            "hko_warnings",
            "Active warnings",
            json!({ "type": "object", "properties": {} }),
        )]
    }

    #[test]
    fn negotiates_known_versions_and_falls_back() {
        assert_eq!(negotiate(Some("2025-06-18")), "2025-06-18");
        assert_eq!(negotiate(Some("2024-11-05")), "2024-11-05");
        assert_eq!(negotiate(Some("1999-01-01")), PROTOCOL_VERSION);
        assert_eq!(negotiate(None), PROTOCOL_VERSION);
    }

    #[test]
    fn initialize_advertises_tools_and_version() {
        let result = initialize_result(Some("2024-11-05"), "test-skill", "1.2.3");
        assert_eq!(result["protocolVersion"], "2024-11-05");
        assert!(result["capabilities"]["tools"].is_object());
        assert_eq!(result["serverInfo"]["name"], "test-skill");
        assert_eq!(result["serverInfo"]["version"], "1.2.3");
    }

    #[test]
    fn tools_list_has_the_expected_shape() {
        let result = tools_result(&tools());
        let listed = result["tools"].as_array().expect("tools array");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0]["name"], "hko_warnings");
        assert_eq!(listed[0]["inputSchema"]["type"], "object");
    }

    #[test]
    fn failures_are_tool_errors_not_protocol_errors() {
        let result = tool_error("upstream unavailable");
        assert_eq!(result["isError"], true);
        assert_eq!(result["content"][0]["text"], "upstream unavailable");
        assert!(result.get("structuredContent").is_none());
    }

    #[test]
    fn json_payloads_carry_structured_content() {
        let result = tool_result(json!({ "count": 0, "warnings": [] }));
        assert_eq!(result["isError"], false);
        assert_eq!(result["structuredContent"]["count"], 0);
        assert!(
            result["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("count")
        );
    }

    #[test]
    fn string_payloads_stay_text_only() {
        let result = tool_result(Value::String("table".into()));
        assert_eq!(result["content"][0]["text"], "table");
        assert!(result.get("structuredContent").is_none());
    }
}
