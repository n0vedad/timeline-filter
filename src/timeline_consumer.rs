//! Timeline Consumer
//!
//! This module implements timeline polling and filtering for AT Protocol feeds.
//! It polls `app.bsky.feed.getTimeline` for each configured user and applies
//! repost filters based on user configuration.
//!
//! ## Type Definitions vs. AT Protocol Spec
//!
//! Our type definitions intentionally deviate from the official AT Protocol lexicon
//! specs in some cases to handle real-world API behavior:
//!
//! - **PostView**: Fields like `cid`, `record`, and `indexedAt` are marked as REQUIRED
//!   in the lexicon but are made Optional here to handle deleted/unavailable posts
//! - **ProfileViewBasic**: Field `handle` is marked as REQUIRED in the lexicon but
//!   is made Optional here to handle suspended/deleted accounts
//!
//! This defensive approach allows us to gracefully handle API edge cases rather than
//! failing to parse entire timeline responses when encountering malformed data.
//!
//! Posts with missing critical fields (like `indexedAt`) are logged and skipped during
//! indexing rather than causing the entire poll cycle to fail.

use anyhow::{Context, Result};
use chrono::Duration;
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
    pub async fn run_background(mut self) -> Result<()> {
        tracing::info!(
            user_count = self.config.timeline_feeds.len(),
            "TimelineConsumerTask started"
        );

        if self.config.timeline_feeds.is_empty() {
            tracing::warn!("No timeline feeds configured, consumer will idle");
        }

        loop {
            // Check for cancellation
            if self.cancellation_token.is_cancelled() {
                tracing::info!("TimelineConsumerTask cancelled");
                break;
            }

            // Run poll cycle
            self.poll_cycle().await;
        }

        Ok(())
    }

    /// Execute one polling cycle for all users
    /// Uses dual-track polling like Bluesky's Following feed:
    /// - Track 1: New posts (60s interval, no cursor) - always runs
    /// - Track 2: Backfill (10s interval, with cursor) - runs until 500 posts indexed
    async fn poll_cycle(&mut self) {
        // Clone feed list to avoid borrow checker issues
        let mut feeds = self.config.timeline_feeds.timeline_feeds.clone();

        for feed in &mut feeds {
            // Check if backfill is still needed
            let needs_backfill = match timeline_storage::needs_backfill(&self.pool, &feed.did).await {
                Ok(needs) => needs,
                Err(e) => {
                    tracing::error!(
                        user_did = %feed.did,
                        error = ?e,
                        "Failed to check backfill status"
                    );
                    continue;
                }
            };

            // TRACK 1: New posts polling (60s interval, always active)
            let new_posts_interval = Duration::seconds(60);
            match timeline_storage::should_poll(&self.pool, &feed.did, new_posts_interval).await {
                Ok(true) => {
                    // Poll WITHOUT cursor to get newest posts
                    if let Err(e) = self.poll_timeline_mode(feed, false).await {
                        tracing::error!(
                            user_did = %feed.did,
                            error = ?e,
                            "Failed to poll new posts"
                        );
                    }
                }
                Ok(false) => {
                    tracing::trace!(
                        user_did = %feed.did,
                        "Skipping new posts poll - not enough time elapsed"
                    );
                }
                Err(e) => {
                    tracing::error!(
                        user_did = %feed.did,
                        error = ?e,
                        "Failed to check new posts poll status"
                    );
                }
            }

            // TRACK 2: Backfill polling (10s interval, runs only if needed)
            if needs_backfill {
                let backfill_interval = feed.poll_interval_duration()
                    .unwrap_or(Duration::seconds(10));

                // Use separate "backfill" tracking in database
                match timeline_storage::should_poll_backfill(&self.pool, &feed.did, backfill_interval).await {
                    Ok(true) => {
                        // Poll WITH cursor to get older posts
                        if let Err(e) = self.poll_timeline_mode(feed, true).await {
                            tracing::error!(
                                user_did = %feed.did,
                                error = ?e,
                                "Failed to poll backfill"
                            );
                        }
                    }
                    Ok(false) => {
                        tracing::trace!(
                            user_did = %feed.did,
                            "Skipping backfill poll - not enough time elapsed"
                        );
                    }
                    Err(e) => {
                        tracing::error!(
                            user_did = %feed.did,
                            error = ?e,
                            "Failed to check backfill poll status"
                        );
                    }
                }
            }
        }

        // Update feeds back (in case tokens were refreshed)
        self.config.timeline_feeds.timeline_feeds = feeds;

        // Sleep briefly before next cycle to avoid tight loop
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    }

    /// Poll timeline in specific mode (backfill=true uses cursor, backfill=false gets newest)
    async fn poll_timeline_mode(&mut self, feed: &mut TimelineFeed, is_backfill: bool) -> Result<()> {
        tracing::debug!(
            user_did = %feed.did,
            feed_uri = %feed.feed_uri,
            mode = if is_backfill { "backfill" } else { "new_posts" },
            "Polling timeline"
        );

        // 0. Check if token needs refresh and refresh if necessary
        self.ensure_valid_token(feed).await?;

        // 1. Determine cursor based on mode
        let cursor = if is_backfill {
            // BACKFILL MODE: Use cursor to fetch older posts
            let cursor = timeline_storage::get_cursor(&self.pool, &feed.did)
                .await
                .context("Failed to get cursor")?;

            if let Some(ref cursor) = cursor {
                tracing::debug!(
                    user_did = %feed.did,
                    cursor = %cursor,
                    "Backfill mode: fetching older posts with cursor"
                );
            }
            cursor
        } else {
            // NEW POSTS MODE: No cursor to fetch newest posts (like Following feed every 60s)
            tracing::debug!(
                user_did = %feed.did,
                "New posts mode: fetching latest posts without cursor"
            );
            None
        };

        // 3. Fetch timeline from AT Protocol
        let timeline = self
            .fetch_timeline(feed, cursor, feed.max_posts_per_poll)
            .await
            .context("Failed to fetch timeline")?;

        // 3. Filter posts based on user's filter config
        let filtered = self.filter_posts(&timeline.feed, &feed.filters);
        let blocked_count = timeline.feed.len() - filtered.len();

        // 4. Index filtered posts into feed_content table
        let mut new_posts = 0;
        let mut updated_posts = 0;
        let mut reposts = 0;
        for post_view in filtered {
            // Skip posts without author (deleted/blocked accounts)
            if post_view.post.author.is_none() {
                tracing::debug!(
                    uri = %post_view.post.uri,
                    "Skipping post without author (deleted/blocked account)"
                );
                continue;
            }

            // Determine which URIs to store, whether it's a repost, and which timestamp to use:
            // - If it's a repost: uri=original post, repost_uri=repost URI, use repost indexed_at
            // - Otherwise: uri=post URI, repost_uri=None, use post indexed_at
            let (uri, repost_uri, is_repost, indexed_at_str) = if let Some(reason) = &post_view.reason {
                if reason.reason_type == "app.bsky.feed.defs#reasonRepost" {
                    if let Some(ref repost_uri_val) = reason.uri {
                        reposts += 1;
                        tracing::trace!(
                            post_uri = %post_view.post.uri,
                            repost_uri = %repost_uri_val,
                            reposter = %reason.by.did,
                            "Indexing repost"
                        );
                        // For reposts: uri=original post, repost_uri=repost record
                        (post_view.post.uri.clone(), Some(repost_uri_val.clone()), true, &reason.indexed_at)
                    } else {
                        tracing::warn!(
                            post_uri = %post_view.post.uri,
                            "Repost reason missing URI, falling back to post URI"
                        );
                        // Fallback to post indexed_at
                        let Some(ref post_indexed_at) = post_view.post.indexed_at else {
                            tracing::debug!(uri = %post_view.post.uri, "Skipping post without indexedAt");
                            continue;
                        };
                        (post_view.post.uri.clone(), None, false, post_indexed_at)
                    }
                } else {
                    // Not a repost, use post indexed_at
                    let Some(ref post_indexed_at) = post_view.post.indexed_at else {
                        tracing::debug!(uri = %post_view.post.uri, "Skipping post without indexedAt");
                        continue;
                    };
                    (post_view.post.uri.clone(), None, false, post_indexed_at)
                }
            } else {
                // No reason, use post indexed_at
                let Some(ref post_indexed_at) = post_view.post.indexed_at else {
                    tracing::debug!(uri = %post_view.post.uri, "Skipping post without indexedAt");
                    continue;
                };
                (post_view.post.uri.clone(), None, false, post_indexed_at)
            };

            let indexed_at = match parse_indexed_at(indexed_at_str) {
                Ok(ts) => ts,
                Err(e) => {
                    tracing::warn!(
                        uri = %uri,
                        error = ?e,
                        "Failed to parse indexedAt, skipping post"
                    );
                    continue;
                }
            };

            match feed_content_upsert(
                &self.pool,
                &FeedContent {
                    feed_id: feed.feed_uri.clone(),
                    uri,
                    indexed_at,
                    score: 1,
                    is_repost,
                    repost_uri,
                },
            )
            .await
            {
                Ok(true) => new_posts += 1,      // New post inserted
                Ok(false) => updated_posts += 1, // Duplicate post skipped
                Err(e) => {
                    tracing::error!(
                        uri = %post_view.post.uri,
                        error = ?e,
                        "Failed to index post"
                    );
                }
            }
        }

        let total_processed = new_posts + updated_posts;

        // 5. Update poll state in database (separate for each mode)
        if is_backfill {
            // BACKFILL MODE: Save cursor and update backfill state
            timeline_storage::update_poll_state(
                &self.pool,
                &feed.did,
                timeline.cursor.as_deref(),
                new_posts, // Only count NEW posts, not duplicates
                blocked_count as i32,
            )
            .await
            .context("Failed to update backfill poll state")?;

            timeline_storage::update_poll_state_backfill(
                &self.pool,
                &feed.did,
                new_posts, // Only count NEW posts, not duplicates
            )
            .await
            .context("Failed to update backfill tracking")?;
        } else {
            // NEW POSTS MODE: Update new posts state (no cursor saved)
            timeline_storage::update_poll_state(
                &self.pool,
                &feed.did,
                None, // Never save cursor in new posts mode
                new_posts, // Only count NEW posts, not duplicates
                blocked_count as i32,
            )
            .await
            .context("Failed to update new posts poll state")?;
        }

        // Get feed stats for logging
        let stats = timeline_storage::get_feed_stats(&self.pool, &feed.feed_uri)
            .await
            .unwrap_or(timeline_storage::FeedStats {
                total_posts: 0,
                total_reposts: 0,
                total_blocked: 0,
            });

        tracing::info!(
            user_did = %feed.did,
            mode = if is_backfill { "backfill" } else { "new_posts" },
            "Poll: fetched={}, blocked={}, indexed={} (new={}, reposts={}, dupes={}), total_db={} (reposts={}, blocked={})",
            timeline.feed.len(),
            blocked_count,
            total_processed,
            new_posts,
            reposts,
            updated_posts,
            stats.total_posts,
            stats.total_reposts,
            stats.total_blocked,
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

        // Get body as text first for better error messages
        let body_text = response
            .text()
            .await
            .context("Failed to read response body")?;

        let timeline: TimelineResponse = serde_json::from_str(&body_text)
            .map_err(|e| {
                // Log first 1000 chars of response for debugging
                let preview = if body_text.len() > 1000 {
                    format!("{}... (truncated, total {} bytes)", &body_text[..1000], body_text.len())
                } else {
                    body_text.clone()
                };
                tracing::error!(
                    error = %e,
                    response_preview = %preview,
                    "Failed to parse getTimeline response"
                );
                e
            })
            .context("Failed to parse getTimeline response")?;

        tracing::trace!(
            posts = timeline.feed.len(),
            has_cursor = timeline.cursor.is_some(),
            "Received timeline response"
        );

        Ok(timeline)
    }

    /// Ensure the access token is valid, refresh if necessary
    async fn ensure_valid_token(&self, feed: &mut TimelineFeed) -> Result<()> {
        // Check if token is expired or will expire soon (within 5 minutes)
        if let Some(ref expires_at) = feed.oauth.expires_at {
            let expires = chrono::DateTime::parse_from_rfc3339(expires_at)
                .context("Failed to parse token expiration")?;
            let now = chrono::Utc::now();
            let buffer = chrono::Duration::minutes(5);

            if expires.signed_duration_since(now) < buffer {
                tracing::info!(
                    user_did = %feed.did,
                    expires_at = %expires_at,
                    "Access token expired or expiring soon, refreshing"
                );
                self.refresh_token(feed).await?;
            }
        } else {
            // No expiration time set, assume token might be expired and try to refresh if we have refresh_token
            if feed.oauth.refresh_token.is_some() {
                tracing::warn!(
                    user_did = %feed.did,
                    "No token expiration set, attempting refresh as precaution"
                );
                self.refresh_token(feed).await?;
            }
        }

        Ok(())
    }

    /// Refresh the OAuth access token using the refresh token
    async fn refresh_token(&self, feed: &mut TimelineFeed) -> Result<()> {
        let refresh_token = feed.oauth.refresh_token.as_ref()
            .ok_or_else(|| anyhow::anyhow!("No refresh token available"))?;

        tracing::info!(
            user_did = %feed.did,
            pds_url = %feed.oauth.pds_url,
            "Refreshing OAuth token"
        );

        let url = format!("{}/xrpc/com.atproto.server.refreshSession", feed.oauth.pds_url);

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", refresh_token))
            .send()
            .await
            .context("Failed to send refresh token request")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_else(|_| "(failed to read body)".to_string());
            anyhow::bail!("Token refresh failed: {} - {}", status, body);
        }

        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct RefreshResponse {
            access_jwt: String,
            refresh_jwt: String,
            did: String,
            /// User handle - we don't store this as timeline config uses static YAML
            /// In a full session manager this would be updated like Bluesky does
            handle: String,
            #[serde(default)]
            did_doc: Option<serde_json::Value>,
        }

        let refresh_response: RefreshResponse = response
            .json()
            .await
            .context("Failed to parse refresh response")?;

        // Validate DID matches (security check - like Bluesky does)
        if refresh_response.did != feed.did {
            anyhow::bail!(
                "DID mismatch during token refresh: expected {}, got {}",
                feed.did,
                refresh_response.did
            );
        }

        tracing::debug!(
            user_did = %feed.did,
            handle = %refresh_response.handle,
            "Token refresh successful"
        );

        // Update feed with new tokens
        feed.oauth.access_token = refresh_response.access_jwt.clone();
        feed.oauth.refresh_token = Some(refresh_response.refresh_jwt.clone());

        // Update PDS URL from didDoc if present (allows PDS migration like Bluesky)
        if let Some(did_doc) = refresh_response.did_doc {
            if let Some(pds_url) = extract_pds_endpoint(&did_doc) {
                tracing::info!(
                    user_did = %feed.did,
                    old_pds = %feed.oauth.pds_url,
                    new_pds = %pds_url,
                    "Updating PDS URL from DID document"
                );
                feed.oauth.pds_url = pds_url;
            }
        }

        // Set expiration to 2 hours from now (typical AT Protocol token lifetime)
        let expires_at = (chrono::Utc::now() + chrono::Duration::hours(2))
            .to_rfc3339();
        feed.oauth.expires_at = Some(expires_at.clone());

        // Update database with new tokens
        timeline_storage::update_tokens(
            &self.pool,
            &feed.did,
            &feed.oauth.access_token,
            feed.oauth.refresh_token.as_deref(),
            Some(&expires_at),
        )
        .await
        .context("Failed to update tokens in database")?;

        tracing::info!(
            user_did = %feed.did,
            expires_at = %expires_at,
            "Successfully refreshed OAuth token"
        );

        Ok(())
    }

    /// Filter posts based on user's filter configuration
    fn filter_posts<'a>(
        &self,
        posts: &'a [FeedViewPost],
        filters: &FilterConfig,
    ) -> Vec<&'a FeedViewPost> {
        Self::filter_posts_static(posts, filters)
    }

    /// Static version of filter_posts for testing
    fn filter_posts_static<'a>(
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

/// Extract PDS endpoint URL from DID document
/// Follows the same logic as Bluesky's getPdsEndpoint() function
fn extract_pds_endpoint(did_doc: &serde_json::Value) -> Option<String> {
    // Look for service with id "#atproto_pds" and type "AtprotoPersonalDataServer"
    let services = did_doc.get("service")?.as_array()?;

    for service in services {
        let id = service.get("id")?.as_str()?;
        let service_type = service.get("type")?.as_str()?;
        let endpoint = service.get("serviceEndpoint")?.as_str()?;

        if (id.ends_with("#atproto_pds") || id == "#atproto_pds")
            && service_type == "AtprotoPersonalDataServer"
        {
            // Validate URL format
            if endpoint.starts_with("http://") || endpoint.starts_with("https://") {
                return Some(endpoint.to_string());
            }
        }
    }

    None
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
///
/// NOTE: According to the official AT Protocol lexicon (app.bsky.feed.defs#postView),
/// the fields `cid`, `record`, `author`, and `indexedAt` are marked as REQUIRED.
/// However, in practice, the Bluesky API sometimes returns posts with missing fields
/// (e.g., deleted posts, unavailable content, suspended accounts, blocked users).
///
/// We mark these fields as Optional to handle these edge cases gracefully,
/// rather than failing to parse the entire timeline response.
/// Posts with missing critical fields (like indexedAt or author) are skipped during processing.
#[derive(Debug, Deserialize)]
pub struct PostView {
    /// AT-URI of the post (REQUIRED by spec)
    pub uri: String,
    /// CID of the post
    /// Per spec: REQUIRED, but we make it Optional for robustness
    pub cid: Option<String>,
    /// Author of the post
    /// Per spec: REQUIRED, but we make it Optional for deleted/blocked accounts
    pub author: Option<ProfileViewBasic>,
    /// Post record
    /// Per spec: REQUIRED, but we make it Optional for deleted/unavailable posts
    #[serde(default)]
    pub record: Option<serde_json::Value>,
    /// When the post was indexed
    /// Per spec: REQUIRED (datetime), but we make it Optional for deleted/unavailable posts
    /// Posts without this field are skipped during indexing
    #[serde(rename = "indexedAt")]
    pub indexed_at: Option<String>,
}

/// Repost reason
#[derive(Debug, Deserialize)]
pub struct ReasonRepost {
    /// Always "app.bsky.feed.defs#reasonRepost"
    #[serde(rename = "$type")]
    pub reason_type: String,
    /// Who reposted
    pub by: ProfileViewBasic,
    /// URI of the repost record
    pub uri: Option<String>,
    /// CID of the repost record
    pub cid: Option<String>,
    /// When it was reposted
    #[serde(rename = "indexedAt")]
    pub indexed_at: String,
}

/// Basic profile view
///
/// NOTE: According to the official AT Protocol lexicon (app.bsky.actor.defs#profileViewBasic),
/// both `did` and `handle` are marked as REQUIRED.
/// However, in practice, the API sometimes returns profiles with missing `handle`
/// (e.g., suspended/deleted accounts, accounts in invalid states).
///
/// We mark `handle` as Optional to handle these edge cases gracefully.
#[derive(Debug, Deserialize)]
pub struct ProfileViewBasic {
    /// DID of the user (REQUIRED by spec)
    pub did: String,
    /// Handle of the user
    /// Per spec: REQUIRED, but we make it Optional for suspended/deleted accounts
    pub handle: Option<String>,
    /// Display name
    /// Per spec: Optional
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
    /// Avatar URL
    /// Per spec: Optional
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

        let mut filters = FilterConfig::default();
        filters
            .blocked_reposters
            .insert("did:plc:blocked".to_string());

        let posts = vec![
            // Regular post (should pass)
            FeedViewPost {
                post: PostView {
                    uri: "at://did:plc:author1/post/1".to_string(),
                    cid: Some("cid1".to_string()),
                    author: Some(ProfileViewBasic {
                        did: "did:plc:author1".to_string(),
                        handle: Some("author1.bsky.social".to_string()),
                        display_name: None,
                        avatar: None,
                    }),
                    record: Some(serde_json::json!({"text": "Hello"})),
                    indexed_at: Some("2025-10-17T00:00:00Z".to_string()),
                },
                reason: None,
                reply: None,
            },
            // Repost from blocked user (should be filtered)
            FeedViewPost {
                post: PostView {
                    uri: "at://did:plc:author2/post/2".to_string(),
                    cid: Some("cid2".to_string()),
                    author: Some(ProfileViewBasic {
                        did: "did:plc:author2".to_string(),
                        handle: Some("author2.bsky.social".to_string()),
                        display_name: None,
                        avatar: None,
                    }),
                    record: Some(serde_json::json!({"text": "World"})),
                    indexed_at: Some("2025-10-17T00:00:00Z".to_string()),
                },
                reason: Some(ReasonRepost {
                    reason_type: "app.bsky.feed.defs#reasonRepost".to_string(),
                    by: ProfileViewBasic {
                        did: "did:plc:blocked".to_string(),
                        handle: Some("blocked.bsky.social".to_string()),
                        display_name: None,
                        avatar: None,
                    },
                    uri: Some("at://did:plc:blocked/app.bsky.feed.repost/xyz".to_string()),
                    cid: Some("repost_cid".to_string()),
                    indexed_at: "2025-10-17T00:00:00Z".to_string(),
                }),
                reply: None,
            },
            // Repost from allowed user (should pass)
            FeedViewPost {
                post: PostView {
                    uri: "at://did:plc:author3/post/3".to_string(),
                    cid: Some("cid3".to_string()),
                    author: Some(ProfileViewBasic {
                        did: "did:plc:author3".to_string(),
                        handle: Some("author3.bsky.social".to_string()),
                        display_name: None,
                        avatar: None,
                    }),
                    record: Some(serde_json::json!({"text": "Test"})),
                    indexed_at: Some("2025-10-17T00:00:00Z".to_string()),
                },
                reason: Some(ReasonRepost {
                    reason_type: "app.bsky.feed.defs#reasonRepost".to_string(),
                    by: ProfileViewBasic {
                        did: "did:plc:allowed".to_string(),
                        handle: Some("allowed.bsky.social".to_string()),
                        display_name: None,
                        avatar: None,
                    },
                    uri: Some("at://did:plc:allowed/app.bsky.feed.repost/abc".to_string()),
                    cid: Some("repost_cid2".to_string()),
                    indexed_at: "2025-10-17T00:00:00Z".to_string(),
                }),
                reply: None,
            },
        ];

        // Use static filter function (no need for task instance)
        let filtered = TimelineConsumerTask::filter_posts_static(&posts, &filters);

        // Should have 2 posts (regular post + allowed repost)
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].post.uri, "at://did:plc:author1/post/1");
        assert_eq!(filtered[1].post.uri, "at://did:plc:author3/post/3");
    }
}
