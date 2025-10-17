use anyhow::Result;
use axum::{extract::State, response::IntoResponse, Json};
use serde_json::json;

use crate::errors::SupercellError;

use super::context::WebContext;

pub async fn handle_well_known(
    State(web_context): State<WebContext>,
) -> Result<impl IntoResponse, SupercellError> {
    // Strip protocol from external_base for DID (did:web doesn't include protocol)
    let hostname = web_context.external_base
        .trim_start_matches("https://")
        .trim_start_matches("http://");

    // Ensure serviceEndpoint has https:// protocol
    let service_endpoint = if web_context.external_base.starts_with("http://") || web_context.external_base.starts_with("https://") {
        web_context.external_base.clone()
    } else {
        format!("https://{}", web_context.external_base)
    };

    Ok(Json(json!({
         "@context": ["https://www.w3.org/ns/did/v1"],
         "id": format!("did:web:{}", hostname),
         "service": [
            {
                "id": "#bsky_fg",
                "type": "BskyFeedGenerator",
                "serviceEndpoint": service_endpoint,
            }
         ]
    }))
    .into_response())
}
