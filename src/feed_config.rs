use std::collections::HashSet;

use anyhow::{Context, Result};
use chrono::Duration;
use serde::Deserialize;

/// Root configuration structure for timeline feeds
#[derive(Clone, Debug, Deserialize)]
pub struct TimelineFeeds {
    #[serde(default)]
    pub timeline_feeds: Vec<TimelineFeed>,
}

/// Configuration for a single user's timeline feed
#[derive(Clone, Debug, Deserialize)]
pub struct TimelineFeed {
    /// User's DID (Decentralized Identifier)
    pub did: String,

    /// Feed URI for this filtered timeline
    /// e.g. "at://did:plc:feedgen/app.bsky.feed.generator/user123-filtered"
    pub feed_uri: String,

    /// Display name for the feed
    pub name: String,

    /// Description of the feed
    pub description: String,

    /// OAuth configuration for accessing user's timeline
    pub oauth: OAuthConfig,

    /// Filtering rules
    #[serde(default)]
    pub filters: FilterConfig,

    /// How often to poll this user's timeline (overrides global default)
    /// Examples: "30s", "1m", "5m"
    #[serde(default)]
    pub poll_interval: Option<String>,

    /// Maximum number of posts to fetch per poll
    #[serde(default = "default_max_posts")]
    pub max_posts_per_poll: u32,

    /// Maximum number of posts to index during backfill
    /// - Some(500): Stop backfill after 500 posts indexed (default, recommended)
    /// - None: Continue backfill until cursor becomes undefined (can be thousands of posts!)
    #[serde(default = "default_backfill_limit")]
    pub backfill_limit: Option<u32>,
}

impl TimelineFeed {
    /// Parse poll_interval string into chrono::Duration
    pub fn poll_interval_duration(&self) -> Option<Duration> {
        self.poll_interval.as_ref().and_then(|s| {
            duration_str::parse_chrono(s)
                .map_err(|e| {
                    tracing::warn!(
                        interval = %s,
                        error = ?e,
                        "Failed to parse poll_interval, using default"
                    );
                    e
                })
                .ok()
        })
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        self.validate_with_cleanup_age(None)
    }

    /// Validate the configuration with cleanup max age for backfill limit checking
    pub fn validate_with_cleanup_age(&self, cleanup_max_age: Option<chrono::Duration>) -> Result<()> {
        // Validate DID format
        if !self.did.starts_with("did:") {
            anyhow::bail!("Invalid DID format: {}", self.did);
        }

        // Validate feed_uri format
        if !self.feed_uri.starts_with("at://") {
            anyhow::bail!("Invalid feed_uri format: {}", self.feed_uri);
        }

        // Validate OAuth config
        self.oauth.validate()?;

        // Validate poll_interval if present
        if let Some(interval) = &self.poll_interval {
            duration_str::parse_chrono(interval)
                .map_err(|e| anyhow::anyhow!("Invalid poll_interval '{}': {}", interval, e))?;
        }

        // Validate max_posts_per_poll
        if self.max_posts_per_poll == 0 {
            anyhow::bail!("max_posts_per_poll must be greater than 0");
        }
        if self.max_posts_per_poll > 100 {
            anyhow::bail!("max_posts_per_poll cannot exceed 100");
        }

        // Validate backfill_limit and check against cleanup_max_age
        if let Some(limit) = self.backfill_limit {
            if limit == 0 {
                anyhow::bail!("backfill_limit must be greater than 0 or null for unlimited");
            }

            // Check against cleanup_max_age if provided
            if let Some(cleanup_age) = cleanup_max_age {
                // Estimate posts per day (conservative: active users post ~20-100 times/day on timeline)
                // We'll use 500 posts/day as a reasonable average for timeline polling
                const ESTIMATED_POSTS_PER_DAY: f64 = 500.0;

                let cleanup_days = cleanup_age.num_hours() as f64 / 24.0;
                let estimated_posts_in_cleanup_window = (cleanup_days * ESTIMATED_POSTS_PER_DAY) as u32;

                // Warn if backfill_limit significantly exceeds cleanup window
                if limit > estimated_posts_in_cleanup_window * 2 {
                    let wasted_posts = limit - estimated_posts_in_cleanup_window;
                    tracing::warn!(
                        user_did = %self.did,
                        backfill_limit = limit,
                        cleanup_max_age = ?cleanup_age,
                        estimated_posts_in_cleanup_window = estimated_posts_in_cleanup_window,
                        wasted_posts = wasted_posts,
                        "backfill_limit significantly exceeds CLEANUP_TASK_MAX_AGE capacity! \
                         Approximately {} posts will be indexed during backfill but immediately \
                         eligible for cleanup. Consider reducing backfill_limit to ~{}-{} to match \
                         your cleanup window ({:.1} days).",
                        wasted_posts,
                        estimated_posts_in_cleanup_window / 2,
                        estimated_posts_in_cleanup_window,
                        cleanup_days
                    );
                } else if limit > estimated_posts_in_cleanup_window {
                    tracing::info!(
                        user_did = %self.did,
                        backfill_limit = limit,
                        cleanup_max_age = ?cleanup_age,
                        estimated_posts_in_cleanup_window = estimated_posts_in_cleanup_window,
                        "backfill_limit slightly exceeds CLEANUP_TASK_MAX_AGE capacity. \
                         This provides a small buffer but ~{} posts may be eligible for \
                         immediate cleanup.",
                        limit - estimated_posts_in_cleanup_window
                    );
                }
            } else {
                // No cleanup_max_age provided, use generic validation
                const RECOMMENDED_MAX: u32 = 5000;
                if limit > RECOMMENDED_MAX {
                    tracing::warn!(
                        user_did = %self.did,
                        backfill_limit = limit,
                        recommended_max = RECOMMENDED_MAX,
                        "backfill_limit exceeds recommended maximum. \
                         Posts older than CLEANUP_TASK_MAX_AGE will be deleted anyway. \
                         Consider: backfill_limit ≈ (posts_per_day × cleanup_days). \
                         Example: 500 posts/day × 2 days = 1000"
                    );
                }
            }
        } else {
            // Unlimited backfill
            if let Some(cleanup_age) = cleanup_max_age {
                let cleanup_days = cleanup_age.num_hours() as f64 / 24.0;
                tracing::warn!(
                    user_did = %self.did,
                    cleanup_max_age = ?cleanup_age,
                    "Unlimited backfill enabled! This may index thousands of posts and take hours. \
                     Posts older than CLEANUP_TASK_MAX_AGE ({:.1} days) will be deleted anyway. \
                     Consider setting backfill_limit to match your cleanup policy.",
                    cleanup_days
                );
            } else {
                tracing::warn!(
                    user_did = %self.did,
                    "Unlimited backfill enabled! This may index thousands of posts and take hours. \
                     Posts older than CLEANUP_TASK_MAX_AGE will be deleted anyway. \
                     Consider setting backfill_limit to match your cleanup policy (e.g., 500-5000)."
                );
            }
        }

        // Validate filters
        self.filters.validate()?;

        Ok(())
    }
}

/// OAuth configuration for a user
#[derive(Clone, Debug, Deserialize)]
pub struct OAuthConfig {
    /// Access token for AT Protocol API calls
    pub access_token: String,

    /// Optional refresh token for renewing access token
    #[serde(default)]
    pub refresh_token: Option<String>,

    /// Optional expiration timestamp (ISO 8601 format)
    #[serde(default)]
    pub expires_at: Option<String>,

    /// PDS (Personal Data Server) URL
    /// Examples: "https://bsky.social", "https://pds.example.com"
    pub pds_url: String,
}

impl OAuthConfig {
    /// Validate the OAuth configuration
    pub fn validate(&self) -> Result<()> {
        // Validate access_token is not empty
        if self.access_token.trim().is_empty() {
            anyhow::bail!("access_token cannot be empty");
        }

        // Validate PDS URL format
        if !self.pds_url.starts_with("http://") && !self.pds_url.starts_with("https://") {
            anyhow::bail!("Invalid pds_url format: {}", self.pds_url);
        }

        // Validate expires_at if present
        if let Some(expires_at) = &self.expires_at {
            chrono::DateTime::parse_from_rfc3339(expires_at)
                .with_context(|| format!("Invalid expires_at format: {}", expires_at))?;
        }

        Ok(())
    }

    /// Check if the access token is expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = &self.expires_at {
            if let Ok(expires) = chrono::DateTime::parse_from_rfc3339(expires_at) {
                return chrono::Utc::now() >= expires.with_timezone(&chrono::Utc);
            }
        }
        false
    }
}

/// Filtering rules for timeline content
#[derive(Clone, Debug, Deserialize, Default)]
pub struct FilterConfig {
    /// List of DIDs whose reposts should be filtered out
    /// The original posts from these users will still appear
    #[serde(default)]
    pub blocked_reposters: HashSet<String>,

    // Future filter types can be added here:
    // pub blocked_authors: HashSet<String>,
    // pub blocked_keywords: Vec<String>,
    // pub minimum_likes: Option<u32>,
}

impl FilterConfig {
    /// Check if a DID is in the blocked reposters list
    pub fn is_reposter_blocked(&self, did: &str) -> bool {
        self.blocked_reposters.contains(did)
    }

    /// Validate the filter configuration
    pub fn validate(&self) -> Result<()> {
        // Validate all blocked reposter DIDs
        for did in &self.blocked_reposters {
            if !did.starts_with("did:") {
                anyhow::bail!("Invalid DID in blocked_reposters: {}", did);
            }
        }

        Ok(())
    }
}

/// Default value for max_posts_per_poll
fn default_max_posts() -> u32 {
    50
}

/// Default value for backfill_limit
fn default_backfill_limit() -> Option<u32> {
    Some(500)
}

impl TimelineFeeds {
    /// Load TimelineFeeds from a file path with optional cleanup_max_age for validation
    pub fn load_from_path(path: &str, cleanup_max_age: Option<Duration>) -> Result<Self> {
        if path.is_empty() {
            // Return empty config if no path provided
            return Ok(TimelineFeeds {
                timeline_feeds: vec![],
            });
        }

        let content = std::fs::read(path)
            .with_context(|| format!("Failed to read timeline feeds config file: {}", path))?;

        let feeds: TimelineFeeds = serde_yaml::from_slice(&content)
            .with_context(|| format!("Failed to parse timeline feeds config: {}", path))?;

        // Validate all feeds with cleanup_max_age
        for (idx, feed) in feeds.timeline_feeds.iter().enumerate() {
            feed.validate_with_cleanup_age(cleanup_max_age)
                .with_context(|| format!("Invalid configuration for feed #{} ({})", idx, feed.did))?;
        }

        tracing::info!(
            count = feeds.timeline_feeds.len(),
            cleanup_max_age = ?cleanup_max_age,
            "Loaded timeline feeds configuration"
        );

        Ok(feeds)
    }
}

/// Load TimelineFeeds from a file path (without cleanup validation)
impl TryFrom<String> for TimelineFeeds {
    type Error = anyhow::Error;

    fn try_from(path: String) -> Result<Self, Self::Error> {
        TimelineFeeds::load_from_path(&path, None)
    }
}

impl TimelineFeeds {
    /// Get a feed by DID
    pub fn get_by_did(&self, did: &str) -> Option<&TimelineFeed> {
        self.timeline_feeds.iter().find(|f| f.did == did)
    }

    /// Get a feed by feed URI
    pub fn get_by_feed_uri(&self, feed_uri: &str) -> Option<&TimelineFeed> {
        self.timeline_feeds.iter().find(|f| f.feed_uri == feed_uri)
    }

    /// Check if configuration is empty
    pub fn is_empty(&self) -> bool {
        self.timeline_feeds.is_empty()
    }

    /// Get number of configured feeds
    pub fn len(&self) -> usize {
        self.timeline_feeds.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_timeline_feed() {
        let feed = TimelineFeed {
            did: "did:plc:test123".to_string(),
            feed_uri: "at://did:plc:feedgen/app.bsky.feed.generator/test".to_string(),
            name: "Test Feed".to_string(),
            description: "A test feed".to_string(),
            oauth: OAuthConfig {
                access_token: "test_token".to_string(),
                refresh_token: None,
                expires_at: None,
                pds_url: "https://bsky.social".to_string(),
            },
            filters: FilterConfig::default(),
            poll_interval: Some("30s".to_string()),
            max_posts_per_poll: 50,
            backfill_limit: Some(500),
        };

        assert!(feed.validate().is_ok());
    }

    #[test]
    fn test_invalid_did() {
        let feed = TimelineFeed {
            did: "invalid".to_string(),
            feed_uri: "at://did:plc:feedgen/app.bsky.feed.generator/test".to_string(),
            name: "Test Feed".to_string(),
            description: "A test feed".to_string(),
            oauth: OAuthConfig {
                access_token: "test_token".to_string(),
                refresh_token: None,
                expires_at: None,
                pds_url: "https://bsky.social".to_string(),
            },
            filters: FilterConfig::default(),
            poll_interval: None,
            max_posts_per_poll: 50,
            backfill_limit: Some(500),
        };

        assert!(feed.validate().is_err());
    }

    #[test]
    fn test_poll_interval_duration() {
        let feed = TimelineFeed {
            did: "did:plc:test123".to_string(),
            feed_uri: "at://did:plc:feedgen/app.bsky.feed.generator/test".to_string(),
            name: "Test Feed".to_string(),
            description: "A test feed".to_string(),
            oauth: OAuthConfig {
                access_token: "test_token".to_string(),
                refresh_token: None,
                expires_at: None,
                pds_url: "https://bsky.social".to_string(),
            },
            filters: FilterConfig::default(),
            poll_interval: Some("30s".to_string()),
            backfill_limit: Some(500),
            max_posts_per_poll: 50,
        };

        let duration = feed.poll_interval_duration();
        assert!(duration.is_some());
        assert_eq!(duration.unwrap().num_seconds(), 30);
    }

    #[test]
    fn test_filter_config() {
        let mut filters = FilterConfig::default();
        filters.blocked_reposters.insert("did:plc:blocked1".to_string());
        filters.blocked_reposters.insert("did:plc:blocked2".to_string());

        assert!(filters.is_reposter_blocked("did:plc:blocked1"));
        assert!(!filters.is_reposter_blocked("did:plc:notblocked"));
    }

    #[test]
    fn test_oauth_expiration() {
        // Not expired
        let oauth = OAuthConfig {
            access_token: "test".to_string(),
            refresh_token: None,
            expires_at: Some("2099-12-31T23:59:59Z".to_string()),
            pds_url: "https://bsky.social".to_string(),
        };
        assert!(!oauth.is_expired());

        // Expired
        let oauth_expired = OAuthConfig {
            access_token: "test".to_string(),
            refresh_token: None,
            expires_at: Some("2020-01-01T00:00:00Z".to_string()),
            pds_url: "https://bsky.social".to_string(),
        };
        assert!(oauth_expired.is_expired());
    }

    #[test]
    fn test_backfill_limit_validation_with_cleanup_age() {
        // Test case 1: backfill_limit matches cleanup window (48h, ~1000 posts)
        let feed_good = TimelineFeed {
            did: "did:plc:test123".to_string(),
            feed_uri: "at://did:plc:feedgen/app.bsky.feed.generator/test".to_string(),
            name: "Test Feed".to_string(),
            description: "A test feed".to_string(),
            oauth: OAuthConfig {
                access_token: "test_token".to_string(),
                refresh_token: None,
                expires_at: None,
                pds_url: "https://bsky.social".to_string(),
            },
            filters: FilterConfig::default(),
            poll_interval: None,
            max_posts_per_poll: 50,
            backfill_limit: Some(1000),
        };

        let cleanup_age_48h = Some(Duration::hours(48));
        // Should not error, might log info but that's fine
        assert!(feed_good.validate_with_cleanup_age(cleanup_age_48h).is_ok());

        // Test case 2: backfill_limit WAY too high (10000 posts for 48h window)
        let feed_excessive = TimelineFeed {
            did: "did:plc:test456".to_string(),
            feed_uri: "at://did:plc:feedgen/app.bsky.feed.generator/test2".to_string(),
            name: "Test Feed 2".to_string(),
            description: "A test feed with excessive backfill".to_string(),
            oauth: OAuthConfig {
                access_token: "test_token".to_string(),
                refresh_token: None,
                expires_at: None,
                pds_url: "https://bsky.social".to_string(),
            },
            filters: FilterConfig::default(),
            poll_interval: None,
            max_posts_per_poll: 50,
            backfill_limit: Some(10000),
        };

        // Should not error but will log warning (we can't test log output easily)
        assert!(feed_excessive.validate_with_cleanup_age(cleanup_age_48h).is_ok());

        // Test case 3: Unlimited backfill with cleanup age
        let feed_unlimited = TimelineFeed {
            did: "did:plc:test789".to_string(),
            feed_uri: "at://did:plc:feedgen/app.bsky.feed.generator/test3".to_string(),
            name: "Test Feed 3".to_string(),
            description: "A test feed with unlimited backfill".to_string(),
            oauth: OAuthConfig {
                access_token: "test_token".to_string(),
                refresh_token: None,
                expires_at: None,
                pds_url: "https://bsky.social".to_string(),
            },
            filters: FilterConfig::default(),
            poll_interval: None,
            max_posts_per_poll: 50,
            backfill_limit: None,
        };

        // Should not error but will log warning
        assert!(feed_unlimited.validate_with_cleanup_age(cleanup_age_48h).is_ok());

        // Test case 4: Reasonable limit for 7 day cleanup
        let feed_7d = TimelineFeed {
            did: "did:plc:test999".to_string(),
            feed_uri: "at://did:plc:feedgen/app.bsky.feed.generator/test4".to_string(),
            name: "Test Feed 4".to_string(),
            description: "A test feed with 7 day cleanup".to_string(),
            oauth: OAuthConfig {
                access_token: "test_token".to_string(),
                refresh_token: None,
                expires_at: None,
                pds_url: "https://bsky.social".to_string(),
            },
            filters: FilterConfig::default(),
            poll_interval: None,
            max_posts_per_poll: 50,
            backfill_limit: Some(3500),
        };

        let cleanup_age_7d = Some(Duration::days(7));
        // Should be fine - 3500 is reasonable for 7 days
        assert!(feed_7d.validate_with_cleanup_age(cleanup_age_7d).is_ok());
    }
}
