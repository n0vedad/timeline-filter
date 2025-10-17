use anyhow::Result;
use axum::{extract::State, response::IntoResponse, Json};
use serde_json::json;

use crate::errors::SupercellError;
use crate::timeline_storage;

use super::context::WebContext;

/// Handle describeFeedGenerator endpoint
///
/// Returns service DID and list of feeds hosted by this generator.
/// Required by AT Protocol for feed generator discovery.
///
/// Includes both Jetstream-based feeds and Timeline-based feeds.
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

    // Collect Jetstream feeds (from config.yml)
    let mut all_feeds: Vec<serde_json::Value> = web_context.feeds
        .keys()
        .map(|k| json!({"uri": k}))
        .collect();

    // Add Timeline feeds (from timeline_feeds.yml / database)
    if let Ok(timeline_feed_uris) = timeline_storage::get_all_feed_uris(&web_context.pool).await {
        all_feeds.extend(
            timeline_feed_uris
                .into_iter()
                .map(|uri| json!({"uri": uri}))
        );
    }

    Ok(Json(json!({
        "did": service_did,
        "feeds": all_feeds,
    })))
}
