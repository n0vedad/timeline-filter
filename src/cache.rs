use anyhow::Result;
use chrono::Utc;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use crate::storage::{feed_content_cached, StoragePool};

pub(crate) struct InnerCache {
    pub(crate) page_size: u8,
    pub(crate) cached_feeds: HashMap<String, Vec<Vec<String>>>,
}

#[derive(Clone)]
pub struct Cache {
    pub(crate) inner_cache: Arc<RwLock<InnerCache>>,
}

impl Default for InnerCache {
    fn default() -> Self {
        Self {
            page_size: 20,
            cached_feeds: HashMap::new(),
        }
    }
}

impl Default for Cache {
    fn default() -> Self {
        Self {
            inner_cache: Arc::new(RwLock::new(InnerCache::default())),
        }
    }
}

impl InnerCache {
    pub(crate) fn new(page_size: u8) -> Self {
        Self {
            page_size,
            cached_feeds: HashMap::new(),
        }
    }
}

impl Cache {
    pub fn new(page_size: u8) -> Self {
        Self {
            inner_cache: Arc::new(RwLock::new(InnerCache::new(page_size))),
        }
    }

    pub(crate) async fn get_posts(&self, feed_id: &str, page: usize) -> Option<Vec<String>> {
        let inner = self.inner_cache.read().await;

        let feed_chunks = inner.cached_feeds.get(feed_id)?;

        if page > feed_chunks.len() {
            return None;
        }

        feed_chunks.get(page).cloned()
    }

    #[allow(clippy::ptr_arg)]
    pub(crate) async fn update_feed(&self, feed_id: &str, posts: &Vec<String>) {
        let mut inner = self.inner_cache.write().await;

        let chunks = posts
            .chunks(inner.page_size.into())
            .map(|chunk| chunk.to_vec())
            .collect();

        inner.cached_feeds.insert(feed_id.to_string(), chunks);
    }
}

pub struct CacheTask {
    pub(crate) pool: StoragePool,
    pub(crate) cache: Cache,
    pub(crate) config: crate::config::Config,

    pub(crate) cancellation_token: CancellationToken,
}

impl CacheTask {
    pub fn new(
        pool: StoragePool,
        cache: Cache,
        config: crate::config::Config,
        cancellation_token: CancellationToken,
    ) -> Self {
        Self {
            pool,
            cache,
            config,
            cancellation_token,
        }
    }

    pub async fn run_background(&self, interval: chrono::Duration) -> Result<()> {
        let interval = interval.to_std()?;

        let sleeper = tokio::time::sleep(interval);
        tokio::pin!(sleeper);

        loop {
            tokio::select! {
            () = self.cancellation_token.cancelled() => {
                break;
            },
            () = &mut sleeper => {

                    if let Err(err) = self.main().await {
                        tracing::error!("CacheTask task failed: {}", err);
                    }


                sleeper.as_mut().reset(tokio::time::Instant::now() + interval);
            }
            }
        }
        Ok(())
    }

    pub async fn main(&self) -> Result<()> {
        for feed in &self.config.feeds.feeds {
            let query = feed.query.clone();

            match query {
                crate::config::FeedQuery::Simple { limit } => {
                    if let Err(err) = self.generate_simple(&feed.uri, *limit.as_ref()).await {
                        tracing::error!(error = ?err, feed_uri = ?feed.uri, "failed to generate simple feed");
                    }
                }
                crate::config::FeedQuery::Popular { gravity, limit } => {
                    if let Err(err) = self
                        .generate_popular(&feed.uri, gravity, *limit.as_ref())
                        .await
                    {
                        tracing::error!(error = ?err, feed_uri = ?feed.uri, "failed to generate simple feed");
                    }
                }
            }
        }

        Ok(())
    }

    async fn generate_simple(&self, feed_uri: &str, limit: u32) -> Result<()> {
        let posts = feed_content_cached(&self.pool, feed_uri, limit).await?;
        let posts = posts.iter().map(|post| post.uri.clone()).collect();
        self.cache.update_feed(feed_uri, &posts).await;
        Ok(())
    }

    async fn generate_popular(&self, feed_uri: &str, gravity: f64, limit: u32) -> Result<()> {
        let posts = feed_content_cached(&self.pool, feed_uri, limit).await?;

        let now = Utc::now().timestamp();
        let mut scored_posts = posts
            .iter()
            .map(|post| {
                let age = post.age_in_hours(now);

                let score = ((post.score - 1).min(0) as f64) / ((2 + age) as f64).powf(gravity);

                (score, post.uri.clone(), age)
            })
            .collect::<Vec<(f64, String, i64)>>();

        scored_posts.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());

        println!("{:?}", scored_posts);

        let sorted_posts = scored_posts.iter().map(|post| post.1.clone()).collect();

        self.cache.update_feed(feed_uri, &sorted_posts).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use anyhow::Result;

    #[tokio::test]
    async fn record_feed_content() -> Result<()> {
        let sorted_posts = (0..12)
            .map(|value| format!("at://did:not:real/post/{}", value))
            .collect();

        let cache = Cache::new(5);
        cache.update_feed("feed", &sorted_posts).await;

        assert_eq!(
            cache.get_posts("feed", 0).await,
            Some(
                (0..5)
                    .map(|value| format!("at://did:not:real/post/{}", value))
                    .collect()
            )
        );
        assert_eq!(
            cache.get_posts("feed", 1).await,
            Some(
                (5..10)
                    .map(|value| format!("at://did:not:real/post/{}", value))
                    .collect()
            )
        );
        assert_eq!(
            cache.get_posts("feed", 2).await,
            Some(
                (10..12)
                    .map(|value| format!("at://did:not:real/post/{}", value))
                    .collect()
            )
        );
        assert_eq!(cache.get_posts("feed", 3).await, None);

        Ok(())
    }
}
