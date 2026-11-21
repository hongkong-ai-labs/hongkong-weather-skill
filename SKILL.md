---
name: hongkong-weather-skill
description: Hong Kong Observatory weather data — current temperature, humidity, rainfall and UV readings, active warnings, the local forecast, and the nine-day forecast for Hong Kong. Use for any question about Hong Kong weather, HKO forecasts, typhoons, rainstorms, cold or hot weather warnings, or district rainfall.
license: MIT
---

# Hong Kong weather (HKO)

`hkweather` is a Rust CLI and MCP server for the Hong Kong Observatory's
public open-data API. It needs no API key and no account. It is an independent,
unofficial project and is not affiliated with HKO.

Use it when the user asks about:

- current temperature, humidity, UV index or rainfall in Hong Kong
- today's local forecast, the outlook, or the nine-day forecast
- active weather warnings: tropical cyclones, rainstorms, thunderstorm, fire
  danger, cold weather, very hot weather, strong monsoon, landslip, flooding
- rainfall by district, or how wet a month or year has been
- whether a typhoon signal or rainstorm warning is in force

## Before first use

Run `scripts/ensure-binary.sh` from the skill directory:

```console
$ bash scripts/ensure-binary.sh
/Users/you/.local/bin/hkweather
```

It prints the absolute path of a working binary. Use that path for every
command, or capture it once:

```bash
BIN="$(bash scripts/ensure-binary.sh)"
```

It checks, in order: `$HKWEATHER_BIN`, `hkweather` on `PATH`,
`scripts/install.sh`, then `cargo build --release --locked` in the repo root.

### Install the binary directly

`npx skills add` installs the skill files. If the binary is missing, install it
with the release installer or cargo:

```bash
curl -fsSL https://raw.githubusercontent.com/hongkong-ai-labs/hongkong-weather-skill/main/scripts/install.sh | sh
# or
cargo install --git https://github.com/hongkong-ai-labs/hongkong-weather-skill --locked
```

Register it with any MCP client:

```bash
npx add-mcp "$(command -v hkweather) mcp" --name hongkong-weather -a claude-code -a codex -a cursor -y
claude mcp add hongkong-weather -- hkweather mcp
codex mcp add hongkong-weather -- hkweather mcp
```

`add-mcp` detects the client configuration files in the current project; add
`-g` for a global install. `$(command -v hkweather)` keeps the entry correct
when the binary is on `PATH`; substitute an absolute path if your client does
not inherit the shell `PATH`.

## CLI

Prefer `--json` when you need to parse. Tables are for humans. All timestamps
are Hong Kong time (UTC+8).

```bash
$BIN current --json                       # all current readings
$BIN current --place "Sha Tin" --json     # one place
$BIN forecast --json                      # nine-day forecast
$BIN forecast --days 3 --json             # first three days
$BIN warnings --json                      # active warnings
$BIN warnings --all --json                # include cancelled warnings
$BIN local --json                         # local forecast + outlook
$BIN rain --json                          # district rainfall since midnight
$BIN uv --json                            # UV index by station
$BIN all --json                           # everything in one object
$BIN update --check
```

`--lang tc` or `--lang sc` returns HKO's Traditional or Simplified Chinese
text. Field names stay English.

### What the JSON contains

`current`:

| Field | Meaning |
| --- | --- |
| `update_time` | HKO publication time, RFC 3339 with `+08:00` |
| `temperature` | `[{place, value, unit}]` |
| `humidity` | `[{place, value, unit}]` |
| `rainfall` | `[{place, max, main, unit}]`, millimetres since midnight |
| `uv_index` | `[{place, value, desc}]` |
| `warnings` | HKO warning headline strings (`[]` when none) |
| `rainfall_totals` | midnight-to-noon, last month, year to date (mm) |

`forecast.days[]` has `forecastDate` (`YYYYMMDD`), `week`, `forecastWeather`,
`forecastWind`, `forecastMaxtemp` / `forecastMintemp`
(`{value, unit}`), `forecastMaxrh` / `forecastMinrh`, `ForecastIcon`, and
`PSR` — the probability of significant rain (`Low`, `Medium`, `High`).

`warnings` are objects with `code`, `name`, `cancelled`, `subtype`,
`update_time`, `expiry_time`, and `contents`. A code such as `WTCSGNL` is the
tropical cyclone warning signal statement; the signal number is in the
`contents` text, so quote that text rather than guessing a number.

## MCP

```bash
$BIN mcp
```

```json
{
  "mcpServers": {
    "hongkong-weather": {
      "command": "/absolute/path/to/hkweather",
      "args": ["mcp"]
    }
  }
}
```

Tools:

| Tool | Purpose |
| --- | --- |
| `hko_current_weather` | Current readings, optional `station` substring |
| `hko_forecast` | Nine-day forecast, optional `days` |
| `hko_warnings` | Active warnings, optional `include_cancelled` |
| `hko_local_forecast` | Local forecast and outlook |
| `hko_rainfall` | District rainfall since midnight |

All tools accept `lang` (`en`, `tc`, `sc`).

## Answering safely

- Say "HKO says" and give the publication time. HKO updates readings and
  forecasts on its own schedule; a cached answer can be a few minutes old.
- Warnings are safety information. Quote the warning text and signal number.
  Do not downgrade, summarise away, or predict when a warning will be lifted.
- The nine-day forecast is a forecast, not a guarantee. Give the date range and
  the rain probability band rather than a single certainty.
- "Low" rain probability does not mean no rain; scattered showers still happen.
- UV index is station-specific and is not a sunscreen recommendation.
- Rainfall is since midnight in millimetres, not an hourly rate.
- This is an independent client of HKO open data. It is not an official HKO
  channel. For decisions involving safety, ferries, flights, or outdoor work,
  check the HKO website or app directly.

## Source

HKO open data, `https://data.weather.gov.hk/weatherAPI/opendata/weather.php`,
with `dataType=rhrread|fnd|flw|warninginfo`. No key is required. Responses are
cached for 60 seconds in-process (`--cache-ttl 0` disables).
