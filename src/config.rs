
use anyhow::{anyhow, Result};
use chrono::Duration;

use crate::timeline_config::TimelineFeeds;

#[derive(Clone)]
pub struct HttpPort(u16);

#[derive(Clone)]
pub struct CertificateBundles(Vec<String>);

#[derive(Clone)]
pub struct TaskEnable(bool);

#[derive(Clone)]
pub struct TaskInterval(Duration);


#[derive(Clone)]
pub struct Config {
    pub version: String,
    pub http_port: HttpPort,
    pub external_base: String,
    pub database_url: String,
    pub certificate_bundles: CertificateBundles,
    pub user_agent: String,
    pub cleanup_task_enable: TaskEnable,
    pub cleanup_task_interval: TaskInterval,
    pub cleanup_task_max_age: TaskInterval,
    pub timeline_feeds: Option<TimelineFeeds>,
    pub timeline_consumer_enable: TaskEnable,
    pub poll_interval: TaskInterval,
}

impl Config {
    pub fn new() -> Result<Self> {
        let http_port: HttpPort = default_env("HTTP_PORT", "4050").try_into()?;
        let external_base = require_env("EXTERNAL_BASE")?;

        let database_url = default_env("DATABASE_URL", "sqlite://development.db");

        let certificate_bundles: CertificateBundles =
            optional_env("CERTIFICATE_BUNDLES").try_into()?;

        let default_user_agent = format!(
            "timeline-filter ({}; +https://github.com/YOUR-USERNAME/timeline-filter)",
            version()?
        );

        let user_agent = default_env("USER_AGENT", &default_user_agent);

        let cleanup_task_enable: TaskEnable =
            default_env("CLEANUP_TASK_ENABLE", "true").try_into()?;

        let cleanup_task_interval: TaskInterval =
            default_env("CLEANUP_TASK_INTERVAL", "1h").try_into()?;

        let cleanup_task_max_age: TaskInterval =
            default_env("CLEANUP_TASK_MAX_AGE", "48h").try_into()?;

        // Timeline Filter configuration
        let timeline_feeds_path = optional_env("TIMELINE_FEEDS");
        let timeline_feeds: Option<TimelineFeeds> = if timeline_feeds_path.is_empty() {
            None
        } else {
            Some(timeline_feeds_path.try_into()?)
        };

        let timeline_consumer_enable: TaskEnable =
            default_env("TIMELINE_CONSUMER_ENABLE", "true").try_into()?;

        let poll_interval: TaskInterval =
            default_env("POLL_INTERVAL", "30s").try_into()?;

        Ok(Self {
            version: version()?,
            http_port,
            external_base,
            database_url,
            certificate_bundles,
            user_agent,
            cleanup_task_enable,
            cleanup_task_interval,
            cleanup_task_max_age,
            timeline_feeds,
            timeline_consumer_enable,
            poll_interval,
        })
    }
}

fn require_env(name: &str) -> Result<String> {
    std::env::var(name)
        .map_err(|err| anyhow::Error::new(err).context(anyhow!("{} must be set", name)))
}

fn optional_env(name: &str) -> String {
    std::env::var(name).unwrap_or("".to_string())
}

fn default_env(name: &str, default_value: &str) -> String {
    std::env::var(name).unwrap_or(default_value.to_string())
}

pub fn version() -> Result<String> {
    option_env!("GIT_HASH")
        .or(option_env!("CARGO_PKG_VERSION"))
        .map(|val| val.to_string())
        .ok_or(anyhow!("one of GIT_HASH or CARGO_PKG_VERSION must be set"))
}

impl TryFrom<String> for HttpPort {
    type Error = anyhow::Error;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty() {
            Ok(Self(80))
        } else {
            value.parse::<u16>().map(Self).map_err(|err| {
                anyhow::Error::new(err).context(anyhow!("parsing PORT into u16 failed"))
            })
        }
    }
}

impl AsRef<u16> for HttpPort {
    fn as_ref(&self) -> &u16 {
        &self.0
    }
}

impl TryFrom<String> for CertificateBundles {
    type Error = anyhow::Error;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Ok(Self(
            value
                .split(';')
                .filter_map(|s| {
                    if s.is_empty() {
                        None
                    } else {
                        Some(s.to_string())
                    }
                })
                .collect::<Vec<String>>(),
        ))
    }
}

impl AsRef<Vec<String>> for CertificateBundles {
    fn as_ref(&self) -> &Vec<String> {
        &self.0
    }
}

impl AsRef<bool> for TaskEnable {
    fn as_ref(&self) -> &bool {
        &self.0
    }
}

impl TryFrom<String> for TaskEnable {
    type Error = anyhow::Error;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        let value = value.parse::<bool>().map_err(|err| {
            anyhow::Error::new(err).context(anyhow!("parsing task enable into bool failed"))
        })?;
        Ok(Self(value))
    }
}

impl AsRef<Duration> for TaskInterval {
    fn as_ref(&self) -> &Duration {
        &self.0
    }
}

impl TryFrom<String> for TaskInterval {
    type Error = anyhow::Error;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        let duration = duration_str::parse_chrono(&value)
            .map_err(|err| anyhow!(err).context("parsing task interval into duration failed"))?;
        Ok(Self(duration))
    }
}

