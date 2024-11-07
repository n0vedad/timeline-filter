use anyhow::{Context, Result};
use serde_json_path::JsonPath;

use crate::config;

pub trait Matcher: Sync + Send {
    fn matches(&self, value: &serde_json::Value) -> bool;
}

pub struct FeedMatcher {
    pub(crate) feed: String,
    pub(crate) aturi: Option<serde_json_path::JsonPath>,
    matchers: Vec<Box<dyn Matcher>>,
}

pub(crate) struct FeedMatchers(pub(crate) Vec<FeedMatcher>);

impl FeedMatchers {
    pub(crate) fn from_config(config_feeds: &config::Feeds) -> Result<Self> {
        let mut feed_matchers = vec![];

        for config_feed in config_feeds.feeds.iter() {
            let feed = config_feed.uri.clone();

            let aturi = config_feed
                .aturi
                .as_ref()
                .and_then(|value| JsonPath::parse(value).ok());

            let mut matchers = vec![];

            for config_feed_matcher in config_feed.matchers.iter() {
                match config_feed_matcher {
                    config::Matcher::Equal { path, value } => {
                        matchers
                            .push(Box::new(EqualsMatcher::new(value, path)?) as Box<dyn Matcher>);
                    }
                    config::Matcher::Prefix { path, value } => {
                        matchers
                            .push(Box::new(PrefixMatcher::new(value, path)?) as Box<dyn Matcher>);
                    }
                    config::Matcher::Sequence { path, values } => {
                        matchers.push(Box::new(SequenceMatcher::new(values, path)?) as Box<dyn Matcher>);
                    }
                }
            }

            feed_matchers.push(FeedMatcher {
                feed,
                aturi,
                matchers,
            });
        }

        Ok(Self(feed_matchers))
    }
}

impl FeedMatcher {
    pub(crate) fn matches(&self, value: &serde_json::Value) -> bool {
        self.matchers.iter().any(|matcher| matcher.matches(value))
    }
}

pub struct EqualsMatcher {
    expected: String,
    path: JsonPath,
}

impl EqualsMatcher {
    pub fn new(expected: &str, path: &str) -> Result<Self> {
        let path = JsonPath::parse(path).context("cannot parse path")?;
        Ok(Self {
            expected: expected.to_string(),
            path,
        })
    }
}

impl Matcher for EqualsMatcher {
    fn matches(&self, value: &serde_json::Value) -> bool {
        let nodes = self.path.query(value).all();

        let string_nodes = nodes
            .iter()
            .filter_map(|value| {
                if let serde_json::Value::String(actual) = value {
                    Some(actual.to_lowercase().clone())
                } else {
                    None
                }
            })
            .collect::<Vec<String>>();

        string_nodes.iter().any(|value| value == &self.expected)
    }
}

pub struct PrefixMatcher {
    prefix: String,
    path: JsonPath,
}

impl PrefixMatcher {
    pub(crate) fn new(prefix: &str, path: &str) -> Result<Self> {
        let path = JsonPath::parse(path).context("cannot parse path")?;
        Ok(Self {
            prefix: prefix.to_string(),
            path,
        })
    }
}

impl Matcher for PrefixMatcher {
    fn matches(&self, value: &serde_json::Value) -> bool {
        let nodes = self.path.query(value).all();

        let string_nodes = nodes
            .iter()
            .filter_map(|value| {
                if let serde_json::Value::String(actual) = value {
                    Some(actual.to_lowercase().clone())
                } else {
                    None
                }
            })
            .collect::<Vec<String>>();

        string_nodes
            .iter()
            .any(|value| value.starts_with(&self.prefix))
    }
}

pub struct SequenceMatcher {
    expected: Vec<String>,
    path: JsonPath,
}

impl SequenceMatcher {
    pub(crate) fn new(expected: &[String], path: &str) -> Result<Self> {
        let path = JsonPath::parse(path).context("cannot parse path")?;
        Ok(Self {
            expected: expected.to_owned(),
            path,
        })
    }
}

impl Matcher for SequenceMatcher {
    fn matches(&self, value: &serde_json::Value) -> bool {
        let nodes = self.path.query(value).all();

        let string_nodes = nodes
            .iter()
            .filter_map(|value| {
                if let serde_json::Value::String(actual) = value {
                    Some(actual.to_lowercase().clone())
                } else {
                    None
                }
            })
            .collect::<Vec<String>>();

        for string_node in string_nodes {
            let mut last_found: i32 = -1;

            let mut found_index = 0;
            for (index, expected) in self.expected.iter().enumerate() {
                if let Some(current_found) = string_node.find(expected) {
                    if (current_found as i32) > last_found {
                        last_found = current_found as i32;
                        found_index = index;
                    } else {
                        last_found = -1;
                        break;
                    }
                } else {
                    last_found = -1;
                    break;
                }
            }

            if last_found != -1 && found_index == self.expected.len() - 1 {
                return true;
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equals_matcher() {
        let raw_json = r#"{
    "did": "did:plc:tgudj2fjm77pzkuawquqhsxm",
    "time_us": 1730491093829414,
    "kind": "commit",
    "commit": {
        "rev": "3l7vxhiuibq2u",
        "operation": "create",
        "collection": "app.bsky.feed.post",
        "rkey": "3l7vxhiu4kq2u",
        "record": {
            "$type": "app.bsky.feed.post",
            "createdAt": "2024-11-01T19:58:12.980Z",
            "langs": ["en", "es"],
            "text": "hey dnd question, what does a 45 on a stealth check look like"
        },
        "cid": "bafyreide7jpu67vvkn4p2iznph6frbwv6vamt7yg5duppqjqggz4sdfik4"
    }
}"#;

        let value: serde_json::Value = serde_json::from_str(raw_json).expect("json is valid");

        let tests = vec![
            ("$.did", "did:plc:tgudj2fjm77pzkuawquqhsxm", true),
            ("$.commit.record['$type']", "app.bsky.feed.post", true),
            ("$.commit.record.langs.*", "en", true),
            (
                "$.commit.record.text",
                "hey dnd question, what does a 45 on a stealth check look like",
                true,
            ),
            ("$.did", "did:plc:tgudj2fjm77pzkuawquqhsxn", false),
            ("$.commit.record.notreal", "value", false),
        ];

        for (path, expected, result) in tests {
            let matcher = EqualsMatcher::new(expected, path).expect("matcher is valid");
            assert_eq!(matcher.matches(&value), result);
        }
    }

    #[test]
    fn prefix_matcher() {
        let raw_json = r#"{
    "did": "did:plc:tgudj2fjm77pzkuawquqhsxm",
    "time_us": 1730491093829414,
    "kind": "commit",
    "commit": {
        "rev": "3l7vxhiuibq2u",
        "operation": "create",
        "collection": "app.bsky.feed.post",
        "rkey": "3l7vxhiu4kq2u",
        "record": {
            "$type": "app.bsky.feed.post",
            "createdAt": "2024-11-01T19:58:12.980Z",
            "langs": ["en"],
            "text": "hey dnd question, what does a 45 on a stealth check look like",
            "facets": [
                {
                    "features": [{"$type": "app.bsky.richtext.facet#tag", "tag": "dungeonsanddragons"}],
                    "index": { "byteEnd": 1, "byteStart": 0 }
                },
                {
                    "features": [{"$type": "app.bsky.richtext.facet#tag", "tag": "gaming"}],
                    "index": { "byteEnd": 1, "byteStart": 0 }
                }
            ]
        },
        "cid": "bafyreide7jpu67vvkn4p2iznph6frbwv6vamt7yg5duppqjqggz4sdfik4"
    }
}"#;

        let value: serde_json::Value = serde_json::from_str(raw_json).expect("json is valid");

        let tests = vec![
            ("$.commit.record['$type']", "app.bsky.", true),
            ("$.commit.record.langs.*", "e", true),
            ("$.commit.record.text", "hey dnd question", true),
            ("$.commit.record.facets[*].features[?(@['$type'] == 'app.bsky.richtext.facet#tag')].tag", "dungeons", true),
            ("$.commit.record.notreal", "value", false),
            ("$.commit.record['$type']", "com.bsky.", false),
        ];

        for (path, prefix, result) in tests {
            let matcher = PrefixMatcher::new(prefix, path).expect("matcher is valid");
            assert_eq!(matcher.matches(&value), result);
        }
    }

    #[test]
    fn sequence_matcher() {
        let raw_json = r#"{
    "did": "did:plc:tgudj2fjm77pzkuawquqhsxm",
    "time_us": 1730491093829414,
    "kind": "commit",
    "commit": {
        "rev": "3l7vxhiuibq2u",
        "operation": "create",
        "collection": "app.bsky.feed.post",
        "rkey": "3l7vxhiu4kq2u",
        "record": {
            "$type": "app.bsky.feed.post",
            "createdAt": "2024-11-01T19:58:12.980Z",
            "langs": ["en"],
            "text": "hey dnd question, what does a 45 on a stealth check look like",
            "facets": [
                {
                    "features": [{"$type": "app.bsky.richtext.facet#tag", "tag": "dungeonsanddragons"}],
                    "index": { "byteEnd": 1, "byteStart": 0 }
                },
                {
                    "features": [{"$type": "app.bsky.richtext.facet#tag", "tag": "gaming"}],
                    "index": { "byteEnd": 1, "byteStart": 0 }
                }
            ]
        },
        "cid": "bafyreide7jpu67vvkn4p2iznph6frbwv6vamt7yg5duppqjqggz4sdfik4"
    }
}"#;

        let value: serde_json::Value = serde_json::from_str(raw_json).expect("json is valid");

        let tests = vec![
            (
                "$.commit.record.text",
                vec!["hey".into(), "dnd".into(), "question".into()],
                true,
            ),
            (
                "$.commit.record.facets[*].features[?(@['$type'] == 'app.bsky.richtext.facet#tag')].tag",
                vec!["dungeons".into(), "and".into(), "dragons".into()],
                true,
            ),
            (
                "$.commit.record.text",
                vec!["hey".into(), "question".into(), "dnd".into()],
                false,
            ),
            (
                "$.commit.record.operation",
                vec!["hey".into(), "dnd".into(), "question".into()],
                false,
            ),
            (
                "$.commit.record.text",
                vec!["hey".into(), "nick".into()],
                false,
            ),
        ];

        for (path, values, result) in tests {
            let matcher = SequenceMatcher::new(&values, path).expect("matcher is valid");
            assert_eq!(matcher.matches(&value), result);
        }
    }

    #[test]
    fn sequence_matcher_edge_case_1() {
        let raw_json = r#"{"text": "Stellwerkstörung. Und Signalstörung.  Und der Alternativzug ist auch ausgefallen. Und überhaupt."}"#;
        let value: serde_json::Value = serde_json::from_str(raw_json).expect("json is valid");
        let matcher =
            SequenceMatcher::new(&vec!["smoke".to_string(), "signal".to_string()], "$.text")
                .expect("matcher is valid");
        assert_eq!(matcher.matches(&value), false);
    }
}
