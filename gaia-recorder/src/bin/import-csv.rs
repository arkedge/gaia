use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDateTime, Utc};
use clap::Parser;
use duckdb::{params, Connection};

#[derive(Parser, Debug)]
#[clap(author, version, about = "Import CSV telemetry and command logs to gaia-recorder database")]
struct Args {
    /// Path to the directory containing TLM/ and CMD/ subdirectories
    #[clap(short, long)]
    input_dir: PathBuf,

    /// Output database path
    #[clap(short, long)]
    output_db: PathBuf,

    /// Session ID (defaults to input directory name)
    #[clap(short, long)]
    session_id: Option<String>,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    // Create database
    let conn = Connection::open(&args.output_db)
        .context("Failed to create database")?;

    init_db(&conn)?;

    // Import telemetry
    let tlm_dir = args.input_dir.join("TLM");
    if tlm_dir.exists() {
        println!("Importing telemetry from {:?}", tlm_dir);
        import_telemetry_dir(&conn, &tlm_dir)?;
    }

    // Import commands
    let cmd_dir = args.input_dir.join("CMD");
    if cmd_dir.exists() {
        println!("Importing commands from {:?}", cmd_dir);
        import_command_dir(&conn, &cmd_dir)?;
    }

    // Create indexes after all data is inserted (much faster)
    create_indexes(&conn)?;

    // Optimize database
    println!("Optimizing database...");
    conn.execute("ANALYZE", [])?;
    println!("Import completed successfully!");
    println!("Database saved to: {:?}", args.output_db);

    // Show database size
    let metadata = std::fs::metadata(&args.output_db)?;
    println!("Database size: {:.2} MB", metadata.len() as f64 / 1024.0 / 1024.0);

    Ok(())
}

fn init_db(conn: &Connection) -> Result<()> {
    // DuckDB automatically uses compression; optimized data types reduce storage
    // Create tables without indexes first (indexes will be created after data insertion)
    // Schema must match gaia-recorder's main.rs schema exactly
    conn.execute(
        "CREATE SEQUENCE IF NOT EXISTS seq_telemetry_samples START 1",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS telemetry_samples (
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
        )",
        [],
    )?;

    conn.execute(
        "CREATE SEQUENCE IF NOT EXISTS seq_command_logs START 1",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS command_logs (
            id INTEGER PRIMARY KEY DEFAULT nextval('seq_command_logs'),
            time_ms BIGINT NOT NULL,
            command_name VARCHAR NOT NULL,
            params_json VARCHAR
        )",
        [],
    )?;

    Ok(())
}

fn create_indexes(conn: &Connection) -> Result<()> {
    println!("Creating indexes...");

    // Index must match gaia-recorder's main.rs
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_telemetry_query
         ON telemetry_samples(tmiv_name, field_name, is_raw, time_primary_ms)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_command_time
         ON command_logs(time_ms)",
        [],
    )?;

    Ok(())
}

fn import_telemetry_dir(conn: &Connection, tlm_dir: &Path) -> Result<()> {
    for entry in std::fs::read_dir(tlm_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("csv") {
            println!("  Processing telemetry file: {:?}", path.file_name());
            import_telemetry_csv(conn, &path)?;
        }
    }
    Ok(())
}

fn import_telemetry_csv(conn: &Connection, csv_path: &Path) -> Result<()> {
    // Extract TMIV name from filename: RT.MOBC.HK.csv -> RT.MOBC.HK
    let tmiv_name = csv_path
        .file_stem()
        .and_then(|s| s.to_str())
        .context("Invalid filename")?
        .to_string();

    // Step 1: Read CSV header to get column names
    let file = File::open(csv_path)?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines();
    let header = lines.next().context("Empty CSV file")??;
    let headers: Vec<&str> = header.split(',').collect();

    // Step 2: Create temporary table and load CSV using DuckDB native import
    let csv_path_str = csv_path.to_str().context("Invalid path")?;
    conn.execute(
        &format!(
            "CREATE TEMP TABLE temp_csv AS SELECT * FROM read_csv_auto('{}', header=true, all_varchar=true)",
            csv_path_str
        ),
        [],
    )?;

    println!("    Loaded CSV into temporary table");

    // Step 3: Transform and insert data using SQL
    // Build column transformation expressions for UNPIVOT
    let mut column_exprs = Vec::new();
    for (i, header) in headers.iter().enumerate() {
        if i == 0 {
            continue; // Skip timestamp column
        }

        // Convert field name format:
        // - Replace underscore with dot: SH_TI -> SH.TI
        // - Replace @RAW suffix with :raw: SH_TI@RAW -> SH.TI:raw
        // - Add :conv suffix if no @RAW: SH_TI -> SH.TI:conv
        let field_name_clean = if header.contains("@RAW") {
            header.replace("@RAW", ":raw").replace('_', ".")
        } else {
            format!("{}:conv", header.replace('_', "."))
        };

        let is_raw = if field_name_clean.ends_with(":raw") { 1 } else { 0 };

        // Use column name with quotes to handle special characters
        // Parse timestamp: Truncate nanoseconds to microseconds and remove space before timezone
        // Format: "2026-01-21 07:03:45.066818027 +00:00" (36 chars) -> "2026-01-21 07:03:45.066818+00:00" (32 chars)
        // Take first 26 chars (up to microseconds) + last 6 chars (timezone)
        column_exprs.push(format!(
            "SELECT
                '{tmiv_name}' as tmiv_name,
                '{field_name_clean}' as field_name,
                {is_raw} as is_raw,
                CAST(epoch_ms(CAST(
                    SUBSTRING(\"{}\"::VARCHAR, 1, 26) || SUBSTRING(\"{}\"::VARCHAR, LENGTH(\"{}\"::VARCHAR) - 5)
                AS TIMESTAMP)) AS BIGINT) as time_primary_ms,
                CAST(epoch_ms(CAST(
                    SUBSTRING(\"{}\"::VARCHAR, 1, 26) || SUBSTRING(\"{}\"::VARCHAR, LENGTH(\"{}\"::VARCHAR) - 5)
                AS TIMESTAMP)) AS BIGINT) as time_received_ms,
                CASE
                    WHEN \"{}\"::VARCHAR = '' THEN NULL
                    WHEN TRY_CAST(\"{}\"::VARCHAR AS BIGINT) IS NOT NULL THEN 'integer'
                    WHEN TRY_CAST(\"{}\"::VARCHAR AS DOUBLE) IS NOT NULL THEN 'double'
                    ELSE 'string'
                END as value_type,
                CASE
                    WHEN TRY_CAST(\"{}\"::VARCHAR AS DOUBLE) IS NOT NULL AND TRY_CAST(\"{}\"::VARCHAR AS BIGINT) IS NULL
                    THEN TRY_CAST(\"{}\"::VARCHAR AS DOUBLE)
                    ELSE NULL
                END as value_num,
                TRY_CAST(\"{}\"::VARCHAR AS BIGINT) as value_int,
                CASE
                    WHEN TRY_CAST(\"{}\"::VARCHAR AS BIGINT) IS NULL AND TRY_CAST(\"{}\"::VARCHAR AS DOUBLE) IS NULL AND \"{}\"::VARCHAR != ''
                    THEN \"{}\"::VARCHAR
                    ELSE NULL
                END as value_text,
                NULL as value_bytes
            FROM temp_csv
            WHERE \"{}\"::VARCHAR != ''",
            headers[0], headers[0], headers[0], headers[0], headers[0], headers[0], header, header, header, header, header, header, header, header, header, header, header, header
        ));
    }

    // Combine all columns with UNION ALL
    let insert_query = format!(
        "INSERT INTO telemetry_samples
         (tmiv_name, field_name, is_raw, time_primary_ms, time_received_ms, value_type, value_num, value_int, value_text, value_bytes)
         {}",
        column_exprs.join("\nUNION ALL\n")
    );

    // Execute the bulk insert
    let count = conn.execute(&insert_query, [])?;
    println!("    Inserted {} samples total", count);

    // Step 4: Drop temporary table
    conn.execute("DROP TABLE temp_csv", [])?;

    Ok(())
}


fn import_command_dir(conn: &Connection, cmd_dir: &Path) -> Result<()> {
    for entry in std::fs::read_dir(cmd_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("csv") {
            println!("  Processing command file: {:?}", path.file_name());
            import_command_csv(conn, &path)?;
        }
    }
    Ok(())
}

fn import_command_csv(conn: &Connection, csv_path: &Path) -> Result<()> {
    let file = File::open(csv_path)?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines();

    // Skip header
    lines.next();

    let mut tx = conn.unchecked_transaction()?;
    let mut count = 0;

    for line in lines {
        let line = line?;
        let parts: Vec<&str> = line.splitn(2, ',').collect();

        if parts.len() != 2 {
            continue;
        }

        let timestamp_str = parts[0];
        let command_str = parts[1];

        // Skip non-:cmd entries for Ocea logs
        let trimmed = command_str.trim();
        let without_prefix = trimmed.strip_prefix(".:").unwrap_or(trimmed);
        if !without_prefix.starts_with("cmd ") {
            continue;
        }

        // Parse timestamp (format: "2026-01-13 08:27:14.773")
        let time_ms = parse_command_timestamp(timestamp_str)?;

        // Parse command
        let (command_name, params_json) = parse_command(command_str);

        tx.execute(
            "INSERT INTO command_logs (time_ms, command_name, params_json)
             VALUES (?1, ?2, ?3)",
            params![time_ms, command_name, params_json],
        )?;

        count += 1;
        if count % 5000 == 0 {
            tx.commit()?;
            tx = conn.unchecked_transaction()?;
            print!("    Inserted {} commands\r", count);
        }
    }

    tx.commit()?;
    println!("    Inserted {} commands total", count);

    Ok(())
}

fn parse_command_timestamp(timestamp_str: &str) -> Result<i64> {
    // Format: "2026-01-13 08:27:14.773"
    let dt = NaiveDateTime::parse_from_str(timestamp_str, "%Y-%m-%d %H:%M:%S%.f")
        .context("Failed to parse command timestamp")?;
    let dt_utc = DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc);
    Ok(dt_utc.timestamp_millis())
}

fn parse_command(command_str: &str) -> (String, String) {
    // Examples:
    // ".:call integ_test_comet_mobc_emc ->"
    // ".:note テレメトリ初期化 -> "
    // ".:cmd MOBC TLM_MGR_INIT RT NOT_QUEUED -> SUCCESS"
    // ".:wait 1500ms ->"

    let trimmed = command_str.trim();

    // Remove leading ".:" if present
    let without_prefix = trimmed.strip_prefix(".:").unwrap_or(trimmed);

    // Split by " -> " to separate command from result
    let parts: Vec<&str> = without_prefix.splitn(2, " -> ").collect();
    let command_part = parts[0].trim();
    let result_part = if parts.len() > 1 { parts[1].trim() } else { "" };

    // Extract command type and name
    let tokens: Vec<&str> = command_part.split_whitespace().collect();
    let command_type = tokens.get(0).unwrap_or(&"unknown");
    let command_name = if tokens.len() > 1 {
        tokens[1..].join(" ")
    } else {
        command_part.to_string()
    };

    // Build JSON params
    let params_json = format!(
        r#"{{"type":"{}","args":"{}","result":"{}"}}"#,
        command_type,
        command_part,
        result_part
    );

    (format!("{}:{}", command_type, command_name), params_json)
}
