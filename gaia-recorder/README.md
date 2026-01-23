# gaia-recorder

Satellite telemetry and command recording service with REST API and gRPC interface.

## Architecture Overview

gaia-recorder is a telemetry recording and playback service designed for satellite ground station operations. It stores telemetry data (TMIV) and commands (TCO) in a DuckDB database and provides both gRPC ingestion and REST API query interfaces.

### Key Features

- **Dual Interface**: gRPC for real-time telemetry ingestion, REST API for querying
- **Persistent Storage**: DuckDB-based storage with optimized indexing
- **Playback Mode**: Read-only mode for analyzing recorded sessions
- **Field Name Transformation**: Automatic conversion between gRPC (`SH_TI@RAW`) and database (`SH.TI:raw`) formats
- **Value Type Normalization**: Backward-compatible type system supporting legacy formats
- **Time Axis Selection**: Query by primary (satellite) or received (ground station) timestamps
- **Smart Downsampling**: Automatic data reduction for large datasets using min-max-avg algorithm

## Module Structure

The codebase is organized into focused modules with clear responsibilities:

```
gaia-recorder/
├── src/
│   ├── main.rs              (~120 lines) - Application entry point and wiring
│   ├── lib.rs               - Public API exports for binaries
│   ├── api/                 - HTTP and gRPC API layers
│   │   ├── mod.rs           - API module exports
│   │   ├── grpc.rs          - gRPC service implementation (RecorderService)
│   │   └── http.rs          - REST API handlers and router
│   ├── db/                  - Database operations
│   │   ├── mod.rs           - Database module exports
│   │   ├── schema.rs        - Schema initialization
│   │   ├── queries.rs       - Query operations (telemetry, commands, time range)
│   │   └── insert.rs        - Insert operations (telemetry, commands)
│   ├── domain/              - Domain types and business logic
│   │   ├── mod.rs           - Domain module exports
│   │   └── telemetry.rs     - ValueType enum with backward compatibility
│   ├── transform/           - Data transformation utilities
│   │   ├── mod.rs           - Transform module exports
│   │   └── field_names.rs   - FieldName conversion (gRPC ↔ database)
│   └── bin/
│       ├── import-csv.rs    - CSV import utility
│       └── list-fields.rs   - Field listing utility
├── Cargo.toml
└── README.md
```

## Module Responsibilities

### `main.rs` - Application Entry Point
- CLI argument parsing (`--bind-addr`, `--bind-port`, `--data-dir`, `--playback-mode`)
- Logging initialization
- State setup
- gRPC and HTTP service wiring
- Server lifecycle management

### `api/` - API Layer
**Purpose**: Separate HTTP and gRPC interfaces from business logic

- **`grpc.rs`**: RecorderService implementing gRPC Recorder trait
  - `post_telemetry`: Ingest TMIV packets
  - `post_command`: Ingest TCO packets
- **`http.rs`**: REST API handlers and router creation
  - Session management (`/api/recording/start`, `/api/recording/stop`)
  - Telemetry queries (`/api/telemetry/query`)
  - Command queries (`/api/commands/query`)
  - Time range queries (`/api/telemetry/time_range`)
  - Recording list (`/api/recordings/list`)
  - Schema endpoint (`/api/schema`)

### `db/` - Database Layer
**Purpose**: Encapsulate all database operations with DuckDB

- **`schema.rs`**: Database initialization and table creation
  - `telemetry_samples` table with composite index
  - `command_logs` table with time index
- **`queries.rs`**: Read operations
  - `query_telemetry`: Fetch telemetry with downsampling
  - `query_commands`: Fetch command logs
  - `query_time_range`: Get min/max timestamps
  - Internal downsampling functions (stride, min-max-avg)
- **`insert.rs`**: Write operations
  - `insert_telemetry_sample`: Store TMIV field samples
  - `insert_command_log`: Store TCO command logs
  - `build_params_json`: Serialize command parameters

### `domain/` - Domain Types
**Purpose**: Define business domain types with validation

- **`telemetry.rs`**: `ValueType` enum
  - Canonical formats: `integer`, `double`, `string`, `enum`, `bytes`, `unknown`
  - Backward compatibility: supports legacy `int`, `num`, `text` formats
  - Conversion methods: `to_db_string()`, `from_db_string()`

### `transform/` - Data Transformation
**Purpose**: Centralize format conversions

- **`field_names.rs`**: `FieldName` struct
  - gRPC format: `SH_TI` or `SH_TI@RAW` (underscores, `@RAW` suffix)
  - Database format: `SH.TI:conv` or `SH.TI:raw` (dots, `:conv`/`:raw` suffix)
  - Bidirectional conversion with comprehensive test coverage

## Data Flow

### Telemetry Ingestion (gRPC)
```
Satellite → gRPC PostTelemetry
           → RecorderService::post_telemetry
           → transform::FieldName::from_grpc (SH_TI@RAW → SH.TI:raw)
           → db::insert_telemetry_sample
           → DuckDB telemetry_samples table
```

### Telemetry Query (REST API)
```
Frontend → GET /api/telemetry/query
        → api::http::query_telemetry
        → resolve_session_path (session_id → db_path)
        → db::query_telemetry
        → Downsample if needed (min-max-avg for numeric, stride for non-numeric)
        → Normalize value_type (domain::ValueType::from_db_string)
        → JSON response
```

### CSV Import
```
CSV file → import-csv binary
        → transform::FieldName::from_grpc (column names)
        → domain::ValueType (int→integer, num→double, text→string)
        → DuckDB COPY FROM with type inference
        → telemetry_samples table
```

## Database Schema

### `telemetry_samples`
```sql
CREATE TABLE telemetry_samples (
    id INTEGER PRIMARY KEY,
    tmiv_name VARCHAR NOT NULL,           -- TMIV packet name
    field_name VARCHAR NOT NULL,          -- Field name with :raw or :conv suffix
    is_raw TINYINT NOT NULL,              -- 0=converted, 1=raw
    time_primary_ms BIGINT NOT NULL,      -- Satellite timestamp
    time_received_ms BIGINT NOT NULL,     -- Ground station timestamp
    value_type VARCHAR(20) NOT NULL,      -- integer, double, enum, string, bytes
    value_num DOUBLE,                     -- Numeric value (integer, double, bytes≤8)
    value_int BIGINT,                     -- Integer value
    value_text VARCHAR,                   -- Text value (enum label, string)
    value_bytes BLOB                      -- Raw bytes
);

CREATE INDEX idx_telemetry_query
    ON telemetry_samples (tmiv_name, field_name, is_raw, time_primary_ms);
```

### `command_logs`
```sql
CREATE TABLE command_logs (
    id INTEGER PRIMARY KEY,
    time_ms BIGINT NOT NULL,              -- Command timestamp
    command_name VARCHAR NOT NULL,        -- TCO command name
    params_json VARCHAR NOT NULL          -- JSON-serialized parameters
);

CREATE INDEX idx_command_time ON command_logs (time_ms);
```

## Field Name Formats

### gRPC Format (Input)
- Normal telemetry: `SH_TI`, `OBC_MM_OPSMODE`
- Raw telemetry: `SH_TI@RAW`, `OBC_MM_OPSMODE@RAW`
- Case-insensitive suffix: `@raw` also accepted

### Database Format (Storage)
- Normal telemetry: `SH.TI:conv`, `OBC.MM.OPSMODE:conv`
- Raw telemetry: `SH.TI:raw`, `OBC.MM.OPSMODE:raw`
- Underscores replaced with dots
- Suffix indicates conversion status

### Conversion Rules
1. Replace `_` with `.`: `SH_TI` → `SH.TI`
2. Strip `@RAW`/`@raw` suffix if present
3. Add `:raw` for raw telemetry, `:conv` for converted telemetry

## Value Types

### Canonical Formats (New)
- `integer` - 64-bit signed integer
- `double` - 64-bit floating point
- `string` - Text string
- `enum` - Enumeration with text label
- `bytes` - Raw byte array
- `unknown` - Unknown or missing type

### Legacy Formats (Backward Compatible)
- `int` → `integer`
- `num` → `double`
- `text` → `string`

### Type Storage Strategy
- **Numeric types** (`integer`, `double`): Stored in `value_num` and `value_int`
- **Text types** (`enum`, `string`): Stored in `value_text`
- **Bytes type**: Stored in `value_bytes`, with `value_int`/`value_num` if ≤8 bytes

## Downsampling Algorithm

When query returns more points than `max_points`, automatic downsampling is applied:

### For Numeric Data (Min-Max-Avg)
1. Divide data into `max_points / 3` buckets
2. For each bucket, calculate min, max, avg
3. Output 3 points per bucket (min, max, avg) at bucket's median timestamp
4. Preserves value range and trends

### For Non-Numeric Data (Stride)
1. Calculate stride: `samples.len() / max_points`
2. Take every Nth sample
3. Preserves temporal distribution

## Operating Modes

### Recording Mode (Default)
- Accepts gRPC telemetry and commands
- Creates `recording_YYYYMMDD_HHMMSS[_suffix].duckdb` in `--data-dir`
- REST API can start/stop recording sessions
- Multiple sessions can be queried by `session_id`

### Playback Mode (`--playback-mode`)
- Read-only: `/api/recording/start` and `/api/recording/stop` return 403
- No new database files created
- Existing recordings can be queried
- Time range queries automatically adjusted to database bounds
- Useful for offline analysis

## API Endpoints

### Session Management
- `POST /api/recording/start` - Start new recording session (requires `suffix`)
- `POST /api/recording/stop` - Stop current recording session
- `GET /api/recording/session` - Get current session info

### Queries
- `GET /api/telemetry/query` - Query telemetry samples
  - Params: `session_id`, `tmiv_name`, `field_name`, `is_raw`, `start_ms`, `end_ms`, `max_points`, `time_axis`
- `GET /api/commands/query` - Query command logs
  - Params: `session_id`, `start_ms`, `end_ms`, `max_points`
- `GET /api/telemetry/time_range` - Get min/max timestamps
  - Params: `session_id`

### Metadata
- `GET /api/recordings/list` - List all recording sessions
- `GET /api/schema` - Get satellite schema JSON (if `--schema-file` provided)

## Configuration

### Command-Line Arguments
```bash
gaia-recorder \
  --bind-addr 0.0.0.0 \           # Bind address (default: 0.0.0.0)
  --bind-port 8920 \              # Bind port (default: 8920)
  --data-dir ./recordings \       # Data directory (default: recordings)
  --playback-mode \               # Enable read-only playback mode
  --schema-file schema.json       # Optional satellite schema file
```

### Environment Variables
All CLI arguments can be set via environment variables:
- `BIND_ADDR`, `BIND_PORT`, `DATA_DIR`, `PLAYBACK_MODE`, `SCHEMA_FILE`

## Binaries

### `gaia-recorder` (Main Service)
```bash
# Start recorder
cargo run --release

# Start in playback mode
cargo run --release -- --playback-mode --data-dir /path/to/recordings
```

### `import-csv` (CSV Import Utility)
```bash
cargo run --release --bin import-csv -- \
  --input-dir /path/to/csv \
  --output-db recording.duckdb \
  --session-id 20250120_190015
```

Expected CSV structure:
- `TLM/` directory with TMIV CSV files
- `CMD/` directory with TCO CSV files
- Column names in gRPC format (`SH_TI`, `SH_TI@RAW`)
- First column is timestamp

### `list-fields` (Field Listing Utility)
Lists all available fields in a recording database.

## Testing

```bash
# Run all tests
cargo test

# Run specific module tests
cargo test domain::telemetry
cargo test transform::field_names

# Build release binary
cargo build --release
```

## Performance Considerations

### Indexing
- Composite index on `(tmiv_name, field_name, is_raw, time_primary_ms)` enables fast queries
- Time index on command_logs for efficient temporal queries

### DuckDB Features
- Automatic compression reduces disk usage
- Optimized data types (TINYINT for is_raw, BIGINT for timestamps)
- Efficient BLOB storage for telemetry bytes

### Downsampling
- Applied automatically when `samples.len() > max_points`
- Default `max_points` = 2000 for telemetry, 10000 for commands
- Min-max-avg preserves value range for numeric data
- Stride sampling maintains temporal distribution

## Design Decisions

### Why DuckDB?
- **Embedded**: No separate database server required
- **Performant**: Columnar storage optimized for analytical queries
- **Portable**: Single-file databases, easy to backup/transfer
- **Compression**: Automatic compression reduces storage costs

### Why Separate gRPC and REST?
- **gRPC**: Low-latency telemetry ingestion from satellite systems
- **REST**: Simple HTTP queries for frontend/tooling integration
- **Fallback Service**: gRPC handled by Axum fallback, single port

### Why Field Name Transformation?
- **gRPC Compatibility**: `_` is standard in protobuf field names
- **Database Clarity**: `.` is more readable for hierarchical field names
- **Raw Indicator**: `:raw` vs `:conv` suffix clarifies data processing status
- **Frontend Consistency**: Single format simplifies frontend logic

### Why Value Type Normalization?
- **Data Consistency**: All new data uses canonical types
- **Backward Compatibility**: Existing databases with legacy types still work
- **Migration-Free**: No database migration required, transparent parsing
- **Type Safety**: Enum-based types catch errors at compile time

## Future Enhancements

Potential areas for improvement (not yet implemented):

1. **Connection Pooling**: DuckDB connection pool for high-frequency ingestion
2. **Structured Error Types**: Replace `anyhow::Error` with custom error enum
3. **API Versioning**: Version REST API endpoints for backward compatibility
4. **Metrics**: Prometheus metrics for query latency, ingestion rate
5. **Compression Tuning**: Experiment with DuckDB compression settings
6. **Query Caching**: Cache frequent time range queries
7. **Batch Inserts**: Group multiple TMIV fields into single transaction

## Troubleshooting

### "Text file busy" when updating binary
The recorder process is running. Stop it first:
```bash
pkill gaia-recorder
cp target/release/gaia-recorder <destination>
```

### Empty query results in playback mode
Check that session_id matches a recording in `--data-dir`:
```bash
curl http://localhost:8920/api/recordings/list | jq .
```

### Field name not found
Verify field name format includes `:raw` or `:conv` suffix:
- Correct: `SH.TI:conv`, `SH.TI:raw`
- Incorrect: `SH.TI`, `SH_TI@RAW`

For gRPC ingestion, use gRPC format (`SH_TI`, `SH_TI@RAW`). The service handles conversion.

## License

MPL-2.0

## References

- [DuckDB Documentation](https://duckdb.org/docs/)
- [Axum Web Framework](https://docs.rs/axum/)
- [Tonic gRPC](https://docs.rs/tonic/)
