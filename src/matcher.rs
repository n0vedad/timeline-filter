use anyhow::{anyhow, Context, Result};

use serde_json_path::JsonPath;

use crate::config;

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct MatcherResult {
    pub matched: bool,
    pub aturi: String,
}

impl PartialEq<bool> for MatcherResult {
    fn eq(&self, other: &bool) -> bool {
        self.matched == *other
    }
}

impl MatcherResult {
    fn get_matched(&mut self) -> bool {
        self.matched
    }

    fn set_matched(&mut self, value: bool) {
        self.matched = value;
    }

    fn get_aturi(&mut self) -> String {
        self.aturi.clone()
    }

    fn set_aturi(&mut self, value: String) {
        self.aturi = value;
    }
}

pub trait Matcher: Sync + Send {
    fn matches(&self, value: &serde_json::Value) -> Result<MatcherResult>;
}

pub struct FeedMatcher {
    pub(crate) feed: String,
    matchers: Vec<Box<dyn Matcher>>,
}

pub(crate) struct FeedMatchers(pub(crate) Vec<FeedMatcher>);

impl FeedMatchers {
    pub(crate) fn from_config(config_feeds: &config::Feeds) -> Result<Self> {
        let mut feed_matchers = vec![];

        for config_feed in config_feeds.feeds.iter() {
            let feed = config_feed.uri.clone();

            let mut matchers = vec![];

            for config_feed_matcher in config_feed.matchers.iter() {
                match config_feed_matcher {
                    config::Matcher::Equal { path, value, aturi } => {
                        matchers
                            .push(Box::new(EqualsMatcher::new(value, path, aturi)?)
                                as Box<dyn Matcher>);
                    }
                    config::Matcher::Prefix { path, value, aturi } => {
                        matchers
                            .push(Box::new(PrefixMatcher::new(value, path, aturi)?)
                                as Box<dyn Matcher>);
                    }
                    config::Matcher::Sequence {
                        path,
                        values,
                        aturi,
                    } => {
                        matchers.push(Box::new(SequenceMatcher::new(values, path, aturi)?)
                            as Box<dyn Matcher>);
                    }

                    #[cfg(feature = "rhai")]
                    config::Matcher::Rhai { script } => {
                        matchers
                            .push(Box::new(rhai::RhaiMatcher::new(script)?) as Box<dyn Matcher>);
                    }

                    #[cfg(not(feature = "rhai"))]
                    config::Matcher::Rhai { .. } => {
                        return Err(anyhow!("rhai not enabled in this build"))
                    }
                }
            }

            feed_matchers.push(FeedMatcher { feed, matchers });
        }

        Ok(Self(feed_matchers))
    }
}

impl FeedMatcher {
    pub(crate) fn matches(&self, value: &serde_json::Value) -> Option<MatcherResult> {
        for matcher in self.matchers.iter() {
            let result = matcher.matches(value);
            if let Err(err) = result {
                tracing::error!(error = ?err, "matcher returned error");
                continue;
            }
            let result = result.unwrap();
            if result.matched {
                return Some(result);
            }
        }
        None
    }
}

pub struct EqualsMatcher {
    expected: String,
    path: JsonPath,
    aturi_path: Option<JsonPath>,
}

impl EqualsMatcher {
    pub fn new(expected: &str, path: &str, aturi: &Option<String>) -> Result<Self> {
        let path = JsonPath::parse(path).context("cannot parse path")?;
        let aturi_path = if let Some(aturi) = aturi {
            let parsed_aturi_path =
                JsonPath::parse(aturi).context("cannot parse aturi jsonpath")?;
            Some(parsed_aturi_path)
        } else {
            None
        };
        Ok(Self {
            expected: expected.to_string(),
            path,
            aturi_path,
        })
    }
}

impl Matcher for EqualsMatcher {
    fn matches(&self, value: &serde_json::Value) -> Result<MatcherResult> {
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

        if string_nodes.iter().any(|value| value == &self.expected) {
            let aturi = extract_aturi(self.aturi_path.as_ref(), value)
                .ok_or(anyhow!("matcher matched but could not create at-uri"))?;
            Ok(MatcherResult {
                matched: true,
                aturi,
            })
        } else {
            Ok(MatcherResult::default())
        }
    }
}

pub struct PrefixMatcher {
    prefix: String,
    path: JsonPath,
    aturi_path: Option<JsonPath>,
}

impl PrefixMatcher {
    pub(crate) fn new(prefix: &str, path: &str, aturi: &Option<String>) -> Result<Self> {
        let path = JsonPath::parse(path).context("cannot parse path")?;
        let aturi_path = if let Some(aturi) = aturi {
            let parsed_aturi_path =
                JsonPath::parse(aturi).context("cannot parse aturi jsonpath")?;
            Some(parsed_aturi_path)
        } else {
            None
        };
        Ok(Self {
            prefix: prefix.to_string(),
            path,
            aturi_path,
        })
    }
}

impl Matcher for PrefixMatcher {
    fn matches(&self, value: &serde_json::Value) -> Result<MatcherResult> {
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

        let found = string_nodes
            .iter()
            .any(|value| value.starts_with(&self.prefix));
        if found {
            let aturi = extract_aturi(self.aturi_path.as_ref(), value)
                .ok_or(anyhow!("matcher matched but could not create at-uri"))?;
            Ok(MatcherResult {
                matched: true,
                aturi,
            })
        } else {
            Ok(MatcherResult::default())
        }
    }
}

pub struct SequenceMatcher {
    expected: Vec<String>,
    path: JsonPath,
    aturi_path: Option<JsonPath>,
}

impl SequenceMatcher {
    pub(crate) fn new(expected: &[String], path: &str, aturi: &Option<String>) -> Result<Self> {
        let path = JsonPath::parse(path).context("cannot parse path")?;
        let aturi_path = if let Some(aturi) = aturi {
            let parsed_aturi_path =
                JsonPath::parse(aturi).context("cannot parse aturi jsonpath")?;
            Some(parsed_aturi_path)
        } else {
            None
        };
        Ok(Self {
            expected: expected.to_owned(),
            path,
            aturi_path,
        })
    }
}

impl Matcher for SequenceMatcher {
    fn matches(&self, value: &serde_json::Value) -> Result<MatcherResult> {
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
                let aturi = extract_aturi(self.aturi_path.as_ref(), value)
                    .ok_or(anyhow!("matcher matched but could not create at-uri"))?;
                return Ok(MatcherResult {
                    matched: true,
                    aturi,
                });
            }
        }

        Ok(MatcherResult::default())
    }
}

fn extract_aturi(aturi: Option<&JsonPath>, event_value: &serde_json::Value) -> Option<String> {
    if let Some(aturi_path) = aturi {
        let nodes = aturi_path.query(event_value).all();
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

        for value in string_nodes {
            if value.starts_with("at://") {
                return Some(value);
            }
        }
    }

    let rtype = event_value
        .get("commit")
        .and_then(|commit| commit.get("record"))
        .and_then(|commit| commit.get("$type"))
        .and_then(|did| did.as_str());

    if Some("app.bsky.feed.post") == rtype {
        let did = event_value.get("did").and_then(|did| did.as_str())?;
        let commit = event_value.get("commit")?;
        let collection = commit.get("collection").and_then(|did| did.as_str())?;
        let rkey = commit.get("rkey").and_then(|did| did.as_str())?;
        let uri = format!("at://{}/{}/{}", did, collection, rkey);
        return Some(uri);
    }

    None
}

#[cfg(feature = "rhai")]
pub mod rhai {

    use super::{Matcher, MatcherResult};
    use anyhow::{anyhow, Context, Result};

    use rhai::{serde::to_dynamic, Dynamic, Engine, Scope, AST};
    use std::{path::PathBuf, str::FromStr};

    pub struct RhaiMatcher {
        source: String,
        engine: Engine,
        ast: AST,
    }

    impl RhaiMatcher {
        pub(crate) fn new(source: &str) -> Result<Self> {
            let mut engine = Engine::new();
            engine
                .register_type_with_name::<MatcherResult>("MatcherResult")
                .register_get_set(
                    "matched",
                    MatcherResult::get_matched,
                    MatcherResult::set_matched,
                )
                .register_get_set("aturi", MatcherResult::get_aturi, MatcherResult::set_aturi)
                .register_fn("new_matcher_result", MatcherResult::default)
                .register_fn("build_aturi", build_aturi);
            let ast = engine
                .compile_file(PathBuf::from_str(source)?)
                .context("cannot compile script")?;
            Ok(Self {
                source: source.to_string(),
                engine,
                ast,
            })
        }
    }

    impl Matcher for RhaiMatcher {
        fn matches(&self, value: &serde_json::Value) -> Result<MatcherResult> {
            let mut scope = Scope::new();
            let value_map = to_dynamic(value);
            if let Err(err) = value_map {
                tracing::error!(source = ?self.source, error = ?err, "error converting value to dynamic");
                return Ok(MatcherResult::default());
            }
            let value_map = value_map.unwrap();
            scope.push("event", value_map);

            self.engine
                .eval_ast_with_scope::<MatcherResult>(&mut scope, &self.ast)
                .context("error evaluating script")
        }
    }

    fn build_aturi_maybe(event: Dynamic) -> Result<String> {
        println!("{event:?}");
        let event = event.as_map_ref().map_err(|err| anyhow!(err))?;

        let commit = event.get("commit").ok_or(anyhow!("no commit on event"))?.as_map_ref().map_err(|err| anyhow!(err))?;
        let record = commit.get("record").ok_or(anyhow!("no record on event commit"))?.as_map_ref().map_err(|err| anyhow!(err))?;

        let rtype = record.get("$type").ok_or(anyhow!("no $type on event commit record"))?.as_immutable_string_ref().map_err(|err| anyhow!(err))?;

        if rtype.as_str() == "app.bsky.feed.post" {
            let did = event.get("did").ok_or(anyhow!("no did on event"))?.as_immutable_string_ref().map_err(|err| anyhow!(err))?;
            let collection = commit.get("collection").ok_or(anyhow!("no collection on event"))?.as_immutable_string_ref().map_err(|err| anyhow!(err))?;
            let rkey = commit.get("rkey").ok_or(anyhow!("no rkey on event commit"))?.as_immutable_string_ref().map_err(|err| anyhow!(err))?;

            return Ok(format!("at://{}/{}/{}", did.as_str(), collection.as_str(), rkey.as_str()));
        }


        Err(anyhow!("no aturi for event"))
    }

    fn build_aturi(event: Dynamic) -> String {
        let aturi = build_aturi_maybe(event);
        if let Err(err) = aturi {
            println!("error {err:?}");
            return "".into();
        }
        aturi.unwrap()
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
            let matcher = EqualsMatcher::new(expected, path, &None).expect("matcher is valid");
            assert_eq!(matcher.matches(&value).expect("match ok"), result);
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
            let matcher = PrefixMatcher::new(prefix, path, &None).expect("matcher is valid");
            assert_eq!(matcher.matches(&value).expect("match ok"), result);
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
            let matcher = SequenceMatcher::new(&values, path, &None).expect("matcher is valid");
            assert_eq!(matcher.matches(&value).expect("match ok"), result);
        }
    }

    #[test]
    fn sequence_matcher_edge_case_1() {
        let raw_json = r#"{"text": "Stellwerkstörung. Und Signalstörung.  Und der Alternativzug ist auch ausgefallen. Und überhaupt."}"#;
        let value: serde_json::Value = serde_json::from_str(raw_json).expect("json is valid");
        let matcher = SequenceMatcher::new(
            &vec!["smoke".to_string(), "signal".to_string()],
            "$.text",
            &None,
        )
        .expect("matcher is valid");
        assert_eq!(matcher.matches(&value).expect("match ok"), false);
    }
}

#[cfg(all(test, feature = "rhai"))]
mod rhaitests {

    use super::rhai::*;
    use super::*;
    use anyhow::{anyhow, Result};
    use std::path::PathBuf;

    #[cfg(feature = "rhai")]
    #[test]
    fn rhai_matcher() -> Result<()> {
        let testdata = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata");

        let tests = vec![
            (
                "post1.json",
                [
                    ("rhai_match_everything.rhai", true, "at://did:plc:cbkjy5n7bk3ax2wplmtjofq2/app.bsky.feed.post/3laadb7behk25"),
                    ("rhai_match_type.rhai", true, "at://did:plc:cbkjy5n7bk3ax2wplmtjofq2/app.bsky.feed.post/3laadb7behk25"),
                    ("rhai_match_poster.rhai", true, "at://did:plc:cbkjy5n7bk3ax2wplmtjofq2/app.bsky.feed.post/3laadb7behk25"),
                    ("rhai_match_reply_root.rhai", false, ""),
                ],
            ),
            (
                "post2.json",
                [
                    ("rhai_match_everything.rhai", true, "at://did:plc:cbkjy5n7bk3ax2wplmtjofq2/app.bsky.feed.post/3laadftr72k25"),
                    ("rhai_match_type.rhai", true, "at://did:plc:cbkjy5n7bk3ax2wplmtjofq2/app.bsky.feed.post/3laadftr72k25"),
                    ("rhai_match_poster.rhai", true, "at://did:plc:cbkjy5n7bk3ax2wplmtjofq2/app.bsky.feed.post/3laadftr72k25"),
                    ("rhai_match_reply_root.rhai", true, "at://did:plc:cbkjy5n7bk3ax2wplmtjofq2/app.bsky.feed.post/3laadftr72k25"),
                ],
            ),
        ];

        for (input_json, matcher_tests) in tests {
            let input_json_path = testdata.join(input_json);
            let json_content = std::fs::read(input_json_path).map_err(|err| {
                anyhow::Error::new(err).context(anyhow!("reading input_json failed"))
            })?;
            let value: serde_json::Value =
                serde_json::from_slice(&json_content).context("parsing input_json failed")?;

            for (matcher_file_name, matched, aturi) in matcher_tests {
                let matcher_path = testdata.join(matcher_file_name);
                let matcher = RhaiMatcher::new(&matcher_path.to_string_lossy())
                    .context("could not construct matcher")?;
                let result = matcher.matches(&value)?;
                assert_eq!(result.matched, matched, "matched {}: {}", input_json, matcher_file_name);
                assert_eq!(result.aturi, aturi, "aturi {}: {}", input_json, matcher_file_name);
            }
        }

        Ok(())
    }
}
