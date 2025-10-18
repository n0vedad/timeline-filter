use anyhow::{Context, Result};
use chrono::{Duration, Utc};

use crate::storage::StoragePool;
use crate::timeline_config::{FilterConfig, TimelineFeed, TimelineFeeds};

/// Synchronize timeline feeds configuration from YAML to database
/// This should be called on startup to ensure DB matches config file
pub async fn sync_config_to_db(pool: &StoragePool, feeds: &TimelineFeeds) -> Result<()> {
    tracing::info!(
        count = feeds.timeline_feeds.len(),
        "Syncing timeline feeds config to database"
    );

    for feed in &feeds.timeline_feeds {
        sync_user_config(pool, feed).await?;
        sync_user_filters(pool, &feed.did, &feed.filters).await?;
    }

    Ok(())
}

/// Sync a single user's configuration to database
async fn sync_user_config(pool: &StoragePool, feed: &TimelineFeed) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    let poll_interval_seconds = feed
        .poll_interval_duration()
        .map(|d| d.num_seconds() as i64)
        .unwrap_or(30);

    sqlx::query(
        r#"
        INSERT INTO timeline_user_config (
            did, feed_uri, name, description,
            access_token, refresh_token, token_expires_at, pds_url,
            poll_interval_seconds, max_posts_per_poll,
            created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(did) DO UPDATE SET
            feed_uri = excluded.feed_uri,
            name = excluded.name,
            description = excluded.description,
            access_token = excluded.access_token,
            refresh_token = excluded.refresh_token,
            token_expires_at = excluded.token_expires_at,
            pds_url = excluded.pds_url,
            poll_interval_seconds = excluded.poll_interval_seconds,
            max_posts_per_poll = excluded.max_posts_per_poll,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(&feed.did)
    .bind(&feed.feed_uri)
    .bind(&feed.name)
    .bind(&feed.description)
    .bind(&feed.oauth.access_token)
    .bind(&feed.oauth.refresh_token)
    .bind(&feed.oauth.expires_at)
    .bind(&feed.oauth.pds_url)
    .bind(poll_interval_seconds)
    .bind(feed.max_posts_per_poll as i64)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .with_context(|| format!("Failed to sync user config for {}", feed.did))?;

    Ok(())
}

/// Sync a user's filters to database
async fn sync_user_filters(pool: &StoragePool, user_did: &str, filters: &FilterConfig) -> Result<()> {
    // Delete existing filters for this user
    sqlx::query("DELETE FROM timeline_user_filters WHERE user_did = ?")
        .bind(user_did)
        .execute(pool)
        .await?;

    // Insert blocked reposters
    for blocked_did in &filters.blocked_reposters {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO timeline_user_filters (user_did, filter_type, filter_value, created_at)
            VALUES (?, 'blocked_reposter', ?, ?)
            "#,
        )
        .bind(user_did)
        .bind(blocked_did)
        .bind(&now)
        .execute(pool)
        .await
        .with_context(|| {
            format!(
                "Failed to insert blocked_reposter filter for {} -> {}",
                user_did, blocked_did
            )
        })?;
    }

    Ok(())
}

/// Load user configuration from database
pub async fn get_user_config(pool: &StoragePool, user_did: &str) -> Result<Option<UserConfig>> {
    let result = sqlx::query_as::<_, UserConfig>(
        r#"
        SELECT
            did, feed_uri, name, description,
            access_token, refresh_token, token_expires_at, pds_url,
            poll_interval_seconds, max_posts_per_poll
        FROM timeline_user_config
        WHERE did = ?
        "#,
    )
    .bind(user_did)
    .fetch_optional(pool)
    .await?;

    Ok(result)
}

/// Load user filters from database
pub async fn get_user_filters(pool: &StoragePool, user_did: &str) -> Result<UserFilters> {
    let filters = sqlx::query_as::<_, FilterRow>(
        "SELECT filter_type, filter_value FROM timeline_user_filters WHERE user_did = ?",
    )
    .bind(user_did)
    .fetch_all(pool)
    .await?;

    let mut blocked_reposters = Vec::new();

    for filter in filters {
        match filter.filter_type.as_str() {
            "blocked_reposter" => blocked_reposters.push(filter.filter_value),
            _ => {
                tracing::warn!(
                    filter_type = %filter.filter_type,
                    "Unknown filter type in database"
                );
            }
        }
    }

    Ok(UserFilters { blocked_reposters })
}

/// Check if enough time has passed to poll this user's timeline
pub async fn should_poll(pool: &StoragePool, user_did: &str, interval: Duration) -> Result<bool> {
    let result = sqlx::query_scalar::<_, Option<String>>(
        "SELECT last_poll_at FROM timeline_poll_cursor WHERE user_did = ?",
    )
    .bind(user_did)
    .fetch_optional(pool)
    .await?;

    match result {
        Some(Some(last_poll_str)) => {
            let last_poll = chrono::DateTime::parse_from_rfc3339(&last_poll_str)
                .context("Failed to parse last_poll_at")?;
            let now = Utc::now();
            let elapsed = now.signed_duration_since(last_poll.with_timezone(&Utc));
            Ok(elapsed >= interval)
        }
        _ => Ok(true), // Never polled or no record, should poll now
    }
}

/// Get the last cursor for a user's timeline
pub async fn get_cursor(pool: &StoragePool, user_did: &str) -> Result<Option<String>> {
    let result = sqlx::query_scalar::<_, Option<String>>(
        "SELECT last_cursor FROM timeline_poll_cursor WHERE user_did = ?",
    )
    .bind(user_did)
    .fetch_optional(pool)
    .await?;

    Ok(result.flatten())
}

/// Update poll state after successfully polling a user's timeline
pub async fn update_poll_state(
    pool: &StoragePool,
    user_did: &str,
    cursor: Option<&str>,
    posts_indexed: i32,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();

    // Check if record exists
    let exists = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM timeline_poll_cursor WHERE user_did = ?",
    )
    .bind(user_did)
    .fetch_one(pool)
    .await?
        > 0;

    if exists {
        // Update existing record
        sqlx::query(
            r#"
            UPDATE timeline_poll_cursor
            SET last_cursor = ?,
                last_poll_at = ?,
                posts_indexed = ?,
                total_posts_indexed = total_posts_indexed + ?
            WHERE user_did = ?
            "#,
        )
        .bind(cursor)
        .bind(&now)
        .bind(posts_indexed)
        .bind(posts_indexed)
        .bind(user_did)
        .execute(pool)
        .await?;
    } else {
        // Insert new record
        sqlx::query(
            r#"
            INSERT INTO timeline_poll_cursor (
                user_did, last_cursor, last_poll_at, posts_indexed, total_posts_indexed
            ) VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(user_did)
        .bind(cursor)
        .bind(&now)
        .bind(posts_indexed)
        .bind(posts_indexed)
        .execute(pool)
        .await?;
    }

    Ok(())
}

/// Get statistics for a user's timeline polling
pub async fn get_poll_stats(pool: &StoragePool, user_did: &str) -> Result<Option<PollStats>> {
    let result = sqlx::query_as::<_, PollStats>(
        r#"
        SELECT
            last_poll_at,
            posts_indexed,
            total_posts_indexed
        FROM timeline_poll_cursor
        WHERE user_did = ?
        "#,
    )
    .bind(user_did)
    .fetch_optional(pool)
    .await?;

    Ok(result)
}

// Database models

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserConfig {
    pub did: String,
    pub feed_uri: String,
    pub name: String,
    pub description: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub token_expires_at: Option<String>,
    pub pds_url: String,
    pub poll_interval_seconds: i64,
    pub max_posts_per_poll: i64,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct FilterRow {
    filter_type: String,
    filter_value: String,
}

#[derive(Debug, Clone)]
pub struct UserFilters {
    pub blocked_reposters: Vec<String>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PollStats {
    pub last_poll_at: String,
    pub posts_indexed: i64,
    pub total_posts_indexed: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline_config::{FilterConfig, OAuthConfig, TimelineFeed};
    use sqlx::SqlitePool;

    async fn setup_test_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn test_sync_user_config() {
        let pool = setup_test_pool().await;

        let feed = TimelineFeed {
            did: "did:plc:test123".to_string(),
            feed_uri: "at://did:plc:feedgen/app.bsky.feed.generator/test".to_string(),
            name: "Test Feed".to_string(),
            description: "A test feed".to_string(),
            oauth: OAuthConfig {
                access_token: "test_token".to_string(),
                refresh_token: Some("refresh_token".to_string()),
                expires_at: Some("2099-12-31T23:59:59Z".to_string()),
                pds_url: "https://bsky.social".to_string(),
            },
            filters: FilterConfig::default(),
            poll_interval: Some("30s".to_string()),
            max_posts_per_poll: 50,
        };

        sync_user_config(&pool, &feed).await.unwrap();

        let loaded = get_user_config(&pool, "did:plc:test123").await.unwrap();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.did, "did:plc:test123");
        assert_eq!(loaded.access_token, "test_token");
    }

    #[tokio::test]
    async fn test_sync_user_filters() {
        let pool = setup_test_pool().await;

        // First create user config
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
            poll_interval: None,
            max_posts_per_poll: 50,
        };

        sync_user_config(&pool, &feed).await.unwrap();

        // Now sync filters
        let mut filters = FilterConfig::default();
        filters
            .blocked_reposters
            .insert("did:plc:blocked1".to_string());
        filters
            .blocked_reposters
            .insert("did:plc:blocked2".to_string());

        sync_user_filters(&pool, "did:plc:test123", &filters)
            .await
            .unwrap();

        let loaded = get_user_filters(&pool, "did:plc:test123").await.unwrap();
        assert_eq!(loaded.blocked_reposters.len(), 2);
        assert!(loaded.blocked_reposters.contains(&"did:plc:blocked1".to_string()));
    }

    #[tokio::test]
    async fn test_poll_state() {
        let pool = setup_test_pool().await;

        // First create a user (required for foreign key)
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
            poll_interval: None,
            max_posts_per_poll: 50,
        };
        sync_user_config(&pool, &feed).await.unwrap();

        // Should poll when no record exists
        let should = should_poll(&pool, "did:plc:test123", Duration::seconds(30))
            .await
            .unwrap();
        assert!(should);

        // Update poll state
        update_poll_state(&pool, "did:plc:test123", Some("cursor123"), 10)
            .await
            .unwrap();

        // Should not poll immediately after
        let should = should_poll(&pool, "did:plc:test123", Duration::seconds(30))
            .await
            .unwrap();
        assert!(!should);

        // Get stats
        let stats = get_poll_stats(&pool, "did:plc:test123").await.unwrap();
        assert!(stats.is_some());
        let stats = stats.unwrap();
        assert_eq!(stats.posts_indexed, 10);
        assert_eq!(stats.total_posts_indexed, 10);
    }
}

/// Get all feed URIs from timeline_user_config
/// Get all feed URIs from timeline_user_config
pub async fn get_all_feed_uris(pool: &StoragePool) -> Result<Vec<String>> {
    let rows = sqlx::query_as::<_, (String,)>(
        "SELECT feed_uri FROM timeline_user_config ORDER BY created_at DESC"
    )
    .fetch_all(pool)
    .await
    .context("Failed to fetch feed URIs")?;

    Ok(rows.into_iter().map(|(uri,)| uri).collect())
}

/// Get posts for a timeline feed (for getFeedSkeleton endpoint)
/// Returns posts ordered by indexed_at DESC with pagination support
pub async fn get_feed_posts(
    pool: &StoragePool,
    feed_uri: &str,
    limit: u32,
    cursor: Option<String>,
) -> Result<Vec<String>> {
    // Parse cursor as offset (simple pagination)
    let offset = cursor
        .and_then(|c| c.parse::<i64>().ok())
        .unwrap_or(0);

    // Timeline Filter stores posts in feed_content table with feed_id = feed_uri
    let rows = sqlx::query_as::<_, (String,)>(
        r#"
        SELECT uri
        FROM feed_content
        WHERE feed_id = ?
        ORDER BY indexed_at DESC
        LIMIT ? OFFSET ?
        "#,
    )
    .bind(feed_uri)
    .bind(limit as i64)
    .bind(offset)
    .fetch_all(pool)
    .await
    .context("Failed to fetch timeline posts")?;

    Ok(rows.into_iter().map(|(uri,)| uri).collect())
}
