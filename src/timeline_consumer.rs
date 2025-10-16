use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use serde::Deserialize;
use tokio_util::sync::CancellationToken;
use tracing;

use crate::storage::{feed_content_upsert, model::FeedContent, StoragePool};
use crate::timeline_config::{FilterConfig, TimelineFeed, TimelineFeeds};
use crate::timeline_storage;

/// Timeline Consumer Task
/// Polls getTimeline() for each configured user and indexes filtered posts
pub struct TimelineConsumerTask {
    pool: StoragePool,
    config: TimelineConsumerConfig,
    http_client: reqwest::Client,
    cancellation_token: CancellationToken,
}

/// Configuration for the Timeline Consumer
pub struct TimelineConsumerConfig {
    pub timeline_feeds: TimelineFeeds,
    pub default_poll_interval: Duration,
    pub user_agent: String,
}

impl TimelineConsumerTask {
    /// Create a new Timeline Consumer Task
    pub fn new(
        pool: StoragePool,
        config: TimelineConsumerConfig,
        cancellation_token: CancellationToken,
    ) -> Result<Self> {
        // Sync config to database on startup
        let feeds_clone = config.timeline_feeds.clone();
        let pool_clone = pool.clone();

        tokio::spawn(async move {
            if let Err(e) = timeline_storage::sync_config_to_db(&pool_clone, &feeds_clone).await {
                tracing::error!(error = ?e, "Failed to sync timeline config to database");
            }
        });

        let http_client = reqwest::Client::builder()
            .user_agent(&config.user_agent)
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .context("Failed to build HTTP client")?;

        Ok(Self {
            pool,
            config,
            http_client,
            cancellation_token,
        })
    }

    /// Run the background polling loop
    pub async fn run_background(&self) -> Result<()> {
        tracing::info!(
            user_count = self.config.timeline_feeds.len(),
            "TimelineConsumerTask started"
        );

        if self.config.timeline_feeds.is_empty() {
            tracing::warn!("No timeline feeds configured, consumer will idle");
        }

        loop {
            tokio::select! {
                () = self.cancellation_token.cancelled() => {
                    tracing::info!("TimelineConsumerTask cancelled");
                    break;
                },
                () = self.poll_cycle() => {},
            }
        }

        Ok(())
    }

    /// Execute one polling cycle for all users
    async fn poll_cycle(&self) {
        for timeline_feed in &self.config.timeline_feeds.timeline_feeds {
            let interval = timeline_feed
                .poll_interval_duration()
                .unwrap_or(self.config.default_poll_interval);

            // Check if enough time has passed since last poll
            match timeline_storage::should_poll(&self.pool, &timeline_feed.did, interval).await {
                Ok(true) => {
                    if let Err(e) = self.poll_user_timeline(timeline_feed).await {
                        tracing::error!(
                            user_did = %timeline_feed.did,
                            error = ?e,
                            "Failed to poll timeline"
                        );
                    }
                }
                Ok(false) => {
                    // Not time to poll yet
                    tracing::trace!(
                        user_did = %timeline_feed.did,
                        "Skipping poll - not enough time elapsed"
                    );
                }
                Err(e) => {
                    tracing::error!(
                        user_did = %timeline_feed.did,
                        error = ?e,
                        "Failed to check poll status"
                    );
                }
            }
        }

        // Sleep briefly before next cycle to avoid tight loop
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    }

    /// Poll timeline for a single user
    async fn poll_user_timeline(&self, feed: &TimelineFeed) -> Result<()> {
        tracing::debug!(
            user_did = %feed.did,
            feed_uri = %feed.feed_uri,
            "Polling timeline"
        );

        // 1. Get last cursor from database
        let cursor = timeline_storage::get_cursor(&self.pool, &feed.did)
            .await
            .context("Failed to get cursor")?;

        if let Some(ref cursor) = cursor {
            tracing::trace!(
                user_did = %feed.did,
                cursor = %cursor,
                "Using cursor from database"
            );
        }

        // 2. Fetch timeline from AT Protocol
        let timeline = self
            .fetch_timeline(feed, cursor, feed.max_posts_per_poll)
            .await
            .context("Failed to fetch timeline")?;

        // 3. Filter posts based on user's filter config
        let filtered = self.filter_posts(&timeline.feed, &feed.filters);

        tracing::info!(
            user_did = %feed.did,
            total = timeline.feed.len(),
            filtered = filtered.len(),
            blocked = timeline.feed.len() - filtered.len(),
            "Processed timeline posts"
        );

        // 4. Index filtered posts into feed_content table
        let mut indexed_count = 0;
        for post_view in filtered {
            let indexed_at = parse_indexed_at(&post_view.post.indexed_at)
                .with_context(|| {
                    format!("Failed to parse indexedAt for {}", post_view.post.uri)
                })?;

            if let Err(e) = feed_content_upsert(
                &self.pool,
                &FeedContent {
                    feed_id: feed.feed_uri.clone(),
                    uri: post_view.post.uri.clone(),
                    indexed_at,
                    score: 1,
                },
            )
            .await
            {
                tracing::error!(
                    uri = %post_view.post.uri,
                    error = ?e,
                    "Failed to index post"
                );
            } else {
                indexed_count += 1;
            }
        }

        // 5. Update cursor and poll state in database
        timeline_storage::update_poll_state(
            &self.pool,
            &feed.did,
            timeline.cursor.as_deref(),
            indexed_count,
        )
        .await
        .context("Failed to update poll state")?;

        tracing::debug!(
            user_did = %feed.did,
            indexed = indexed_count,
            new_cursor = ?timeline.cursor,
            "Successfully completed poll"
        );

        Ok(())
    }

    /// Fetch timeline from AT Protocol getTimeline endpoint
    async fn fetch_timeline(
        &self,
        feed: &TimelineFeed,
        cursor: Option<String>,
        limit: u32,
    ) -> Result<TimelineResponse> {
        let url = format!("{}/xrpc/app.bsky.feed.getTimeline", feed.oauth.pds_url);

        let mut req = self
            .http_client
            .get(&url)
            .header(
                "Authorization",
                format!("Bearer {}", feed.oauth.access_token),
            )
            .query(&[("limit", limit.to_string())]);

        if let Some(cursor) = cursor {
            req = req.query(&[("cursor", cursor)]);
        }

        tracing::trace!(
            url = %url,
            limit = limit,
            "Sending getTimeline request"
        );

        let response = req
            .send()
            .await
            .context("Failed to send getTimeline request")?;

        let status = response.status();
        if !status.is_success() {
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "(failed to read body)".to_string());
            anyhow::bail!("getTimeline failed: {} - {}", status, body);
        }

        let timeline: TimelineResponse = response
            .json()
            .await
            .context("Failed to parse getTimeline response")?;

        tracing::trace!(
            posts = timeline.feed.len(),
            has_cursor = timeline.cursor.is_some(),
            "Received timeline response"
        );

        Ok(timeline)
    }

    /// Filter posts based on user's filter configuration
    fn filter_posts<'a>(
        &self,
        posts: &'a [FeedViewPost],
        filters: &FilterConfig,
    ) -> Vec<&'a FeedViewPost> {
        posts
            .iter()
            .filter(|post| {
                // Check if it's a repost
                if let Some(reason) = &post.reason {
                    // Parse the reason type
                    if reason.reason_type == "app.bsky.feed.defs#reasonRepost" {
                        let reposter_did = &reason.by.did;

                        // Filter out if reposter is blocked
                        if filters.is_reposter_blocked(reposter_did) {
                            tracing::trace!(
                                post_uri = %post.post.uri,
                                reposter = %reposter_did,
                                "Filtered out blocked repost"
                            );
                            return false;
                        }
                    }
                }
                true
            })
            .collect()
    }
}

// AT Protocol Response Types

/// Response from app.bsky.feed.getTimeline
#[derive(Debug, Deserialize)]
pub struct TimelineResponse {
    /// Cursor for pagination
    pub cursor: Option<String>,
    /// Feed view posts
    pub feed: Vec<FeedViewPost>,
}

/// A single feed view post (post + optional reason + optional reply context)
#[derive(Debug, Deserialize)]
pub struct FeedViewPost {
    /// The post itself
    pub post: PostView,
    /// Reason for appearing in feed (e.g., repost)
    pub reason: Option<ReasonRepost>,
    /// Reply context if this is a reply
    #[serde(default)]
    pub reply: Option<ReplyRef>,
}

/// Post view (simplified)
#[derive(Debug, Deserialize)]
pub struct PostView {
    /// AT-URI of the post
    pub uri: String,
    /// CID of the post
    pub cid: String,
    /// Author of the post
    pub author: ProfileViewBasic,
    /// Post record
    pub record: serde_json::Value,
    /// When the post was indexed
    #[serde(rename = "indexedAt")]
    pub indexed_at: String,
}

/// Repost reason
#[derive(Debug, Deserialize)]
pub struct ReasonRepost {
    /// Always "app.bsky.feed.defs#reasonRepost"
    #[serde(rename = "$type")]
    pub reason_type: String,
    /// Who reposted
    pub by: ProfileViewBasic,
    /// When it was reposted
    #[serde(rename = "indexedAt")]
    pub indexed_at: String,
}

/// Basic profile view
#[derive(Debug, Deserialize)]
pub struct ProfileViewBasic {
    /// DID of the user
    pub did: String,
    /// Handle of the user
    pub handle: String,
    /// Display name (optional)
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
    /// Avatar URL (optional)
    pub avatar: Option<String>,
}

/// Reply reference
#[derive(Debug, Deserialize)]
pub struct ReplyRef {
    /// Root post of the thread
    pub root: PostView,
    /// Parent post (immediate reply target)
    pub parent: PostView,
}

// Helper functions

/// Parse ISO 8601 timestamp into microseconds since epoch
fn parse_indexed_at(indexed_at: &str) -> Result<i64> {
    let dt = chrono::DateTime::parse_from_rfc3339(indexed_at)
        .with_context(|| format!("Failed to parse timestamp: {}", indexed_at))?;
    Ok(dt.timestamp_micros())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_indexed_at() {
        let timestamp = "2025-10-17T00:22:35.123Z";
        let result = parse_indexed_at(timestamp);
        assert!(result.is_ok());
        let micros = result.unwrap();
        assert!(micros > 0);
    }

    #[test]
    fn test_filter_posts() {
        use crate::timeline_config::FilterConfig;
        use std::collections::HashSet;

        let pool = sqlx::SqlitePool::connect("sqlite::memory:")
            .await
            .unwrap();
        let config = TimelineConsumerConfig {
            timeline_feeds: TimelineFeeds {
                timeline_feeds: vec![],
            },
            default_poll_interval: Duration::seconds(30),
            user_agent: "test".to_string(),
        };
        let task = TimelineConsumerTask::new(
            pool,
            config,
            CancellationToken::new(),
        )
        .unwrap();

        let mut filters = FilterConfig::default();
        filters
            .blocked_reposters
            .insert("did:plc:blocked".to_string());

        let posts = vec![
            // Regular post (should pass)
            FeedViewPost {
                post: PostView {
                    uri: "at://did:plc:author1/post/1".to_string(),
                    cid: "cid1".to_string(),
                    author: ProfileViewBasic {
                        did: "did:plc:author1".to_string(),
                        handle: "author1.bsky.social".to_string(),
                        display_name: None,
                        avatar: None,
                    },
                    record: serde_json::json!({"text": "Hello"}),
                    indexed_at: "2025-10-17T00:00:00Z".to_string(),
                },
                reason: None,
                reply: None,
            },
            // Repost from blocked user (should be filtered)
            FeedViewPost {
                post: PostView {
                    uri: "at://did:plc:author2/post/2".to_string(),
                    cid: "cid2".to_string(),
                    author: ProfileViewBasic {
                        did: "did:plc:author2".to_string(),
                        handle: "author2.bsky.social".to_string(),
                        display_name: None,
                        avatar: None,
                    },
                    record: serde_json::json!({"text": "World"}),
                    indexed_at: "2025-10-17T00:00:00Z".to_string(),
                },
                reason: Some(ReasonRepost {
                    reason_type: "app.bsky.feed.defs#reasonRepost".to_string(),
                    by: ProfileViewBasic {
                        did: "did:plc:blocked".to_string(),
                        handle: "blocked.bsky.social".to_string(),
                        display_name: None,
                        avatar: None,
                    },
                    indexed_at: "2025-10-17T00:00:00Z".to_string(),
                }),
                reply: None,
            },
            // Repost from allowed user (should pass)
            FeedViewPost {
                post: PostView {
                    uri: "at://did:plc:author3/post/3".to_string(),
                    cid: "cid3".to_string(),
                    author: ProfileViewBasic {
                        did: "did:plc:author3".to_string(),
                        handle: "author3.bsky.social".to_string(),
                        display_name: None,
                        avatar: None,
                    },
                    record: serde_json::json!({"text": "Test"}),
                    indexed_at: "2025-10-17T00:00:00Z".to_string(),
                },
                reason: Some(ReasonRepost {
                    reason_type: "app.bsky.feed.defs#reasonRepost".to_string(),
                    by: ProfileViewBasic {
                        did: "did:plc:allowed".to_string(),
                        handle: "allowed.bsky.social".to_string(),
                        display_name: None,
                        avatar: None,
                    },
                    indexed_at: "2025-10-17T00:00:00Z".to_string(),
                }),
                reply: None,
            },
        ];

        let filtered = task.filter_posts(&posts, &filters);

        // Should have 2 posts (regular post + allowed repost)
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].post.uri, "at://did:plc:author1/post/1");
        assert_eq!(filtered[1].post.uri, "at://did:plc:author3/post/3");
    }
}
