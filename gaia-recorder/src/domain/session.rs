//! Session management for telemetry recording

use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use chrono::Utc;
use serde::Serialize;
use tokio::sync::RwLock;

use crate::db;

/// Recording session information
#[derive(Clone, Debug, Serialize)]
pub struct SessionInfo {
    pub session_id: String,
    pub suffix: String,
    pub started_at_ms: i64,
    pub db_path: String,
    pub active: bool,
}

/// Recording file metadata
#[derive(Clone, Debug, Serialize)]
pub struct RecordingInfo {
    pub session_id: String,
    pub suffix: String,
    pub started_at_ms: Option<i64>,
    pub db_path: String,
}

/// Session management operations
pub struct Session;

impl Session {
    /// Create a new recording session with database initialization
    ///
    /// # Arguments
    /// * `data_dir` - Directory to store the database file
    /// * `suffix` - Optional suffix for the session file name
    ///
    /// # Returns
    /// SessionInfo with session_id, db_path, and timestamps
    pub async fn create(data_dir: &Path, suffix: Option<String>) -> Result<SessionInfo> {
        let suffix = suffix.unwrap_or_default();
        let started_at = Utc::now();
        let session_id = started_at.format("%Y%m%d_%H%M%S").to_string();

        let file_name = if suffix.is_empty() {
            format!("recording_{session_id}.duckdb")
        } else {
            format!("recording_{session_id}_{suffix}.duckdb")
        };

        let db_path = data_dir.join(file_name);
        let db_path_string = db_path.to_string_lossy().to_string();

        // Initialize database schema
        tokio::task::spawn_blocking({
            let db_path = db_path.clone();
            move || db::init_database(&db_path)
        })
        .await??;

        Ok(SessionInfo {
            session_id,
            suffix,
            started_at_ms: started_at.timestamp_millis(),
            db_path: db_path_string,
            active: true,
        })
    }

    /// Resolve session_id to database path
    ///
    /// If session_id is provided, searches in data_dir.
    /// If session_id is None, returns current session path from state.
    pub async fn resolve_path<T>(
        data_dir: &Path,
        session_id: Option<String>,
        current_session: Option<&T>,
    ) -> Option<String>
    where
        T: AsRef<SessionInfo>,
    {
        if let Some(session_id) = session_id {
            let data_dir = data_dir.to_path_buf();
            let list = tokio::task::spawn_blocking(move || Self::list_all(&data_dir))
                .await
                .ok()
                .and_then(|res| res.ok())?;
            let item = list
                .into_iter()
                .find(|item| item.session_id == session_id)?;
            return Some(item.db_path);
        }
        current_session.map(|s| s.as_ref().db_path.clone())
    }

    /// List all recording files in data directory
    ///
    /// Scans for `recording_*.duckdb` files and parses session_id and suffix.
    pub fn list_all(data_dir: &Path) -> Result<Vec<RecordingInfo>> {
        let mut recordings = Vec::new();

        for entry in std::fs::read_dir(data_dir)? {
            let entry = entry?;
            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !file_name.starts_with("recording_") || !file_name.ends_with(".duckdb") {
                continue;
            }

            let trimmed = file_name
                .trim_start_matches("recording_")
                .trim_end_matches(".duckdb");

            // Split into at most 3 parts: YYYYMMDD, HHMMSS, and optional suffix
            let parts: Vec<&str> = trimmed.splitn(3, '_').collect();
            let (session_id, suffix) = if parts.len() >= 2 {
                // We have at least YYYYMMDD_HHMMSS
                let session_id = format!("{}_{}", parts[0], parts[1]);
                let suffix = if parts.len() >= 3 {
                    parts[2].to_string()
                } else {
                    String::new()
                };
                (session_id, suffix)
            } else {
                // Fallback for unexpected format
                (trimmed.to_string(), String::new())
            };

            let started_at_ms =
                chrono::NaiveDateTime::parse_from_str(&session_id, "%Y%m%d_%H%M%S")
                    .ok()
                    .map(|dt| {
                        chrono::DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc)
                            .timestamp_millis()
                    });

            recordings.push(RecordingInfo {
                session_id,
                suffix,
                started_at_ms,
                db_path: path.to_string_lossy().to_string(),
            });
        }

        // Sort by session_id descending (newest first)
        recordings.sort_by(|a, b| b.session_id.cmp(&a.session_id));

        Ok(recordings)
    }
}

impl AsRef<SessionInfo> for SessionInfo {
    fn as_ref(&self) -> &SessionInfo {
        self
    }
}

/// Helper to resolve session path from state
pub async fn resolve_session_path_from_state<S>(
    state: &Arc<RwLock<S>>,
    session_id: Option<String>,
) -> Option<String>
where
    S: SessionState,
{
    let (data_dir, current_session) = {
        let guard = state.read().await;
        (guard.data_dir().to_path_buf(), guard.current_session())
    };

    Session::resolve_path(&data_dir, session_id, current_session.as_ref()).await
}

/// Trait for state that contains session information
pub trait SessionState {
    fn data_dir(&self) -> &Path;
    fn current_session(&self) -> Option<SessionInfo>;
}
