//! Database insert operations for telemetry and commands

use anyhow::Result;
use duckdb::{params, Connection, Transaction};
use gaia_stub::tco_tmiv::{tco_param, Tco, TmivField};

use crate::domain::ValueType;
use crate::transform::FieldName;

/// Insert a telemetry field sample into the database
pub fn insert_telemetry_sample(
    tx: &Transaction<'_>,
    tmiv_name: &str,
    field: &TmivField,
    time_primary_ms: i64,
    time_received_ms: i64,
) -> Result<()> {
    // Store field_name with :raw or :conv suffix (same format as CSV import)
    let field_name_parsed = FieldName::from_grpc(&field.name);
    let field_name = field_name_parsed.to_db_format();
    let is_raw = field_name_parsed.is_raw_int();

    let mut value_num: Option<f64> = None;
    let mut value_int: Option<i64> = None;
    let mut value_text: Option<String> = None;
    let mut value_bytes: Option<Vec<u8>> = None;
    let value_type: ValueType;

    match field.value.as_ref() {
        Some(gaia_stub::tco_tmiv::tmiv_field::Value::Integer(i)) => {
            value_type = ValueType::Integer;
            value_int = Some(*i);
            value_num = Some(*i as f64);
        }
        Some(gaia_stub::tco_tmiv::tmiv_field::Value::Double(d)) => {
            value_type = ValueType::Double;
            value_num = Some(*d);
        }
        Some(gaia_stub::tco_tmiv::tmiv_field::Value::Enum(e)) => {
            value_type = ValueType::Enum;
            value_text = Some(e.clone());
        }
        Some(gaia_stub::tco_tmiv::tmiv_field::Value::String(s)) => {
            value_type = ValueType::String;
            value_text = Some(s.clone());
        }
        Some(gaia_stub::tco_tmiv::tmiv_field::Value::Bytes(b)) => {
            value_type = ValueType::Bytes;
            value_bytes = Some(b.clone());
            if b.len() <= 8 {
                let mut buf = [0u8; 8];
                buf[8 - b.len()..].copy_from_slice(b);
                let raw = u64::from_be_bytes(buf) as i64;
                value_int = Some(raw);
                value_num = Some(raw as f64);
            }
        }
        None => {
            value_type = ValueType::Unknown;
        }
    }

    tx.execute(
        "INSERT INTO telemetry_samples (tmiv_name, field_name, is_raw, time_primary_ms, time_received_ms, value_type, value_num, value_int, value_text, value_bytes)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            tmiv_name,
            field_name,
            is_raw,
            time_primary_ms,
            time_received_ms,
            value_type.to_db_string(),
            value_num,
            value_int,
            value_text,
            value_bytes,
        ],
    )?;

    Ok(())
}

/// Insert a command log entry into the database
pub fn insert_command_log(
    conn: &Connection,
    time_ms: i64,
    command_name: &str,
    params_json: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO command_logs (time_ms, command_name, params_json) VALUES (?1, ?2, ?3)",
        params![time_ms, command_name, params_json],
    )?;
    Ok(())
}

/// Build JSON representation of TCO command parameters
pub fn build_params_json(tco: &Tco) -> String {
    let params: Vec<serde_json::Value> = tco
        .params
        .iter()
        .map(|param| {
            let value = match param.value.as_ref() {
                Some(tco_param::Value::Integer(v)) => {
                    serde_json::json!({"type": "integer", "value": v})
                }
                Some(tco_param::Value::Double(v)) => {
                    serde_json::json!({"type": "double", "value": v})
                }
                Some(tco_param::Value::Bytes(v)) => {
                    let hex = v.iter().map(|b| format!("{:02x}", b)).collect::<String>();
                    serde_json::json!({"type": "bytes", "value": hex})
                }
                None => serde_json::json!({"type": "none"}),
            };
            serde_json::json!({"name": param.name, "value": value})
        })
        .collect();

    serde_json::json!({"name": tco.name, "params": params}).to_string()
}
