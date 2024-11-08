use anyhow::{anyhow, Context, Result};
use axum::{extract::State, response::IntoResponse, Json};
use axum_extra::extract::Query;
use base64::{engine::general_purpose, Engine as _};
use chrono::Utc;
use http::{HeaderMap, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::errors::SupercellError;
use crate::storage::feed_content_paginate;
use crate::storage::{verification_method_get, StoragePool};

use crate::crypto::{validate, JwtClaims, JwtHeader};

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
    headers: HeaderMap,
) -> Result<impl IntoResponse, SupercellError> {
    if feed_params.feed.is_none() {
        return Err(anyhow!("feed parameter is required").into());
    }
    let feed_uri = feed_params.feed.unwrap();

    let feed_control = web_context.feeds.get(&feed_uri);
    if feed_control.is_none() {
        return Ok((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "UnknownFeed",
                "message": "unknown feed",
            })),
        )
            .into_response());
    }

    let feed_control = feed_control.unwrap();

    if !feed_control.allowed.is_empty() {
        let authorization = headers.get("Authorization").and_then(|value| {
            value
                .to_str()
                .map(|inner_value| inner_value.to_string())
                .ok()
        });

        let did = did_from_jwt(&web_context.pool, &web_context.external_base, authorization).await;

        if let Err(err) = did {
            tracing::info!(error = ?err, "failed to validate JWT");
            return Ok(Json(FeedItemsView {
                cursor: None,
                feed: feed_control
                    .deny
                    .as_ref()
                    .map(|value| {
                        vec![FeedItemView {
                            post: value.clone(),
                        }]
                    })
                    .unwrap_or(vec![]),
            })
            .into_response());
        }

        let did = did.unwrap();

        if !feed_control.allowed.contains(&did) {
            return Ok(Json(FeedItemsView {
                cursor: None,
                feed: feed_control
                    .deny
                    .as_ref()
                    .map(|value| {
                        vec![FeedItemView {
                            post: value.clone(),
                        }]
                    })
                    .unwrap_or(vec![]),
            })
            .into_response());
        }
    }

    let parsed_cursor = parse_cursor(feed_params.cursor);
    let feed_items = feed_content_paginate(
        &web_context.pool,
        &feed_uri,
        feed_params.limit,
        parsed_cursor,
    )
    .await?;

    let cursor = feed_items
        .iter()
        .last()
        .map(|last_feed_item| last_feed_item.indexed_at.to_string());

    let feed_item_views = feed_items
        .iter()
        .map(|feed_item| FeedItemView {
            post: feed_item.uri.clone(),
        })
        .collect::<Vec<_>>();

    Ok(Json(FeedItemsView {
        cursor,
        feed: feed_item_views,
    })
    .into_response())
}

pub fn split_token(token: &str) -> Result<[&str; 3]> {
    let mut components = token.split('.');
    let header = components.next().ok_or(anyhow!("missing header"))?;
    let claims = components.next().ok_or(anyhow!("missing claims"))?;
    let signature = components.next().ok_or(anyhow!("missing signature"))?;

    if components.next().is_some() {
        return Err(anyhow!("invalid token"));
    }

    Ok([header, claims, signature])
}

async fn did_from_jwt(
    pool: &StoragePool,
    external_base: &str,
    authorization: Option<String>,
) -> Result<String> {
    let jwt = authorization
        .and_then(|value| {
            value
                .strip_prefix("Bearer ")
                .map(|inner_value| inner_value.to_string())
        })
        .ok_or(anyhow!("missing authorization"))?;
    let [header_part, claims_part, signature_part] = split_token(&jwt)?;

    let header: JwtHeader = {
        let content = general_purpose::URL_SAFE_NO_PAD
            .decode(header_part)
            .context("unable to base64 decode content")?;
        serde_json::from_slice(&content).context("unable to deserialize object")?
    };
    let claims: JwtClaims = {
        let content = general_purpose::URL_SAFE_NO_PAD
            .decode(claims_part)
            .context("unable to base64 decode content")?;
        serde_json::from_slice(&content).context("unable to deserialize object")?
    };

    let now = Utc::now();
    let now = now.timestamp() as i32;

    if header.alg != "ES256K" {
        return Err(anyhow!("unsupported algorithm"));
    }
    if claims.lxm != "app.bsky.feed.getFeedSkeleton" {
        return Err(anyhow!("invalid resource"));
    }
    if claims.aud != format!("did:web:{}", external_base) {
        return Err(anyhow!("invalid audience"));
    }
    if claims.exp < now {
        return Err(anyhow!("token expired"));
    }
    if claims.iat > now {
        return Err(anyhow!("token issued in the future"));
    }

    let multibase = verification_method_get(pool, &claims.iss).await?;
    if multibase.is_none() {
        return Err(anyhow!("verification method not found"));
    }
    let multibase = multibase.unwrap();

    let signature = general_purpose::URL_SAFE_NO_PAD
        .decode(signature_part)
        .context("invalid signature")?;
    let signature: &[u8] = &signature;

    let content = format!("{}.{}", header_part, claims_part);

    validate(&multibase, signature, &content)?;

    Ok(claims.iss)
}

fn parse_cursor(value: Option<String>) -> Option<i64> {
    let value = value.as_ref()?;

    let parts = value.split(",").collect::<Vec<&str>>();

    parts.first().and_then(|value| value.parse::<i64>().ok())
}
