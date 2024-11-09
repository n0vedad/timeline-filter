# Playbook: Match on likes

The feed configuration supports the optional `aturi` attribute, which can be used to extract the post AT-URI from events.

Using this feature, we can match on non-post records, such as likes.

## Configuration

To match on likes, we need to make 2 changes:

1. Add the `aturi` attribute to the feed configuration for the matcher.
2. Set the environment value `COLLECTIONS` to include `app.bsky.feed.like,app.bsky.feed.post`. When not explicitly set, the default value is `app.bsky.feed.post`.

## Example

In this example feed configuration block, the the `aturi` attribute is set to `$.commit.record.subject.uri`. That JSONPath matches the strong-ref `subject` inside of `{"$type":"app.bsky.feed.like"}` records.

This configuration will match against all like records where the subject is a post of `did:plc:decafbad`.

```yaml
feeds:
- uri: "at://did:plc:decafbad/app.bsky.feed.generator/my_popular_posts"
  name: "My popular posts"
  description: "Posts that I've made that have been liked."
  matchers:
  - path: "$.commit.record.subject.uri"
    value: "at://did:plc:decafbad/app.bsky.feed.post/"
    type: prefix
    aturi: "$.commit.record.subject.uri"
```

