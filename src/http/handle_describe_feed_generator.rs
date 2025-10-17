use anyhow::Result;
use axum::{extract::State, response::IntoResponse, Json};
use serde_json::json;

use crate::errors::SupercellError;
use crate::timeline_storage;

use super::context::WebContext;

/// Handle describeFeedGenerator endpoint
///
/// Returns service DID and list of Timeline feeds hosted by this generator.
/// Required by AT Protocol for feed generator discovery.
///
/// Response format:
/// ```json
/// {
///   "did": "did:web:hostname",
///   "feeds": [{"uri": "at://did/app.bsky.feed.generator/rkey"}]
/// }
/// ```
pub async fn handle_describe_feed_generator(
    State(web_context): State<WebContext>,
) -> Result<impl IntoResponse, SupercellError> {
    // Construct service DID from external_base
    // Format: did:web:hostname (strip https:// and trailing slashes)
    let hostname = web_context.external_base
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/');

    let service_did = format!("did:web:{}", hostname);

    // Get Timeline feeds from database
    let all_feeds: Vec<serde_json::Value> = timeline_storage::get_all_feed_uris(&web_context.pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|uri| json!({"uri": uri}))
        .collect();

    Ok(Json(json!({
        "did": service_did,
        "feeds": all_feeds,
    })))
}
