use anyhow::Result;
use chrono::Utc;
use tokio_util::sync::CancellationToken;

use crate::feed_storage::{feed_content_truncate_oldest, StoragePool};

pub struct CleanTask {
    pool: StoragePool,
    max_age: chrono::Duration,
    cancellation_token: CancellationToken,
}

impl CleanTask {
    pub fn new(
        pool: StoragePool,
        max_age: chrono::Duration,
        cancellation_token: CancellationToken,
    ) -> Self {
        Self {
            pool,
            max_age,
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
                        tracing::error!("CleanTask task failed: {}", err);
                    }


                sleeper.as_mut().reset(tokio::time::Instant::now() + interval);
            }
            }
        }
        Ok(())
    }

    pub async fn main(&self) -> Result<()> {
        let now = Utc::now();
        let max_age = now - self.max_age;
        feed_content_truncate_oldest(&self.pool, max_age).await
    }
}
