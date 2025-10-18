use anyhow::Result;
use axum::{response::IntoResponse, Json};
use serde_json::json;

use crate::errors::TimelineFilterError;

pub async fn handle_index() -> Result<impl IntoResponse, TimelineFilterError> {
    Ok(Json(json!({"ok": true})).into_response())
}
