use anyhow::{Context, Result};
use chrono::prelude::*;
use sqlx::{Execute, Pool, QueryBuilder, Sqlite};

use model::FeedContent;

pub type StoragePool = Pool<Sqlite>;

pub mod model {
    use chrono::{DateTime, Utc};
    use sqlx::prelude::*;

    #[derive(Clone, FromRow)]
    pub struct FeedContent {
        pub feed_id: String,
        pub uri: String,
        pub indexed_at: i64,
        pub score: i32,
        pub is_repost: bool,
        pub repost_uri: Option<String>,
    }

    #[derive(Clone, FromRow)]
    pub struct Denylist {
        pub subject: String,
        pub reason: String,
        pub created_at: DateTime<Utc>,
    }
}

/// Insert or skip feed content
/// Returns true if a new post was inserted, false if it was a duplicate (skipped)
pub async fn feed_content_upsert(pool: &StoragePool, feed_content: &FeedContent) -> Result<bool> {
    // Check if post already exists
    let exists = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM feed_content WHERE feed_id = ? AND uri = ?"
    )
    .bind(&feed_content.feed_id)
    .bind(&feed_content.uri)
    .fetch_one(pool)
    .await
    .context("failed to check if post exists")?;

    if exists > 0 {
        // Post already exists - skip it (no UPDATE needed)
        Ok(false) // Duplicate
    } else {
        // Insert new post
        let now = Utc::now();
        sqlx::query("INSERT INTO feed_content (feed_id, uri, indexed_at, updated_at, score, is_repost, repost_uri) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(&feed_content.feed_id)
            .bind(&feed_content.uri)
            .bind(feed_content.indexed_at)
            .bind(now)
            .bind(feed_content.score)
            .bind(feed_content.is_repost)
            .bind(&feed_content.repost_uri)
            .execute(pool)
            .await
            .context("failed to insert feed content record")?;
        Ok(true) // New post
    }
}

pub async fn feed_content_update(pool: &StoragePool, feed_content: &FeedContent) -> Result<()> {
    let mut tx = pool.begin().await.context("failed to begin transaction")?;

    let now = Utc::now();
    sqlx::query(
        "UPDATE feed_content SET score = score + ?, updated_at = ? WHERE feed_id = ? AND uri = ?",
    )
    .bind(feed_content.score)
    .bind(now)
    .bind(&feed_content.feed_id)
    .bind(&feed_content.uri)
    .execute(tx.as_mut())
    .await
    .context("failed to update feed content record")?;

    tx.commit().await.context("failed to commit transaction")
}

pub async fn feed_content_truncate_oldest(pool: &StoragePool, age: DateTime<Utc>) -> Result<()> {
    let mut tx = pool.begin().await.context("failed to begin transaction")?;

    sqlx::query("DELETE FROM feed_content WHERE updated_at < ?")
        .bind(age)
        .execute(tx.as_mut())
        .await
        .context("failed to delete feed content beyond mark")?;

    tx.commit().await.context("failed to commit transaction")
}

pub async fn denylist_insert(pool: &StoragePool, subject: &str, reason: &str) -> Result<()> {
    let mut tx = pool.begin().await.context("failed to begin transaction")?;

    let now = Utc::now();
    sqlx::query("INSERT OR REPLACE INTO denylist (subject, reason, updated_at) VALUES (?, ?, ?)")
        .bind(subject)
        .bind(reason)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .context("failed to upsert denylist record")?;

    tx.commit().await.context("failed to commit transaction")
}

pub async fn denylist_upsert(pool: &StoragePool, subject: &str, reason: &str) -> Result<()> {
    denylist_insert(pool, subject, reason).await
}

pub async fn denylist_remove(pool: &StoragePool, subject: &str) -> Result<()> {
    let mut tx = pool.begin().await.context("failed to begin transaction")?;

    sqlx::query("DELETE FROM denylist WHERE subject = ?")
        .bind(subject)
        .execute(tx.as_mut())
        .await
        .context("failed to delete denylist record")?;

    tx.commit().await.context("failed to commit transaction")
}

pub async fn feed_content_purge_aturi(
    pool: &StoragePool,
    aturi: &str,
    feed: &Option<String>,
) -> Result<()> {
    let mut tx = pool.begin().await.context("failed to begin transaction")?;

    if let Some(feed) = feed {
        sqlx::query("DELETE FROM feed_content WHERE feed_id = ? AND uri = ?")
            .bind(feed)
            .bind(aturi)
            .execute(tx.as_mut())
            .await
            .context("failed to delete denylist record")?;
    } else {
        sqlx::query("DELETE FROM feed_content WHERE uri = ?")
            .bind(aturi)
            .execute(tx.as_mut())
            .await
            .context("failed to delete denylist record")?;
    }

    tx.commit().await.context("failed to commit transaction")
}

pub async fn denylist_exists(pool: &StoragePool, subjects: &[&str]) -> Result<bool> {
    let mut tx = pool.begin().await.context("failed to begin transaction")?;

    let mut query_builder: QueryBuilder<Sqlite> =
        QueryBuilder::new("SELECT COUNT(*) FROM denylist WHERE subject IN (");
    let mut separated = query_builder.separated(", ");
    for subject in subjects {
        separated.push_bind(subject);
    }
    separated.push_unseparated(") ");

    let mut query = sqlx::query_scalar::<_, i64>(query_builder.build().sql());
    for subject in subjects {
        query = query.bind(subject);
    }
    let count = query
        .fetch_one(tx.as_mut())
        .await
        .context("failed to delete denylist record")?;

    tx.commit().await.context("failed to commit transaction")?;

    Ok(count > 0)
}
