use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDateTime, Utc};
use clap::Parser;
use rusqlite::{params, Connection};

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
    // Enable optimizations (use execute_batch for PRAGMAs)
    conn.execute_batch(
        "PRAGMA journal_mode = OFF;
         PRAGMA synchronous = OFF;
         PRAGMA page_size = 4096;
         PRAGMA cache_size = -64000;"  // 64MB cache
    )?;

    // Create tables without indexes first (indexes will be created after data insertion)
    // Schema must match gaia-recorder's main.rs schema exactly
    conn.execute(
        "CREATE TABLE IF NOT EXISTS telemetry_samples (
            id INTEGER PRIMARY KEY,
            tmiv_name TEXT NOT NULL,
            field_name TEXT NOT NULL,
            is_raw INTEGER NOT NULL,
            time_primary_ms INTEGER NOT NULL,
            time_received_ms INTEGER NOT NULL,
            value_type TEXT NOT NULL,
            value_num REAL,
            value_int INTEGER,
            value_text TEXT,
            value_bytes BLOB
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS command_logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            time_ms INTEGER NOT NULL,
            command_name TEXT NOT NULL,
            params_json TEXT
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

    let file = File::open(csv_path)?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines();

    // Parse header
    let header = lines.next().context("Empty CSV file")??;
    let headers: Vec<&str> = header.split(',').collect();

    let mut tx = conn.unchecked_transaction()?;
    let mut count = 0;

    for line in lines {
        let line = line?;
        let values: Vec<&str> = line.split(',').collect();

        if values.len() != headers.len() {
            continue;
        }

        // Parse timestamp
        let timestamp_str = values[0];
        let time_ms = parse_telemetry_timestamp(timestamp_str)?;

        // Insert each field
        for i in 1..headers.len() {
            let field_name = headers[i];
            let value = values[i];

            // Convert field name format:
            // - Replace underscore with dot: SH_TI -> SH.TI
            // - Replace @RAW suffix with :raw: SH_TI@RAW -> SH.TI:raw
            // - Add :conv suffix if no @RAW: SH_TI -> SH.TI:conv
            let field_name_clean = if field_name.contains("@RAW") {
                field_name
                    .replace("@RAW", ":raw")
                    .replace('_', ".")
            } else {
                format!("{}:conv", field_name.replace('_', "."))
            };

            // Determine value type and parse
            if value.is_empty() {
                continue;
            }

            let (value_type, value_num, value_int, value_text) =
                parse_telemetry_value(value, field_name);

            // Determine is_raw flag based on field name
            let is_raw = if field_name_clean.ends_with(":raw") { 1 } else { 0 };

            // time_received_ms is same as time_primary_ms for CSV imports
            let time_received_ms = time_ms;

            tx.execute(
                "INSERT INTO telemetry_samples
                 (tmiv_name, field_name, is_raw, time_primary_ms, time_received_ms, value_type, value_num, value_int, value_text, value_bytes)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    &tmiv_name,
                    &field_name_clean,
                    is_raw,
                    time_ms,
                    time_received_ms,
                    &value_type,
                    value_num,
                    value_int,
                    value_text,
                    None::<Vec<u8>>,
                ],
            )?;

            count += 1;
            if count % 50000 == 0 {
                tx.commit()?;
                tx = conn.unchecked_transaction()?;
                print!("    Inserted {} samples\r", count);
            }
        }
    }

    tx.commit()?;
    println!("    Inserted {} samples total", count);

    Ok(())
}

fn parse_telemetry_timestamp(timestamp_str: &str) -> Result<i64> {
    // Format: "2026-01-13 08:23:58.728680890 +00:00"
    // Parse with chrono
    let dt = DateTime::parse_from_str(timestamp_str, "%Y-%m-%d %H:%M:%S%.f %z")
        .context("Failed to parse timestamp")?;
    Ok(dt.timestamp_millis())
}

fn parse_telemetry_value(value: &str, _field_name: &str) -> (String, Option<f64>, Option<i64>, Option<String>) {
    // Check if it's a RAW field (ends with @RAW)
    // let _is_raw = field_name.ends_with("@RAW");

    // Try to parse as number
    if let Ok(num) = value.parse::<i64>() {
        return ("int".to_string(), None, Some(num), None);
    }

    if let Ok(num) = value.parse::<f64>() {
        return ("num".to_string(), Some(num), None, None);
    }

    // Otherwise it's text
    ("text".to_string(), None, None, Some(value.to_string()))
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
