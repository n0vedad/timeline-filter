use std::str::FromStr;

use anyhow::{anyhow, Context, Result};
use futures_util::SinkExt;
use futures_util::StreamExt;
use http::HeaderValue;
use http::Uri;
use tokio::time::{sleep, Instant};
use tokio_util::sync::CancellationToken;
use tokio_websockets::{ClientBuilder, Message};

use crate::config;
use crate::matcher::FeedMatchers;
use crate::storage;
use crate::storage::consumer_control_get;
use crate::storage::consumer_control_insert;
use crate::storage::feed_content_insert;
use crate::storage::StoragePool;

const MAX_MESSAGE_SIZE: usize = 25000;

#[derive(Clone)]
pub struct ConsumerTaskConfig {
    pub user_agent: String,
    pub compression: bool,
    pub zstd_dictionary_location: String,
    pub jetstream_hostname: String,
    pub feeds: config::Feeds,
    pub collections: Vec<String>,
}

pub struct ConsumerTask {
    cancellation_token: CancellationToken,
    pool: StoragePool,
    config: ConsumerTaskConfig,
    feed_matchers: FeedMatchers,
}

impl ConsumerTask {
    pub fn new(
        pool: StoragePool,
        config: ConsumerTaskConfig,
        cancellation_token: CancellationToken,
    ) -> Result<Self> {
        let feed_matchers = FeedMatchers::from_config(&config.feeds)?;

        Ok(Self {
            pool,
            cancellation_token,
            config,
            feed_matchers,
        })
    }

    pub async fn run_background(&self) -> Result<()> {
        tracing::debug!("ConsumerTask started");

        let last_time_us =
            consumer_control_get(&self.pool, &self.config.jetstream_hostname).await?;

        let uri = Uri::from_str(&format!(
            "wss://{}/subscribe?compress={}&requireHello=true",
            self.config.jetstream_hostname, self.config.compression
        ))
        .context("invalid jetstream URL")?;

        tracing::debug!(uri = ?uri, "connecting to jetstream");

        let (mut client, _) = ClientBuilder::from_uri(uri)
            .add_header(
                http::header::USER_AGENT,
                HeaderValue::from_str(&self.config.user_agent)?,
            )
            .connect()
            .await
            .map_err(|err| anyhow::Error::new(err).context("cannot connect to jetstream"))?;

        let update = model::SubscriberSourcedMessage::Update {
            wanted_collections: self.config.collections.clone(),
            wanted_dids: vec![],
            max_message_size_bytes: MAX_MESSAGE_SIZE as u64,
            cursor: last_time_us,
        };
        let serialized_update = serde_json::to_string(&update)
            .map_err(|err| anyhow::Error::msg(err).context("cannot serialize update"))?;

        client
            .send(Message::text(serialized_update))
            .await
            .map_err(|err| anyhow::Error::msg(err).context("cannot send update"))?;

        let mut decompressor = if self.config.compression {
            // mkdir -p data/ && curl -o data/zstd_dictionary https://github.com/bluesky-social/jetstream/raw/refs/heads/main/pkg/models/zstd_dictionary
            let data: Vec<u8> = std::fs::read(self.config.zstd_dictionary_location.clone())
                .context("unable to load zstd dictionary")?;
            zstd::bulk::Decompressor::with_dictionary(&data)
                .map_err(|err| anyhow::Error::msg(err).context("cannot create decompressor"))?
        } else {
            zstd::bulk::Decompressor::new()
                .map_err(|err| anyhow::Error::msg(err).context("cannot create decompressor"))?
        };

        let interval = std::time::Duration::from_secs(120);
        let sleeper = sleep(interval);
        tokio::pin!(sleeper);

        let mut time_usec = 0i64;

        loop {
            tokio::select! {
                () = self.cancellation_token.cancelled() => {
                    break;
                },
                () = &mut sleeper => {
                        consumer_control_insert(&self.pool, &self.config.jetstream_hostname, time_usec).await?;
                        sleeper.as_mut().reset(Instant::now() + interval);
                },
                item = client.next() => {
                    if item.is_none() {
                        tracing::warn!("jetstream connection closed");
                        break;
                    }
                    let item = item.unwrap();

                    if let Err(err) = item {
                        tracing::error!(error = ?err, "error processing jetstream message");
                        continue;
                    }
                    let item = item.unwrap();

                    let event = if self.config.compression {
                        if !item.is_binary() {
                            tracing::debug!("compression enabled but message from jetstream is not binary");
                            continue;
                        }
                        let payload = item.into_payload();

                        let decoded = decompressor.decompress(&payload, MAX_MESSAGE_SIZE * 3);
                        if let Err(err) = decoded {
                            tracing::debug!(err = ?err, "cannot decompress message");
                            continue;
                        }
                        let decoded = decoded.unwrap();
                        serde_json::from_slice::<model::Event>(&decoded)
                        .context(anyhow!("cannot deserialize message"))
                    } else {
                        if !item.is_text() {
                            tracing::debug!("compression enabled but message from jetstream is not binary");
                            continue;
                        }
                        item.as_text()
                            .ok_or(anyhow!("cannot convert message to text"))
                            .and_then(|value| {
                                serde_json::from_str::<model::Event>(value)
                                .context(anyhow!("cannot deserialize message"))
                            })
                    };
                    if let Err(err) = event {
                        tracing::error!(error = ?err, "error processing jetstream message");

                        continue;
                    }
                    let event = event.unwrap();

                    time_usec = std::cmp::max(time_usec, event.time_us);

                    if event.clone().kind != "commit" {
                        continue;
                    }

                    let event_value = serde_json::to_value(event.clone());
                    if let Err(err) = event_value {
                        tracing::error!(error = ?err, "error processing jetstream message");
                        continue;
                    }
                    let event_value = event_value.unwrap();

                    for feed_matcher in self.feed_matchers.0.iter() {
                        if let Some(match_result) = feed_matcher.matches(&event_value) {
                            tracing::debug!(feed_id = ?feed_matcher.feed, "matched event");
                            if match_result.matched {
                                let feed_content = storage::model::FeedContent{
                                    feed_id: feed_matcher.feed.clone(),
                                    uri: match_result.aturi,
                                    indexed_at: event.clone().time_us,
                                };
                                feed_content_insert(&self.pool, &feed_content).await?;
                            }
                        }
                    }
                }
            }
        }

        tracing::debug!("ConsumerTask stopped");

        Ok(())
    }
}

pub(crate) mod model {

    use std::collections::HashMap;

    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(tag = "type", content = "payload")]
    pub(crate) enum SubscriberSourcedMessage {
        #[serde(rename = "options_update")]
        Update {
            #[serde(rename = "wantedCollections")]
            wanted_collections: Vec<String>,

            #[serde(rename = "wantedDids", skip_serializing_if = "Vec::is_empty", default)]
            wanted_dids: Vec<String>,

            #[serde(rename = "maxMessageSizeBytes")]
            max_message_size_bytes: u64,

            #[serde(skip_serializing_if = "Option::is_none")]
            cursor: Option<i64>,
        },
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub(crate) struct Facet {
        pub(crate) features: Vec<HashMap<String, String>>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub(crate) struct StrongRef {
        pub(crate) uri: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub(crate) struct Reply {
        pub(crate) root: Option<StrongRef>,
        pub(crate) parent: Option<StrongRef>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(tag = "$type")]
    pub(crate) enum Record {
        #[serde(rename = "app.bsky.feed.post")]
        Post {
            #[serde(flatten)]
            extra: HashMap<String, serde_json::Value>,
        },
        #[serde(rename = "app.bsky.feed.like")]
        Like {
            #[serde(flatten)]
            extra: HashMap<String, serde_json::Value>,
        },

        #[serde(untagged)]
        Other {
            #[serde(flatten)]
            extra: HashMap<String, serde_json::Value>,
        },
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(tag = "operation")]
    pub(crate) enum CommitOp {
        #[serde(rename = "create")]
        Create {
            rev: String,
            collection: String,
            rkey: String,
            record: Record,
            cid: String,
        },
        #[serde(rename = "update")]
        Update {
            rev: String,
            collection: String,
            rkey: String,
            record: Record,
            cid: String,
        },
        #[serde(rename = "delete")]
        Delete {
            rev: String,
            collection: String,
            rkey: String,
        },
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub(crate) struct Event {
        pub(crate) did: String,
        pub(crate) kind: String,
        pub(crate) time_us: i64,
        pub(crate) commit: Option<CommitOp>,
    }
}
