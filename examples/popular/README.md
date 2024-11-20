# Example: Popular

This configuration file includes a feed that watches for posts with a tag. The feed is then sorted by a simple "popular" algorithm that takes into account the number of likes, replies, and quotes.

## How It Works

The configuration file includes the `query` block that sets the query to sort by popular. The popular algorithm is relatively simple:

    S / pow((T + 2), G)

`S` is the score of the post. When the post is first encountered, it is given a score of 1. Each time a reply is made or the post is liked, the score is incremented by 1.

`T` is the time since the post was created in hours.

`G` is the gravity factor. The higher the gravity, the faster the score will decrease over time.

If two posts have the same score, then they are sorted by the time they were indexed by Supercell.

## Instructions

1. Create the feed and replace `ATURI` with the full record AT-URI. Should look like `at://YOUR_DID/app.bsky.feed.generator/some_rkey`
2. Review the `popular.rhai` file to see how the algorithm works.
