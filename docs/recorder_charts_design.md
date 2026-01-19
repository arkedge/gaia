# Recorder + Charts Implementation Summary

This document summarizes the current implementation and behavior for the recorder service and the devtools Charts UI, including known behaviors and recent changes.

## Overview

The system consists of:
- `gaia-recorder`: a standalone recorder service that stores telemetry (TMIV) and command logs into per-session SQLite files, and exposes REST endpoints for querying data.
- `tmtc-c2a`: the core TM/TC broker that can forward telemetry/command data to the recorder when `--recorder-endpoint` is provided.
- `devtools-frontend`: the web UI (`/devtools/`) with a new `/charts` screen for time-series visualization, command markers, and recording controls.

## Recorder Service (gaia-recorder)

### Purpose
- Accept telemetry and command logs via gRPC (Recorder API).
- Persist data in SQLite per app session.
- Provide REST APIs for UI access (query series and command logs, list sessions, start/stop recording).

### Operating Modes
- **Default mode**: Automatically starts recording on launch. Accepts new telemetry via gRPC and stores to database.
- **Playback mode** (`--playback-mode`): Read-only mode. No new recording is created. Recording endpoints (`/api/recording/start`, `/api/recording/stop`) return 403 Forbidden. Use this mode to browse historical data without creating new sessions.

### Storage
- File is created at recorder start:
  - `recording_YYYYMMDD_HHMMSS_<suffix>.db` (suffix optional)
- Default directory: `recordings`

### Tables
- `telemetry_samples`
  - Columns: `tmiv_name`, `field_name`, `is_raw`, `time_primary_ms`, `time_received_ms`, `value_type`, `value_num`, `value_int`, `value_text`, `value_bytes`
  - Indexed by `(tmiv_name, field_name, is_raw, time_primary_ms)`
- `command_logs`
  - Columns: `time_ms`, `command_name`, `params_json`
  - Indexed by `time_ms`

### Time handling
- `time_primary_ms`: `tmiv.timestamp` if present; fallback to `plugin_received_time * 1000`
- `time_received_ms`: always `plugin_received_time * 1000`

### REST endpoints
- `POST /api/recording/start` (body: `{ suffix?: string }`)
  - Returns 403 Forbidden in playback mode
- `POST /api/recording/stop`
  - Returns 403 Forbidden in playback mode
- `GET /api/recording/session`
- `GET /api/recordings/list`
- `GET /api/telemetry/query` params:
  - `tmiv_name`, `field_name`, `is_raw`, `start_ms`, `end_ms`, `max_points`, `time_axis`, `session_id`
- `GET /api/telemetry/time_range` params:
  - `session_id` (optional)
  - Returns: `{ min_time_ms: number | null, max_time_ms: number | null }`
  - Queries MIN and MAX of `time_primary_ms` from the telemetry_samples table
- `GET /api/commands/query` params:
  - `start_ms`, `end_ms`, `max_points`, `session_id`
- `GET /api/schema`
  - Returns satellite schema JSON (SatelliteSchema type)
  - Requires `--schema-file` CLI argument to be provided
  - Returns 404 Not Found if no schema file was loaded

### gRPC endpoints (Recorder)
- `PostTelemetry`
- `PostCommand`

## tmtc-c2a Integration

- `tmtc-c2a` can forward TMIV/TCO to the recorder via `--recorder-endpoint`.
- Example:
  - `tmtc-c2a --recorder-endpoint http://127.0.0.1:8920`

## devtools Charts UI

### Route
- `/devtools/#/charts`

### Layout (A plan)
- Left sidebar:
  - Recorder endpoint
  - Start/stop recording (suffix input)
  - Session selector
  - Telemetry/field selector (with search)
  - RAW/converted toggle
  - Time range
  - Follow latest toggle
- Right main:
  - Two fixed panels (Panel A / Panel B)
  - Each panel supports multiple series (2-4 expected)

### Behaviors
- Uses uPlot for charts.
- Live updates every 2 seconds.
- Follow latest ON:
  - X-axis is explicitly set to `[now - rangeMinutes, now]`.
- Zoom:
  - Drag to zoom (X-axis only).
  - Zoom reset icon restores full range and re-enables Follow.

### State timeline rendering
- Enum/string values are converted to categorical states.
- In state-only mode (no numeric series):
  - All state series share the left Y-axis.
  - Each series is placed on its own "lane" (offset values), so multiple states are visible simultaneously.
  - Y-axis labels include series prefix when multiple state series are selected.
- Rendering (2-pass drawing):
  - **First pass**: All bars are drawn horizontally to show state over time.
  - **Second pass**: Text labels are drawn on top of bars to prevent being covered.
  - State labels are drawn at state transitions and at the first visible bar in the viewport.
  - Text is only drawn when there is sufficient space (>20px) in the viewport.
  - Global state mapping ensures consistent colors across all series for the same state value.

### Command markers
- Vertical lines and labels are drawn at command timestamps.
- Toggle via eye icon and switch.

### Panel isolation
- Panel A and Panel B have independent `valueMapsRef` to prevent state mapping conflicts.
- Each panel can display state data and numeric data simultaneously without interference.

### LocalStorage persistence
- **Recorder endpoint**: Saved and restored across sessions.
- **Panel series configuration**: Panel A and Panel B series are saved and restored on reload.
- **Series Picker state**: Last selected TMIV, field, and RAW/converted state are saved and restored.

### Controls UI
- Collapsible "Controls" section with chevron icon.
- Hides all configuration sections (Recording, Debug, Series Picker, Time Range) to focus on chart panels.

### Debug logging
- Built-in debug log buffer (max 1000 entries) tracks:
  - Bar drawing operations with coordinates
  - Text drawing operations with clipping information
  - State mapping and label resolution
  - Series data processing
- Download logs button exports timestamped log file.
- Clear button resets log buffer and console.

## Querying Recorded Data

### Active session
- Select "Active session" in the Recording Files dropdown.
- Data from the currently running recording session is displayed.

### Past recordings
1. Select a session from the Recording Files dropdown (format: `YYYYMMDD_HHMMSS_suffix`).
2. The `session_id` parameter is sent to the API endpoints.
3. Data is queried from the corresponding SQLite database file.

### Automatic time range adjustment

When a past recording session is selected:
1. The UI calls `/api/telemetry/time_range?session_id=XXX` to get the min/max timestamps.
2. "Follow latest" is automatically disabled.
3. The time range is set to the full extent of the recorded data.
4. Recording controls (endpoint, suffix, start/stop buttons) are disabled.
5. A "(Playback Mode)" indicator appears next to the Recording section.
6. The time range is displayed below the recording controls.

### Troubleshooting: Data not showing for selected session

If data doesn't appear:
1. **Check the session exists**: Verify the `.db` file exists in the `recordings` directory.
2. **Check API response**: Open browser DevTools Network tab and verify `/api/telemetry/time_range` returns valid min/max values.
3. **Inspect database**: Use `sqlite3` CLI to verify data exists:
   ```bash
   sqlite3 recordings/recording_YYYYMMDD_HHMMSS_suffix.db
   SELECT COUNT(*) FROM telemetry_samples;
   SELECT datetime(time_primary_ms/1000, 'unixepoch') as time, tmiv_name, field_name
   FROM telemetry_samples LIMIT 10;
   ```

## Usage Examples

### Default mode (live recording)
```bash
gaia-recorder --data-dir recordings
```
- Automatically starts recording on launch
- Creates new database file: `recording_YYYYMMDD_HHMMSS.db`
- Accepts telemetry via gRPC
- UI can start/stop recording with custom suffix

### Playback mode (read-only)
```bash
gaia-recorder --playback-mode --data-dir recordings --schema-file path/to/schema.json
```
- No new recording created
- Recording controls disabled in UI
- Select past sessions from Recording Files dropdown
- Time range automatically adjusted to recorded data extent
- `--schema-file`: Path to satellite schema JSON file (required for standalone viewer without tmtc-c2a)

## Schema Management

### WebSocket-based (with tmtc-c2a)
When the Charts UI is accessed through tmtc-c2a's devtools:
- Satellite schema is provided via WebSocket connection
- Schema is always up-to-date with the running system

### API-based (standalone gaia-recorder)
When using gaia-recorder in standalone mode:
1. Start gaia-recorder with `--schema-file path/to/schema.json`
2. The schema is served via `GET /api/schema`
3. Charts UI automatically fetches from this endpoint if WebSocket is unavailable
4. Schema must be manually updated if satellite configuration changes

### Schema JSON format
The schema file should contain a serialized `SatelliteSchema` object matching the protobuf definition in `gaia-stub`. You can extract this from a running tmtc-c2a instance or generate it from your satellite configuration.

## Known Issues / In-Progress

- Panel series and Series Picker states persist across page reloads, but recording session selection always defaults to "Active session".
- When using `--schema-file`, the schema is static and won't reflect runtime changes to the satellite configuration.

## Build Notes

- `c2a-devtools-frontend` is built during `tmtc-c2a` build via `build.rs`.
- Build can fail due to Vite/esbuild errors if the generated `ChartsView.tsx` has duplicate identifiers.
- Vite warning about outDir not in project root is expected but not fatal.

## Files Touched (Core)

- `gaia-recorder/src/main.rs`
- `gaia-recorder/Cargo.toml`
- `devtools-frontend/src/components/ChartsView.tsx`
- `devtools-frontend/src/components/Layout.tsx`
- `devtools-frontend/src/main.tsx`
- `devtools-frontend/package.json`
- `Cargo.toml` (workspace)

