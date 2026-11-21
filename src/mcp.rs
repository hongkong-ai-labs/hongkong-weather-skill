//! MCP over stdio for HKO data.
//!
//! The protocol loop lives in `hongkong-skill-core`; this file only declares
//! the tools and maps them onto the HKO client. Tool failures become
//! `isError: true` results, so a client keeps its session when HKO is down.

use std::sync::Arc;

use hongkong_skill_core::mcp::{self, Tool};
use serde_json::{Value, json};

use crate::api::{Client, Lang};

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

fn tools() -> Vec<Tool> {
    vec![
        Tool::new(
            "hko_current_weather",
            "Current HKO temperature, humidity, rainfall, UV index and warning messages, optionally filtered to one place.",
            json!({
                "type": "object",
                "properties": {
                    "station": {"type": "string", "description": "Place-name substring, e.g. 'Observatory', 'Sha Tin' or 'Central'."},
                    "lang": {"type": "string", "enum": ["en", "tc", "sc"], "description": "Response language. Default en."}
                }
            }),
        ),
        Tool::new(
            "hko_forecast",
            "HKO nine-day forecast with daily weather, temperatures, humidity and rain probability.",
            json!({
                "type": "object",
                "properties": {
                    "days": {"type": "integer", "minimum": 1, "maximum": 9, "description": "Number of days to return. Default all."},
                    "lang": {"type": "string", "enum": ["en", "tc", "sc"]}
                }
            }),
        ),
        Tool::new(
            "hko_warnings",
            "Active Hong Kong weather warnings, with cancelled warnings optional.",
            json!({
                "type": "object",
                "properties": {
                    "include_cancelled": {"type": "boolean", "description": "Include recently cancelled warnings. Default false."},
                    "lang": {"type": "string", "enum": ["en", "tc", "sc"]}
                }
            }),
        ),
        Tool::new(
            "hko_local_forecast",
            "HKO local weather forecast: general situation, forecast period, description and outlook.",
            json!({
                "type": "object",
                "properties": {
                    "lang": {"type": "string", "enum": ["en", "tc", "sc"]}
                }
            }),
        ),
        Tool::new(
            "hko_rainfall",
            "Rainfall since midnight for every Hong Kong district.",
            json!({
                "type": "object",
                "properties": {
                    "lang": {"type": "string", "enum": ["en", "tc", "sc"]}
                }
            }),
        ),
    ]
}

pub async fn serve(client: Client) -> anyhow::Result<()> {
    let client = Arc::new(client);
    mcp::serve_stdio(
        "hongkong-weather-skill",
        env!("CARGO_PKG_VERSION"),
        tools(),
        move |name, args| {
            let client = Arc::clone(&client);
            async move { call(&client, &name, &args).await }
        },
    )
    .await
}

async fn call(client: &Client, name: &str, args: &Value) -> Result<Value, String> {
    let lang = lang_from(args);
    match name {
        "hko_current_weather" => {
            let report = client.current(lang).await.map_err(err)?;
            let place = station(args);
            Ok(json!({
                "update_time": report.update_time,
                "place_filter": place,
                "temperature": report.temperatures(place.as_deref()),
                "humidity": report.humidity(place.as_deref()),
                "rainfall": report.rainfall(place.as_deref()),
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
            let report = client.forecast(lang).await.map_err(err)?;
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
            let report = client.warnings(lang).await.map_err(err)?;
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
            let report = client.local_forecast(lang).await.map_err(err)?;
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
            let report = client.current(lang).await.map_err(err)?;
            Ok(json!({
                "update_time": report.update_time,
                "rainfall": report.rainfall.data,
                "midnight_to_noon_mm": report.rainfall_00_12,
                "last_month_mm": report.rainfall_last_month,
                "year_to_date_mm": report.rainfall_january_to_last_month
            }))
        }
        other => Err(format!("unknown tool: {other}")),
    }
}

fn err(error: anyhow::Error) -> String {
    format!("{error:#}")
}
