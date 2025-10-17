use anyhow::anyhow;
use axum::{extract::State, response::IntoResponse, Json};
use axum_extra::extract::Query;
use serde::{Deserialize, Serialize};

use crate::errors::SupercellError;
use crate::timeline_storage;

use super::context::WebContext;

#[derive(Deserialize, Default)]
pub struct FeedParams {
    pub feed: Option<String>,
    pub limit: Option<u16>,
    pub cursor: Option<String>,
}

#[derive(Serialize)]
pub struct FeedItemView {
    pub post: String,
}

#[derive(Serialize)]
pub struct FeedItemsView {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    pub feed: Vec<FeedItemView>,
}

pub async fn handle_get_feed_skeleton(
    State(web_context): State<WebContext>,
    Query(feed_params): Query<FeedParams>,
) -> Result<impl IntoResponse, SupercellError> {
    if feed_params.feed.is_none() {
        return Err(anyhow!("feed parameter is required").into());
    }
    let feed_uri = feed_params.feed.unwrap();

    // Get timeline feed posts from database
    let limit = feed_params.limit.unwrap_or(50).min(100) as u32;
    let posts = timeline_storage::get_feed_posts(
        &web_context.pool,
        &feed_uri,
        limit,
        feed_params.cursor.clone(),
    )
    .await
    .map_err(|e| {
        tracing::error!(error = ?e, "Failed to get timeline feed posts");
        anyhow!("Failed to get feed posts")
    })?;

    let offset = feed_params.cursor
        .and_then(|c| c.parse::<u32>().ok())
        .unwrap_or(0);

    let next_cursor = if posts.is_empty() {
        None
    } else {
        Some((offset + posts.len() as u32).to_string())
    };

    let feed_item_views = posts
        .iter()
        .map(|feed_item| FeedItemView {
            post: feed_item.clone(),
        })
        .collect::<Vec<_>>();

    Ok(Json(FeedItemsView {
        cursor: next_cursor,
        feed: feed_item_views,
    })
    .into_response())
}
