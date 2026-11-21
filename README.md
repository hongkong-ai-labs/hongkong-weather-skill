# hkweather

Hong Kong Observatory weather data as a Rust CLI and an MCP server.

`hkweather` reads HKO's public, keyless open data and exposes current
readings, district rainfall, UV, active warnings, the local forecast and the
nine-day forecast as tables, JSON, or MCP tools.

It is an independent, unofficial project, not affiliated with the Hong Kong
Observatory.

## Install

```bash
npx skills add hongkong-ai-labs/hongkong-weather-skill   # agent skill
cargo install --path . --locked                          # binary
sh scripts/install.sh                                    # -> ~/.local/bin
```

Requires Rust 1.85 or newer.

## CLI

```bash
hkweather current                       # all current readings
hkweather current --place "Sha Tin"     # one place
hkweather forecast --days 3             # first three forecast days
hkweather warnings                      # active warnings
hkweather warnings --all                # include cancelled
hkweather local                         # local forecast + outlook
hkweather rain                          # district rainfall
hkweather uv                            # UV index
hkweather all                           # everything
hkweather --json current                # machine-readable
hkweather --lang tc forecast            # Traditional Chinese HKO text
hkweather mcp                           # MCP over stdio
```

## MCP

```json
{
  "mcpServers": {
    "hongkong-weather": {
      "command": "hkweather",
      "args": ["mcp"]
    }
  }
}
```

Tools: `hko_current_weather`, `hko_forecast`, `hko_warnings`,
`hko_local_forecast`, `hko_rainfall`.

## Data

| Dataset | HKO `dataType` |
| --- | --- |
| Current readings | `rhrread` |
| Nine-day forecast | `fnd` |
| Local forecast | `flw` |
| Warnings | `warninginfo` |

Source: `https://data.weather.gov.hk/weatherAPI/opendata/weather.php`.
No API key. Responses are cached for 60 seconds (`--cache-ttl 0` disables).

## Limits

HKO data is published for general information. Warnings and forecasts can
change; always check HKO directly for safety-critical decisions. The API does
not provide a per-minute nowcast, lightning location, or official district
rainfall forecast.

## Licence

MIT. See [LICENSE](LICENSE). HKO data remains subject to the Observatory's
terms of use.
