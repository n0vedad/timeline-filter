let rtype = event?.commit?.record?["$type"];

// If the event is for a like, use the AT-URI in the record subject to
// increment the score of any existing feed_content records.
if rtype == "app.bsky.feed.like" {
  return update_match(build_aturi(event));
}

// Ignore any record types that aren't posts.
if rtype != "app.bsky.feed.post" {
  return false;
}

// Reject posts where the created at is more than 8 days ago.
// See https://docs.rs/duration-str/latest/duration_str/
if matcher_before_duration("-8d", event?.commit?.record?.createdAt ?? "") {
  return false;
}

// This feed only includes posts that are not replies themselves, but does
// look at replies to adjust the score of root posts.
let parent_uri = event?.commit?.record?.reply?.root?.uri ?? "";
if !parent_uri.is_empty() {
  return parent_uri;
}

for facet in event?.commit?.record?.facets ?? [] {
  for feature in facet?.features ?? [] {
    switch feature?["$type"] {
      "app.bsky.richtext.facet#tag" => {
        let tag = feature?["tag"] ?? "";
        let tag_normalized = tag.to_lower();
        if tag_normalized == "supercell" {

          // If the post is not a reply and has the "#supercell" tag then add
          // it to the feed.

          return build_aturi(event);

        }
      }
      _ => {}
    }
  }
}

false
