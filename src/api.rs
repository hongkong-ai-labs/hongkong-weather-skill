//! Hong Kong Observatory open-data client and response models.
//!
//! All endpoints are public, keyless and documented at
//! <https://www.hko.gov.hk/en/abouthko/opendata_intro.htm>. Responses are
//! cached briefly so a single MCP session does not hammer HKO.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, NaiveDate};
use serde::{Deserialize, Serialize};

const BASE: &str = "https://data.weather.gov.hk/weatherAPI/opendata/weather.php";

/// Language accepted by the HKO API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    Tc,
    Sc,
}

impl Lang {
    pub fn parse(value: &str) -> Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "en" | "eng" | "english" => Ok(Self::En),
            "tc" | "zh-hk" | "zh_hk" | "traditional" => Ok(Self::Tc),
            "sc" | "zh-cn" | "zh_cn" | "simplified" => Ok(Self::Sc),
            other => bail!("unknown language '{other}'; use en, tc or sc"),
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Tc => "tc",
            Self::Sc => "sc",
        }
    }
}

/// A single place reading, e.g. a temperature at the Observatory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reading {
    #[serde(default)]
    pub place: String,
    #[serde(default)]
    pub value: serde_json::Value,
    #[serde(default)]
    pub unit: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub main: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
}

impl Reading {
    pub fn display_value(&self) -> String {
        match &self.value {
            serde_json::Value::String(text) => text.clone(),
            serde_json::Value::Number(number) => number.to_string(),
            serde_json::Value::Null => self
                .max
                .map(|max| format!("{max}"))
                .unwrap_or_else(|| "-".to_string()),
            other => other.to_string(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReadingBlock {
    #[serde(default)]
    pub data: Vec<Reading>,
    #[serde(default)]
    pub record_time: Option<String>,
}

/// Current weather report (`dataType=rhrread`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CurrentReport {
    #[serde(default, rename = "updateTime")]
    pub update_time: String,
    #[serde(default)]
    pub temperature: ReadingBlock,
    #[serde(default)]
    pub humidity: ReadingBlock,
    #[serde(default)]
    pub rainfall: ReadingBlock,
    #[serde(default, deserialize_with = "block_or_empty")]
    pub uvindex: ReadingBlock,
    #[serde(default, rename = "warningMessage", deserialize_with = "string_list")]
    pub warning_message: Vec<String>,
    #[serde(default, rename = "rainstormWarning")]
    pub rainstorm_warning: bool,
    #[serde(default, rename = "specialWxTips")]
    pub special_tips: Vec<String>,
    #[serde(default, rename = "tcmessage")]
    pub tc_message: String,
    #[serde(default, rename = "mintempFrom00To09")]
    pub min_temp_00_09: String,
    #[serde(default, rename = "rainfallFrom00To12")]
    pub rainfall_00_12: String,
    #[serde(default, rename = "rainfallLastMonth")]
    pub rainfall_last_month: String,
    #[serde(default, rename = "rainfallJanuaryToLastMonth")]
    pub rainfall_january_to_last_month: String,
}

impl CurrentReport {
    /// Temperature rows matching a case-insensitive place substring.
    pub fn temperatures(&self, place: Option<&str>) -> Vec<Reading> {
        filter_place(&self.temperature.data, place)
    }

    pub fn humidity(&self, place: Option<&str>) -> Vec<Reading> {
        filter_place(&self.humidity.data, place)
    }

    pub fn rainfall(&self, place: Option<&str>) -> Vec<Reading> {
        filter_place(&self.rainfall.data, place)
    }
}

fn filter_place(rows: &[Reading], place: Option<&str>) -> Vec<Reading> {
    match place {
        Some(needle) => {
            let needle = needle.to_lowercase();
            rows.iter()
                .filter(|row| row.place.to_lowercase().contains(&needle))
                .cloned()
                .collect()
        }
        None => rows.to_vec(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemperatureValue {
    #[serde(default)]
    pub value: serde_json::Value,
    #[serde(default)]
    pub unit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForecastDay {
    #[serde(default, rename = "forecastDate")]
    pub date: String,
    #[serde(default)]
    pub week: String,
    #[serde(default, rename = "forecastWind")]
    pub wind: String,
    #[serde(default, rename = "forecastWeather")]
    pub weather: String,
    #[serde(default, rename = "forecastMaxtemp")]
    pub max_temp: Option<TemperatureValue>,
    #[serde(default, rename = "forecastMintemp")]
    pub min_temp: Option<TemperatureValue>,
    #[serde(default, rename = "forecastMaxrh")]
    pub max_rh: Option<TemperatureValue>,
    #[serde(default, rename = "forecastMinrh")]
    pub min_rh: Option<TemperatureValue>,
    #[serde(default, rename = "ForecastIcon")]
    pub icon: Option<i64>,
    #[serde(default, rename = "PSR")]
    pub psr: Option<String>,
}

impl ForecastDay {
    pub fn date_label(&self) -> String {
        NaiveDate::parse_from_str(&self.date, "%Y%m%d")
            .map(|date| date.format("%a %d %b %Y").to_string())
            .unwrap_or_else(|_| self.date.clone())
    }

    pub fn temperature(&self) -> String {
        let low = self.min_temp.as_ref().map(|value| value.value.to_string());
        let high = self.max_temp.as_ref().map(|value| value.value.to_string());
        match (low, high) {
            (Some(low), Some(high)) => format!("{low}-{high}°C"),
            _ => "-".to_string(),
        }
    }

    pub fn humidity(&self) -> String {
        let low = self.min_rh.as_ref().map(|value| value.value.to_string());
        let high = self.max_rh.as_ref().map(|value| value.value.to_string());
        match (low, high) {
            (Some(low), Some(high)) => format!("{low}-{high}%"),
            _ => "-".to_string(),
        }
    }

    /// Probability of significant rain as a percentage, when published.
    pub fn rain_probability(&self) -> String {
        self.psr
            .as_deref()
            .map(|value| match value.to_ascii_lowercase().as_str() {
                "low" => "Low".to_string(),
                "medium" | "med" => "Medium".to_string(),
                "high" => "High".to_string(),
                other => other.to_string(),
            })
            .unwrap_or_else(|| "-".to_string())
    }
}

/// Nine-day forecast (`dataType=fnd`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ForecastReport {
    #[serde(default, rename = "generalSituation")]
    pub general_situation: String,
    #[serde(default, rename = "tcInfo")]
    pub tc_info: String,
    #[serde(default, rename = "fireDangerWarning")]
    pub fire_danger_warning: String,
    #[serde(default, rename = "forecastPeriod")]
    pub forecast_period: String,
    #[serde(default, rename = "forecastDesc")]
    pub forecast_desc: String,
    #[serde(default)]
    pub outlook: String,
    #[serde(default, rename = "updateTime")]
    pub update_time: String,
    #[serde(default, rename = "weatherForecast")]
    pub days: Vec<ForecastDay>,
}

/// Local weather forecast (`dataType=flw`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LocalForecast {
    #[serde(default, rename = "generalSituation")]
    pub general_situation: String,
    #[serde(default, rename = "tcInfo")]
    pub tc_info: String,
    #[serde(default, rename = "fireDangerWarning")]
    pub fire_danger_warning: String,
    #[serde(default, rename = "forecastPeriod")]
    pub forecast_period: String,
    #[serde(default, rename = "forecastDesc")]
    pub forecast_desc: String,
    #[serde(default)]
    pub outlook: String,
    #[serde(default, rename = "updateTime")]
    pub update_time: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WarningDetail {
    #[serde(default)]
    pub contents: Vec<String>,
    #[serde(default, rename = "warningStatementCode")]
    pub code: String,
    #[serde(default)]
    pub subtype: Option<String>,
    #[serde(default, rename = "updateTime")]
    pub update_time: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
    #[serde(default, rename = "expiryTime")]
    pub expiry_time: Option<String>,
}

impl WarningDetail {
    pub fn name(&self) -> &'static str {
        warning_name(&self.code, self.subtype.as_deref())
    }

    pub fn is_cancelled(&self) -> bool {
        self.contents
            .iter()
            .any(|line| line.to_ascii_lowercase().contains("cancel"))
            || self
                .status
                .as_deref()
                .is_some_and(|status| status.eq_ignore_ascii_case("cancelled"))
    }

    pub fn text(&self) -> String {
        self.contents.join(" ")
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WarningReport {
    #[serde(default)]
    pub details: Vec<WarningDetail>,
}

/// HKO sometimes returns an empty string instead of an empty reading block.
fn block_or_empty<'de, D>(deserializer: D) -> Result<ReadingBlock, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::Object(_) => serde_json::from_value(value).map_err(serde::de::Error::custom),
        _ => Ok(ReadingBlock::default()),
    }
}

/// `warningMessage` is a string for a single warning and an array for several.
fn string_list<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(match value {
        serde_json::Value::Array(items) => items
            .into_iter()
            .filter_map(|item| item.as_str().map(ToOwned::to_owned))
            .collect(),
        serde_json::Value::String(text) if !text.trim().is_empty() => vec![text],
        _ => Vec::new(),
    })
}

/// Map HKO warning statement codes to readable names.
pub fn warning_name(code: &str, subtype: Option<&str>) -> &'static str {
    match (code, subtype) {
        ("WTCSGNL", _) => "Tropical Cyclone Warning Signal",
        ("WRAIN", _) => "Rainstorm Warning",
        ("WFIRE", Some("WFIREY")) => "Yellow Fire Danger Warning",
        ("WFIRE", Some("WFIRER")) => "Red Fire Danger Warning",
        ("WFIRE", _) => "Fire Danger Warning",
        ("WTS", _) => "Thunderstorm Warning",
        ("WCOLD", Some("WCOLDY")) => "Yellow Cold Weather Warning",
        ("WCOLD", Some("WCOLDR")) => "Red Cold Weather Warning",
        ("WCOLD", _) => "Cold Weather Warning",
        ("WHOT", Some("WHOTY")) => "Yellow Very Hot Weather Warning",
        ("WHOT", Some("WHOTR")) => "Red Very Hot Weather Warning",
        ("WHOT", _) => "Very Hot Weather Warning",
        ("WMSGNL", _) => "Strong Monsoon Signal",
        ("WFL", _) => "Flooding Warning",
        ("WL", _) => "Landslip Warning",
        ("WFNTSA", _) => "Special Announcement on Flooding in the Northern New Territories",
        ("WTMW", _) => "Tsunami Warning",
        ("WFROST", _) => "Frost Warning",
        ("WHMW", _) => "Monsoon Wind Warning",
        _ => "Weather Warning",
    }
}

/// Cached HKO client.
pub struct Client {
    http: reqwest::Client,
    base: String,
    ttl: Duration,
    cache: Mutex<HashMap<String, (Instant, serde_json::Value)>>,
}

impl Client {
    pub fn new(ttl: Duration) -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(concat!(
                "hongkong-weather-skill/",
                env!("CARGO_PKG_VERSION"),
                " (+https://github.com/hongkong-ai-labs/hongkong-weather-skill)"
            ))
            .timeout(Duration::from_secs(20))
            .build()?;
        Ok(Self {
            http,
            base: BASE.to_string(),
            ttl,
            cache: Mutex::new(HashMap::new()),
        })
    }

    /// Fetch one HKO data type, using the in-memory TTL cache.
    pub async fn fetch(&self, data_type: &str, lang: Lang) -> Result<serde_json::Value> {
        let key = format!("{data_type}:{}", lang.code());
        if self.ttl > Duration::ZERO
            && let Some((at, value)) = self
                .cache
                .lock()
                .expect("weather cache poisoned")
                .get(&key)
                .cloned()
            && at.elapsed() < self.ttl
        {
            return Ok(value);
        }
        let response = self
            .http
            .get(&self.base)
            .query(&[("dataType", data_type), ("lang", lang.code())])
            .send()
            .await
            .with_context(|| format!("requesting HKO {data_type}"))?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            bail!("HKO {data_type} returned HTTP {status}: {body}");
        }
        let value: serde_json::Value = response
            .json()
            .await
            .with_context(|| format!("decoding HKO {data_type}"))?;
        if value
            .get("error")
            .and_then(serde_json::Value::as_str)
            .is_some()
        {
            bail!(
                "HKO {data_type} error: {}",
                value.get("error").cloned().unwrap_or_default()
            );
        }
        if self.ttl > Duration::ZERO {
            self.cache
                .lock()
                .expect("weather cache poisoned")
                .insert(key, (Instant::now(), value.clone()));
        }
        Ok(value)
    }

    pub async fn current(&self, lang: Lang) -> Result<CurrentReport> {
        let value = self.fetch("rhrread", lang).await?;
        Ok(serde_json::from_value(value)?)
    }

    pub async fn forecast(&self, lang: Lang) -> Result<ForecastReport> {
        let value = self.fetch("fnd", lang).await?;
        Ok(serde_json::from_value(value)?)
    }

    pub async fn local_forecast(&self, lang: Lang) -> Result<LocalForecast> {
        let value = self.fetch("flw", lang).await?;
        Ok(serde_json::from_value(value)?)
    }

    pub async fn warnings(&self, lang: Lang) -> Result<WarningReport> {
        let value = self.fetch("warninginfo", lang).await?;
        Ok(serde_json::from_value(value)?)
    }
}

/// Parse an HKO RFC 3339 timestamp, falling back to the raw string.
pub fn timestamp(value: &str) -> String {
    DateTime::parse_from_rfc3339(value)
        .map(|stamp| stamp.naive_local().format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|_| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_current_readings() {
        let json = r#"{
            "updateTime":"2026-11-21T23:02:00+08:00",
            "temperature":{"data":[{"place":"Hong Kong Observatory","value":21.4,"unit":"C"}],
                           "recordTime":"2026-11-21T23:00:00+08:00"},
            "humidity":{"data":[{"place":"Hong Kong Observatory","value":67,"unit":"percent"}]},
            "rainfall":{"data":[{"place":"Central & Western District","max":0,"main":"FALSE","unit":"mm"}]},
            "uvindex":{"data":[{"place":"King's Park","value":0,"desc":"low"}]},
            "warningMessage":["Fire Danger Warning cancelled."]
        }"#;
        let report: CurrentReport = serde_json::from_str(json).unwrap();
        assert_eq!(report.temperatures(None)[0].display_value(), "21.4");
        assert_eq!(report.rainfall(None)[0].max, Some(0.0));
        assert_eq!(report.uvindex.data[0].desc.as_deref(), Some("low"));
    }

    #[test]
    fn parses_forecast_and_warnings() {
        let forecast: ForecastReport = serde_json::from_str(
            r#"{"weatherForecast":[{"forecastDate":"20261122","week":"Sunday",
            "forecastWeather":"Mainly cloudy","forecastMaxtemp":{"value":23,"unit":"C"},
            "forecastMintemp":{"value":19,"unit":"C"},"PSR":"Low"}]}"#,
        )
        .unwrap();
        assert_eq!(forecast.days[0].temperature(), "19-23°C");
        assert_eq!(forecast.days[0].rain_probability(), "Low");

        let warnings: WarningReport = serde_json::from_str(
            r#"{"details":[{"contents":["Fire Danger Warning cancelled."],
            "warningStatementCode":"WFIRE","subtype":"WFIRER",
            "updateTime":"2026-11-21T23:50:00+08:00"}]}"#,
        )
        .unwrap();
        assert!(warnings.details[0].is_cancelled());
        assert_eq!(warnings.details[0].name(), "Red Fire Danger Warning");
    }
}
