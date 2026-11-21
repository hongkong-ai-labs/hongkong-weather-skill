//! MCP over stdio: JSON-RPC 2.0, one object per line.
//!
//! Tools expose the same HKO data as the CLI. The server keeps its cache for
//! the lifetime of the process and never exits on a malformed request.

use std::io::{BufRead, Write};

use serde_json::{Value, json};

use crate::api::{Client, Lang};

pub const PROTOCOL_VERSION: &str = "2025-06-18";
const SUPPORTED_VERSIONS: [&str; 2] = ["2025-06-18", "2024-11-05"];

fn lang_from(args: &Value) -> Lang {
    args.get("lang")
        .and_then(Value::as_str)
        .and_then(|value| Lang::parse(value).ok())
        .unwrap_or(Lang::En)
}

fn station(args: &Value) -> Option<String> {
    args.get("station")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub fn serve(client: Client) -> anyhow::Result<()> {
    let stdin = std::io::stdin();
    let mut stdout = std::io::BufWriter::new(std::io::stdout().lock());
    eprintln!("hkweather MCP server listening on stdio");
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(response) = handle(&client, &line) {
            writeln!(stdout, "{}", serde_json::to_string(&response)?)?;
            stdout.flush()?;
        }
    }
    Ok(())
}

fn handle(client: &Client, line: &str) -> Option<Value> {
    match serde_json::from_str::<Value>(line) {
        Ok(request) => dispatch(client, &request),
        Err(error) => Some(error_response(
            None,
            -32700,
            &format!("parse error: {error}"),
        )),
    }
}

fn dispatch(client: &Client, request: &Value) -> Option<Value> {
    let id = request.get("id").cloned();
    let method = match request.get("method").and_then(Value::as_str) {
        Some(method) => method,
        None => {
            return Some(error_response(
                id,
                -32600,
                "invalid request: missing method",
            ));
        }
    };
    if method.starts_with("notifications/") {
        return None;
    }
    match method {
        "initialize" => Some(success(id, initialize(request.get("params")))),
        "ping" => Some(success(id, json!({}))),
        "tools/list" => Some(success(id, tools())),
        "tools/call" => Some(call(client, id, request.get("params"))),
        other => Some(error_response(
            id,
            -32601,
            &format!("method not found: {other}"),
        )),
    }
}

fn initialize(params: Option<&Value>) -> Value {
    let requested = params
        .and_then(|value| value.get("protocolVersion"))
        .and_then(Value::as_str);
    let version = requested
        .filter(|value| SUPPORTED_VERSIONS.contains(value))
        .unwrap_or(PROTOCOL_VERSION);
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": {} },
        "serverInfo": {
            "name": "hongkong-weather-skill",
            "version": env!("CARGO_PKG_VERSION")
        }
    })
}

fn tools() -> Value {
    json!({ "tools": [
        {
            "name": "hko_current_weather",
            "description": "Current HKO temperature, humidity, rainfall, UV index and warning messages, optionally filtered to one place.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "station": {"type": "string", "description": "Place-name substring, e.g. 'Observatory', 'Sha Tin' or 'Central'."},
                    "lang": {"type": "string", "enum": ["en", "tc", "sc"], "description": "Response language. Default en."}
                }
            }
        },
        {
            "name": "hko_forecast",
            "description": "HKO nine-day forecast with daily weather, temperatures, humidity and rain probability.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "days": {"type": "integer", "minimum": 1, "maximum": 9, "description": "Number of days to return. Default all."},
                    "lang": {"type": "string", "enum": ["en", "tc", "sc"]}
                }
            }
        },
        {
            "name": "hko_warnings",
            "description": "Active Hong Kong weather warnings, with cancelled warnings optional.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "include_cancelled": {"type": "boolean", "description": "Include recently cancelled warnings. Default false."},
                    "lang": {"type": "string", "enum": ["en", "tc", "sc"]}
                }
            }
        },
        {
            "name": "hko_local_forecast",
            "description": "HKO local weather forecast: general situation, forecast period, description and outlook.",
            "inputSchema": {
                "type": "object",
                "properties": {"lang": {"type": "string", "enum": ["en", "tc", "sc"]}}
            }
        },
        {
            "name": "hko_rainfall",
            "description": "Rainfall since midnight for every Hong Kong district.",
            "inputSchema": {
                "type": "object",
                "properties": {"lang": {"type": "string", "enum": ["en", "tc", "sc"]}}
            }
        }
    ]})
}

fn call(client: &Client, id: Option<Value>, params: Option<&Value>) -> Value {
    let Some(params) = params else {
        return error_response(id, -32602, "missing params");
    };
    let name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let lang = lang_from(&args);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build();
    let runtime = match runtime {
        Ok(runtime) => runtime,
        Err(error) => {
            return error_response(id, -32603, &format!("runtime error: {error}"));
        }
    };
    let outcome: anyhow::Result<Value> = runtime.block_on(async {
        match name {
            "hko_current_weather" => {
                let report = client.current(lang).await?;
                let place = station(&args);
                let temperatures = report.temperatures(place.as_deref());
                let humidity = report.humidity(place.as_deref());
                let rainfall = report.rainfall(place.as_deref());
                Ok(json!({
                    "update_time": report.update_time,
                    "place_filter": place,
                    "temperature": temperatures,
                    "humidity": humidity,
                    "rainfall": rainfall,
                    "uv_index": report.uvindex.data,
                    "warnings": report.warning_message,
                    "tropical_cyclone": report.tc_message,
                    "special_tips": report.special_tips,
                    "rainfall_totals": {
                        "midnight_to_noon_mm": report.rainfall_00_12,
                        "last_month_mm": report.rainfall_last_month,
                        "year_to_date_mm": report.rainfall_january_to_last_month
                    }
                }))
            }
            "hko_forecast" => {
                let report = client.forecast(lang).await?;
                let days = args
                    .get("days")
                    .and_then(Value::as_u64)
                    .map(|value| value as usize)
                    .unwrap_or(report.days.len())
                    .clamp(1, 9);
                Ok(json!({
                    "update_time": report.update_time,
                    "general_situation": report.general_situation,
                    "tropical_cyclone": report.tc_info,
                    "outlook": report.outlook,
                    "days": report.days.into_iter().take(days).collect::<Vec<_>>()
                }))
            }
            "hko_warnings" => {
                let report = client.warnings(lang).await?;
                let include = args
                    .get("include_cancelled")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let warnings: Vec<Value> = report
                    .details
                    .into_iter()
                    .filter(|warning| include || !warning.is_cancelled())
                    .map(|warning| {
                        json!({
                            "code": warning.code,
                            "name": warning.name(),
                            "cancelled": warning.is_cancelled(),
                            "subtype": warning.subtype,
                            "update_time": warning.update_time,
                            "expiry_time": warning.expiry_time,
                            "contents": warning.contents
                        })
                    })
                    .collect();
                Ok(json!({ "count": warnings.len(), "warnings": warnings }))
            }
            "hko_local_forecast" => {
                let report = client.local_forecast(lang).await?;
                Ok(json!({
                    "update_time": report.update_time,
                    "general_situation": report.general_situation,
                    "forecast_period": report.forecast_period,
                    "forecast": report.forecast_desc,
                    "outlook": report.outlook,
                    "fire_danger_warning": report.fire_danger_warning
                }))
            }
            "hko_rainfall" => {
                let report = client.current(lang).await?;
                Ok(json!({
                    "update_time": report.update_time,
                    "rainfall": report.rainfall.data,
                    "midnight_to_noon_mm": report.rainfall_00_12,
                    "last_month_mm": report.rainfall_last_month,
                    "year_to_date_mm": report.rainfall_january_to_last_month
                }))
            }
            other => Err(anyhow::anyhow!("unknown tool: {other}")),
        }
    });
    match outcome {
        Ok(value) => success(
            id,
            json!({
                "content": [{
                    "type": "text",
                    "text": serde_json::to_string_pretty(&value).unwrap_or_default()
                }],
                "structuredContent": value,
                "isError": false
            }),
        ),
        Err(error) => error_response(id, -32603, &format!("{error:#}")),
    }
}

fn success(id: Option<Value>, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn error_response(id: Option<Value>, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message }
    })
}
