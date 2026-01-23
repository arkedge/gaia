//! Database query operations for telemetry and commands

use anyhow::Result;
use duckdb::{params, Connection};
use serde::Serialize;
use std::path::Path;

use crate::domain::ValueType;

#[derive(Debug, Serialize, Clone)]
pub struct TelemetrySample {
    pub time_ms: i64,
    pub value_type: String,
    pub value_num: Option<f64>,
    pub value_int: Option<i64>,
    pub value_text: Option<String>,
    pub value_bytes_hex: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct CommandLogItem {
    pub time_ms: i64,
    pub command_name: String,
    pub params_json: String,
}

/// Query telemetry samples from the database
///
/// Note: The `_is_raw` parameter is currently unused as field_name already contains
/// the :raw or :conv suffix. This parameter may be removed in a future refactoring.
pub fn query_telemetry(
    db_path: &str,
    tmiv_name: &str,
    field_name: &str,
    _is_raw: bool,
    start_ms: i64,
    end_ms: i64,
    max_points: usize,
    time_axis: &str,
) -> Result<Vec<TelemetrySample>> {
    // Check if database file exists to prevent creating empty files
    if !Path::new(db_path).exists() {
        return Ok(Vec::new());
    }
    let conn = Connection::open(db_path)?;
    let time_column = if time_axis == "received" {
        "time_received_ms"
    } else {
        "time_primary_ms"
    };
    let mut stmt = conn.prepare(&format!(
        "SELECT {time_column}, value_type, value_num, value_int, value_text, value_bytes
         FROM telemetry_samples
         WHERE tmiv_name = ?1 AND field_name = ?2 AND {time_column} BETWEEN ?3 AND ?4
         ORDER BY {time_column} ASC"
    ))?;

    let rows = stmt.query_map(
        params![tmiv_name, field_name, start_ms, end_ms],
        |row| {
            let time_ms: i64 = row.get(0)?;
            let value_type_raw: String = row.get(1)?;
            // Normalize value_type using ValueType enum (supports legacy formats)
            let value_type = ValueType::from_db_string(&value_type_raw)
                .to_db_string()
                .to_string();
            let value_num: Option<f64> = row.get(2)?;
            let value_int: Option<i64> = row.get(3)?;
            let value_text: Option<String> = row.get(4)?;
            let value_bytes: Option<Vec<u8>> = row.get(5)?;
            let value_bytes_hex = value_bytes.as_ref().map(|bytes| {
                bytes
                    .iter()
                    .map(|b| format!("{:02x}", b))
                    .collect::<String>()
            });
            Ok(TelemetrySample {
                time_ms,
                value_type,
                value_num,
                value_int,
                value_text,
                value_bytes_hex,
            })
        },
    )?;

    let mut samples: Vec<TelemetrySample> = rows.collect::<std::result::Result<_, _>>()?;
    if samples.len() > max_points && max_points > 0 {
        samples = downsample_samples(samples, max_points);
    }
    Ok(samples)
}

fn downsample_samples(samples: Vec<TelemetrySample>, max_points: usize) -> Vec<TelemetrySample> {
    let has_numeric = samples
        .iter()
        .any(|sample| sample.value_num.is_some() || sample.value_int.is_some());
    if !has_numeric {
        return downsample_stride(samples, max_points);
    }
    downsample_min_max_avg(samples, max_points)
}

fn downsample_stride(samples: Vec<TelemetrySample>, max_points: usize) -> Vec<TelemetrySample> {
    if samples.len() <= max_points {
        return samples;
    }
    let step = (samples.len() as f64 / max_points as f64).ceil() as usize;
    samples
        .into_iter()
        .step_by(step.max(1))
        .take(max_points)
        .collect()
}

fn downsample_min_max_avg(
    samples: Vec<TelemetrySample>,
    max_points: usize,
) -> Vec<TelemetrySample> {
    if samples.len() <= max_points {
        return samples;
    }
    let buckets = max_points / 3;
    if buckets == 0 {
        return samples;
    }
    let bucket_size = (samples.len() as f64 / buckets as f64).ceil() as usize;
    let mut downsampled = Vec::with_capacity(max_points);

    for chunk in samples.chunks(bucket_size) {
        let mut min: Option<f64> = None;
        let mut max: Option<f64> = None;
        let mut sum = 0.0;
        let mut count = 0.0;
        for sample in chunk.iter() {
            let value = sample.value_num.or(sample.value_int.map(|v| v as f64));
            let Some(value) = value else {
                continue;
            };
            sum += value;
            count += 1.0;
            min = Some(min.map_or(value, |m| m.min(value)));
            max = Some(max.map_or(value, |m| m.max(value)));
        }
        if count == 0.0 {
            continue;
        }
        let avg = sum / count;
        let time_ms = chunk[chunk.len() / 2].time_ms;
        let make_sample = |value: f64| TelemetrySample {
            time_ms,
            value_type: "double".to_string(),
            value_num: Some(value),
            value_int: None,
            value_text: None,
            value_bytes_hex: None,
        };
        if let Some(min) = min {
            downsampled.push(make_sample(min));
        }
        if let Some(max) = max {
            downsampled.push(make_sample(max));
        }
        downsampled.push(make_sample(avg));
        if downsampled.len() >= max_points {
            break;
        }
    }

    downsampled.truncate(max_points);
    downsampled
}

/// Query command logs from the database
pub fn query_commands(
    db_path: &str,
    start_ms: i64,
    end_ms: i64,
    max_points: usize,
) -> Result<Vec<CommandLogItem>> {
    let conn = Connection::open(db_path)?;
    let mut stmt = conn.prepare(
        "SELECT time_ms, command_name, params_json
         FROM command_logs
         WHERE time_ms BETWEEN ?1 AND ?2
         ORDER BY time_ms ASC
         LIMIT ?3",
    )?;

    let rows = stmt.query_map(params![start_ms, end_ms, max_points as i64], |row| {
        Ok(CommandLogItem {
            time_ms: row.get(0)?,
            command_name: row.get(1)?,
            params_json: row.get(2)?,
        })
    })?;

    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

/// Get the time range (min/max) of telemetry samples in the database
pub fn query_time_range(db_path: &str) -> Result<(Option<i64>, Option<i64>)> {
    let conn = Connection::open(db_path)?;
    let mut stmt =
        conn.prepare("SELECT MIN(time_primary_ms), MAX(time_primary_ms) FROM telemetry_samples")?;

    let result = stmt.query_row([], |row| Ok((row.get(0).ok(), row.get(1).ok())))?;

    Ok(result)
}
