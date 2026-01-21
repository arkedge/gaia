use duckdb::{Connection, Result};
use std::env;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <database_path>", args[0]);
        std::process::exit(1);
    }

    let db_path = &args[1];
    let conn = Connection::open(db_path)?;

    println!("=== All fields in database ===");
    let mut stmt = conn.prepare(
        "SELECT DISTINCT tmiv_name, field_name, is_raw
         FROM telemetry_samples
         ORDER BY tmiv_name, field_name, is_raw"
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i32>(2)?,
        ))
    })?;

    for row in rows {
        let (tmiv_name, field_name, is_raw) = row?;
        // field_name already contains :conv or :raw suffix, don't add it again
        println!("{}:{} (is_raw={})",  tmiv_name, field_name, is_raw);
    }

    Ok(())
}
