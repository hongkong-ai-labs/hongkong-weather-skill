//! Human-readable tables for the CLI.

use crate::api::{CurrentReport, ForecastReport, LocalForecast, Reading, WarningReport, timestamp};

fn width(rows: &[Vec<String>], headers: &[&str]) -> Vec<usize> {
    let mut widths: Vec<usize> = headers.iter().map(|header| header.len()).collect();
    for row in rows {
        for (index, cell) in row.iter().enumerate() {
            if index < widths.len() {
                widths[index] = widths[index].max(cell.chars().count());
            }
        }
    }
    widths
}

fn table(headers: &[&str], rows: &[Vec<String>]) -> String {
    let widths = width(rows, headers);
    let mut out = String::new();
    out.push_str(
        headers
            .iter()
            .enumerate()
            .map(|(index, header)| format!("{header:<width$}", width = widths[index]))
            .collect::<Vec<_>>()
            .join("  ")
            .trim_end(),
    );
    out.push('\n');
    out.push_str(
        &widths
            .iter()
            .map(|width| "-".repeat(*width))
            .collect::<Vec<_>>()
            .join("  "),
    );
    out.push('\n');
    for row in rows {
        out.push_str(
            row.iter()
                .enumerate()
                .map(|(index, cell)| format!("{cell:<width$}", width = widths[index]))
                .collect::<Vec<_>>()
                .join("  ")
                .trim_end(),
        );
        out.push('\n');
    }
    out
}

fn reading_rows(rows: &[Reading]) -> Vec<Vec<String>> {
    rows.iter()
        .map(|row| vec![row.place.clone(), row.display_value(), row.unit.clone()])
        .collect()
}

pub fn current(report: &CurrentReport, place: Option<&str>) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Hong Kong Observatory — current weather ({})\n\n",
        timestamp(&report.update_time)
    ));
    if !report.warning_message.is_empty() {
        out.push_str("Warnings\n");
        for line in &report.warning_message {
            out.push_str(&format!("  ! {line}\n"));
        }
        out.push('\n');
    }
    if !report.tc_message.is_empty() && report.tc_message != "None" {
        out.push_str(&format!("Tropical cyclone: {}\n\n", report.tc_message));
    }
    let temperatures = report.temperatures(place);
    if !temperatures.is_empty() {
        out.push_str("Temperature\n");
        out.push_str(&table(
            &["PLACE", "VALUE", "UNIT"],
            &reading_rows(&temperatures),
        ));
        out.push('\n');
    }
    let humidity = report.humidity(place);
    if !humidity.is_empty() {
        out.push_str("Humidity\n");
        out.push_str(&table(
            &["PLACE", "VALUE", "UNIT"],
            &reading_rows(&humidity),
        ));
        out.push('\n');
    }
    let rainfall = report.rainfall(place);
    if !rainfall.is_empty() {
        out.push_str("Rainfall since midnight\n");
        out.push_str(&table(
            &["PLACE", "VALUE", "UNIT"],
            &reading_rows(&rainfall),
        ));
        out.push('\n');
    }
    if !report.uvindex.data.is_empty() {
        out.push_str("UV index\n");
        let rows: Vec<Vec<String>> = report
            .uvindex
            .data
            .iter()
            .map(|row| {
                vec![
                    row.place.clone(),
                    row.display_value(),
                    row.desc.clone().unwrap_or_else(|| "-".to_string()),
                ]
            })
            .collect();
        out.push_str(&table(&["PLACE", "UV", "DESCRIPTION"], &rows));
        out.push('\n');
    }
    if !report.rainfall_00_12.is_empty() || !report.rainfall_last_month.is_empty() {
        out.push_str("Rainfall totals\n");
        if !report.rainfall_00_12.is_empty() {
            out.push_str(&format!(
                "  Midnight to noon: {} mm\n",
                report.rainfall_00_12
            ));
        }
        if !report.rainfall_last_month.is_empty() {
            out.push_str(&format!(
                "  Last month:      {} mm\n",
                report.rainfall_last_month
            ));
        }
        if !report.rainfall_january_to_last_month.is_empty() {
            out.push_str(&format!(
                "  Year to date:    {} mm\n",
                report.rainfall_january_to_last_month
            ));
        }
        out.push('\n');
    }
    for tip in &report.special_tips {
        out.push_str(&format!("Note: {tip}\n"));
    }
    out.trim_end().to_string()
}

pub fn forecast(report: &ForecastReport, days: Option<usize>) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Hong Kong Observatory — nine-day forecast ({})\n\n",
        timestamp(&report.update_time)
    ));
    if !report.general_situation.is_empty() {
        out.push_str("Situation\n");
        out.push_str(&wrap(&report.general_situation, 96));
        out.push_str("\n\n");
    }
    if !report.tc_info.is_empty() && report.tc_info != "None" {
        out.push_str(&format!("Tropical cyclone: {}\n\n", report.tc_info));
    }
    let limit = days.unwrap_or(report.days.len()).min(report.days.len());
    let rows: Vec<Vec<String>> = report
        .days
        .iter()
        .take(limit)
        .map(|day| {
            vec![
                day.date_label(),
                day.week.clone(),
                day.weather.clone(),
                day.temperature(),
                day.humidity(),
                day.rain_probability(),
            ]
        })
        .collect();
    out.push_str(&table(
        &["DATE", "DAY", "FORECAST", "TEMP", "HUMIDITY", "RAIN"],
        &rows,
    ));
    if !report.outlook.is_empty() {
        out.push_str("\nOutlook\n");
        out.push_str(&wrap(&report.outlook, 96));
        out.push('\n');
    }
    out.trim_end().to_string()
}

pub fn warnings(report: &WarningReport, include_cancelled: bool) -> String {
    let rows: Vec<Vec<String>> = report
        .details
        .iter()
        .filter(|warning| include_cancelled || !warning.is_cancelled())
        .map(|warning| {
            vec![
                warning.code.clone(),
                warning.name().to_string(),
                if warning.is_cancelled() {
                    "cancelled".to_string()
                } else {
                    "active".to_string()
                },
                timestamp(&warning.update_time),
                warning.text(),
            ]
        })
        .collect();
    if rows.is_empty() {
        return "No active Hong Kong weather warnings.".to_string();
    }
    table(&["CODE", "WARNING", "STATUS", "UPDATED", "DETAIL"], &rows)
        .trim_end()
        .to_string()
}

pub fn local(report: &LocalForecast) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Hong Kong local weather forecast ({})\n\n",
        timestamp(&report.update_time)
    ));
    if !report.general_situation.is_empty() {
        out.push_str("Situation\n");
        out.push_str(&wrap(&report.general_situation, 96));
        out.push_str("\n\n");
    }
    if !report.forecast_period.is_empty() {
        out.push_str(&format!("Forecast for {}\n", report.forecast_period));
    }
    if !report.forecast_desc.is_empty() {
        out.push_str(&wrap(&report.forecast_desc, 96));
        out.push('\n');
    }
    if !report.outlook.is_empty() {
        out.push_str("\nOutlook\n");
        out.push_str(&wrap(&report.outlook, 96));
        out.push('\n');
    }
    out.trim_end().to_string()
}

pub fn rain(report: &CurrentReport) -> String {
    let rows: Vec<Vec<String>> = report
        .rainfall
        .data
        .iter()
        .map(|row| {
            vec![
                row.place.clone(),
                row.max
                    .map(|max| format!("{max}"))
                    .unwrap_or_else(|| "-".to_string()),
                row.unit.clone(),
                row.main.clone().unwrap_or_default(),
            ]
        })
        .collect();
    let mut out = table(&["DISTRICT", "MAX SINCE MIDNIGHT", "UNIT", "MAIN"], &rows);
    if !report.rainfall_00_12.is_empty() {
        out.push_str(&format!(
            "\nMidnight to noon total: {} mm\n",
            report.rainfall_00_12
        ));
    }
    out.trim_end().to_string()
}

pub fn uv(report: &CurrentReport) -> String {
    let rows: Vec<Vec<String>> = report
        .uvindex
        .data
        .iter()
        .map(|row| {
            vec![
                row.place.clone(),
                row.display_value(),
                row.desc.clone().unwrap_or_else(|| "-".to_string()),
            ]
        })
        .collect();
    table(&["PLACE", "UV INDEX", "DESCRIPTION"], &rows)
        .trim_end()
        .to_string()
}

fn wrap(text: &str, width: usize) -> String {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > width {
            lines.push(line);
            line = String::new();
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines.join("\n")
}
