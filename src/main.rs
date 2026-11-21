//! `hkweather` — Hong Kong Observatory weather data CLI and MCP server.

mod api;
mod mcp;
mod render;

use std::time::Duration;

use anyhow::Result;
use clap::{Parser, Subcommand};
use hongkong_skill_core::update::{self, Config};
use serde_json::json;

use api::{Client, Lang};

const UPDATER: Config = Config {
    repo: "hongkong-ai-labs/hongkong-weather-skill",
    binary: "hkweather",
    env_prefix: "HKWEATHER",
    version: env!("CARGO_PKG_VERSION"),
};

#[derive(Parser)]
#[command(
    name = "hkweather",
    version,
    about = "Hong Kong Observatory weather data — CLI and MCP server",
    arg_required_else_help = true
)]
struct Cli {
    /// Print machine-readable JSON instead of tables
    #[arg(long, global = true)]
    json: bool,

    /// Response language: en, tc or sc
    #[arg(long, global = true, default_value = "en")]
    lang: String,

    /// Cache HKO responses for N seconds (0 disables)
    #[arg(long, global = true, default_value_t = 60)]
    cache_ttl: u64,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Current temperature, humidity, rainfall and UV readings
    #[command(alias = "now")]
    Current {
        /// Place-name substring, e.g. "Observatory", "Sha Tin"
        #[arg(long, short)]
        place: Option<String>,
    },
    /// Nine-day forecast
    #[command(alias = "fnd")]
    Forecast {
        /// Number of days to show (1-9)
        #[arg(long, short)]
        days: Option<usize>,
    },
    /// Active weather warnings
    Warnings {
        /// Include recently cancelled warnings
        #[arg(long)]
        all: bool,
    },
    /// Local weather forecast for today and tomorrow
    #[command(alias = "flw")]
    Local,
    /// Rainfall by district since midnight
    Rain,
    /// UV index by station
    Uv,
    /// Everything: current, warnings, local forecast and nine-day forecast
    All {
        /// Place-name substring for current readings
        #[arg(long, short)]
        place: Option<String>,
    },
    /// Update this binary using the method it was installed with
    Update {
        /// Only report the current version, latest release and install method
        #[arg(long)]
        check: bool,
        /// Reinstall even when the current version is up to date
        #[arg(long)]
        force: bool,
    },
    /// Run the MCP server over stdio
    Mcp,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let lang = Lang::parse(&cli.lang)?;
    let client = Client::new(Duration::from_secs(cli.cache_ttl))?;

    if let Command::Mcp = cli.command {
        return mcp::serve(client).await;
    }

    match cli.command {
        Command::Current { place } => {
            let report = client.current(lang).await?;
            if cli.json {
                let place_filter = place.clone();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "update_time": report.update_time,
                        "place_filter": place_filter,
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
                    }))?
                );
            } else {
                println!("{}", render::current(&report, place.as_deref()));
            }
        }
        Command::Forecast { days } => {
            let report = client.forecast(lang).await?;
            let days = days.map(|value| value.clamp(1, 9));
            if cli.json {
                let take = days.unwrap_or(report.days.len());
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "update_time": report.update_time,
                        "general_situation": report.general_situation,
                        "tropical_cyclone": report.tc_info,
                        "outlook": report.outlook,
                        "days": report.days.into_iter().take(take).collect::<Vec<_>>()
                    }))?
                );
            } else {
                println!("{}", render::forecast(&report, days));
            }
        }
        Command::Warnings { all } => {
            let report = client.warnings(lang).await?;
            if cli.json {
                let warnings: Vec<_> = report
                    .details
                    .iter()
                    .filter(|warning| all || !warning.is_cancelled())
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
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "count": warnings.len(),
                        "warnings": warnings
                    }))?
                );
            } else {
                println!("{}", render::warnings(&report, all));
            }
        }
        Command::Local => {
            let report = client.local_forecast(lang).await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("{}", render::local(&report));
            }
        }
        Command::Rain => {
            let report = client.current(lang).await?;
            if cli.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "update_time": report.update_time,
                        "rainfall": report.rainfall.data,
                        "midnight_to_noon_mm": report.rainfall_00_12,
                        "last_month_mm": report.rainfall_last_month,
                        "year_to_date_mm": report.rainfall_january_to_last_month
                    }))?
                );
            } else {
                println!("{}", render::rain(&report));
            }
        }
        Command::Uv => {
            let report = client.current(lang).await?;
            if cli.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "update_time": report.update_time,
                        "uv_index": report.uvindex.data
                    }))?
                );
            } else {
                println!("{}", render::uv(&report));
            }
        }
        Command::All { place } => {
            let current = client.current(lang).await?;
            let warnings = client.warnings(lang).await?;
            let local = client.local_forecast(lang).await?;
            let forecast = client.forecast(lang).await?;
            if cli.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "current": {
                            "update_time": current.update_time,
                            "temperature": current.temperatures(place.as_deref()),
                            "humidity": current.humidity(place.as_deref()),
                            "rainfall": current.rainfall(place.as_deref()),
                            "uv_index": current.uvindex.data,
                            "warnings": current.warning_message
                        },
                        "warnings": warnings.details,
                        "local_forecast": local,
                        "forecast": forecast
                    }))?
                );
            } else {
                println!("{}", render::current(&current, place.as_deref()));
                println!("\n{}", render::warnings(&warnings, false));
                println!("\n{}", render::local(&local));
                println!("\n{}", render::forecast(&forecast, None));
            }
        }
        Command::Update { check, force } => {
            update::run(&UPDATER, check, force)?;
        }
        Command::Mcp => unreachable!(),
    }
    Ok(())
}
