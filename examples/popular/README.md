# Example: Popular

This configuration file includes a feed that watches for posts with a tag. The feed is then sorted by a simple "popular" algorithm that takes into account the number of likes, replies, and quotes.

## Instructions

1. Create the feed and replace `ATURI` with the full record AT-URI. Should look like `at://YOUR_DID/app.bsky.feed.generator/some_rkey`
2. Review the `popular.rhai` file to see how the algorithm works.
