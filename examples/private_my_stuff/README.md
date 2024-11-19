# Example: Private My Stuff

This configuration file shows how you can make a private personalized feed based of your posts, replies to your posts, and posts where you are quoted.

This feed is configured with the `allow` attribute that has a list of DIDs that can access the feed. Any request coming from a DID not in that list sees the `deny` post.

## Instructions

1. Create the feed and replace `ATURI` with the full record AT-URI. Should look like `at://YOUR_DID/app.bsky.feed.generator/some_rkey`
2. Update the `DID` placeholders with your DID. There are several references, so be sure to update all of them.
3. Create a new post that people will see when the do not have access to the feed. Replace the `DENIED` placeholder with the full record AT-URI. Should look like `at://YOUR_DID/app.bsky.feed.post/some_rkey`

