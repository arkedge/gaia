//! Database schema initialization

use anyhow::Result;
use duckdb::Connection;
use std::path::Path;

/// Initialize database schema for telemetry and command logging
pub fn init_database(path: &Path) -> Result<()> {
    let conn = Connection::open(path)?;

    // DuckDB automatically uses compression; optimized data types reduce storage
    conn.execute_batch(
        "
        CREATE SEQUENCE IF NOT EXISTS seq_telemetry_samples START 1;
        CREATE TABLE IF NOT EXISTS telemetry_samples (
            id INTEGER PRIMARY KEY DEFAULT nextval('seq_telemetry_samples'),
            tmiv_name VARCHAR NOT NULL,
            field_name VARCHAR NOT NULL,
            is_raw TINYINT NOT NULL,
            time_primary_ms BIGINT NOT NULL,
            time_received_ms BIGINT NOT NULL,
            value_type VARCHAR(20) NOT NULL,
            value_num DOUBLE,
            value_int BIGINT,
            value_text VARCHAR,
            value_bytes BLOB
        );
        CREATE INDEX IF NOT EXISTS idx_telemetry_query
            ON telemetry_samples (tmiv_name, field_name, is_raw, time_primary_ms);

        CREATE SEQUENCE IF NOT EXISTS seq_command_logs START 1;
        CREATE TABLE IF NOT EXISTS command_logs (
            id INTEGER PRIMARY KEY DEFAULT nextval('seq_command_logs'),
            time_ms BIGINT NOT NULL,
            command_name VARCHAR NOT NULL,
            params_json VARCHAR NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_command_time ON command_logs (time_ms);
        ",
    )?;
    Ok(())
}
