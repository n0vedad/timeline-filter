# supercell

> A supercell is a thunderstorm characterized by the presence of a mesocyclone, a deep, persistently rotating updraft.

Supercell is a lightweight and configurable atproto feed generator.

# Configuration

The following environment variables are used:

* `HTTP_PORT` - The port to listen on for HTTP requests.
* `EXTERNAL_BASE` - The hostname of the feed generator.
* `DATABASE_URL` - The URL of the database to use.
* `JETSTREAM_HOSTNAME` - The hostname of the JetStream server to consume events from.
* `COMPRESSION` - Use zstd compression. Default `false`.
* `ZSTD_DICTIONARY` - The path to the ZSTD dictionary to use. Required when compression is enabled.
* `CONSUMER_TASK_ENABLE` - Whether or not to enable the consumer tasks. Default `true`.
* `VMC_TASK_ENABLE` - Whether or not to enable the VMC (verification method cache) tasks. Default `true`.
* `CACHE_TASK_ENABLE` - Whether or not to enable the cache tasks. Default `true`.
* `CACHE_TASK_INTERVAL` - The interval to run the cache tasks. Default `3m`.
* `CLEANUP_TASK_ENABLE` - Whether or not to enable the cleanup tasks. Default `true`.
* `CLEANUP_TASK_INTERVAL` - The interval to run the cleanup tasks. Default `1h`.
* `CLEANUP_TASK_MAX_AGE` - The maximum age of a post before it is considered stale and deleted from storage. Default `48h`.
* `PLC_HOSTNAME` - The hostname of the PLC server to use for VMC tasks. Default `plc.directory`.
* `FEEDS` - The path to the feeds configuration file.
* `COLLECTIONS` - The collections to consume. Default `app.bsky.feed.post`.
* `RUST_LOG` - Logging configuration. Defaults to `supercell=debug,info`

The feed configuration file is a YAML file that contains the feeds to serve and how to match events to the feed. It supports a variable number of matchers with different rules. Matching is done in order and uses json path plus the matcher implementation.

```yaml
feeds:
- uri: "at://did:plc:4acsffvbo4niovge362ptijz/app.bsky.feed.generator/3la5azib4xe2c"
  name: "Smoke Signal Support"
  description: "The Smoke Signal Support feed."
  allow: ["did:plc:cbkjy5n7bk3ax2wplmtjofq2"]
  deny: "at://did:plc:4acsffvbo4niovge362ptijz/app.bsky.feed.post/3la5bsyzj3j23"
  matchers:
  - path: "$.did"
    value: "did:plc:tgudj2fjm77pzkuawquqhsxm"
    type: equal
  - path: "$.commit.record.facets[*].features[?(@['$type'] == 'app.bsky.richtext.facet#tag')].tag"
    values: ["smoke", "signal"]
    type: sequence
  - path: "$.commit.record.facets[*].features[?(@['$type'] == 'app.bsky.richtext.facet#link')].uri"
    value: "https://smokesignal.events/"
    type: prefix
  - path: "$.commit.record.embed.external.uri"
    value: "https://smokesignal.events/"
    type: prefix
```

The `equal` matcher performs an exact string match matched paths.

The `prefix` matcher performs a prefix string match on matched paths. Given the value "foo bar baz", the following prefixes would match: "foo", "foo ", etc.

The `sequence` matcher performs a sequence string match on matched paths. This is used to match a list of values in order making flexible ordered matching without needing regex or complex reverse lookups.

Consider the example string "The quick brown fox jumps over the lazy dog". The following sequences would match:

* "the" "quick"
* "brown"
* "brow" "fox" "lazy" "dog"
* "the" "dog"

JSONPath is a query language for JSON. When used with matchers, JSONPath will use all nodes as inputs and each matcher will match against any of the values.

For example, the following json would match the `equal` matcher with both `$.text` and `$.tags.*`:

```json
{
    "text": "foo",
    "tags": ["foo", "bar"],
}
```

The site [https://jsonpath.com/](https://jsonpath.com/) is a great resource for testing JSONPath queries.

See the `config.example.yml` file for additional examples.

# License

This project is open source under the MIT license.

Copyright (c) 2023 Astrenox Cooperative. All Rights Reserved.

