let rtype = event?.commit?.record?["$type"];

if rtype != "app.bsky.feed.post" {
  return false;
}

const BAIL = [
  // For obvious reasons, keep the feed to PG
  "nsfw",
  // Used a lot with tags like "dogsofbluesky"
  "ofbluesky",
  "ofbsky",
];

const KEYWORDS = [
  ["atproto"],
  ["feed", "generator"],
  ["pds"],
  ["jetstream"],
  ["firehose"],
];
const TAGS = [
  ["atproto"],
  ["feed", "generator"],
  ["pds"],
  ["bluesky"],
  ["bsky"],
  ["atmosphere"],
];

const URLS = [
  "https://atproto.com/",
  "https://github.com/bluesky-social/",
  "https://docs.bsky.app/",
];

let text = event?.commit?.record?.text ?? "";
let text_normalized = text.to_lower();

for token in BAIL {
  if token in text_normalized {
    return false;
  }
}

for needle in KEYWORDS {
  if sequence_matches(needle, text_normalized) {
    return build_aturi(event);
  }
}

let embed_external_url = event?.commit?.record?.embed?.external?.uri ?? "";
if !embed_external_url.is_empty() {
  for needle in URLS {
    if embed_external_url.starts_with(needle) {
      return build_aturi(event);
    }
  }
}

let links = [];
let tags = [];
for facet in event?.commit?.record?.facets ?? [] {
  for feature in facet?.features ?? [] {
    switch feature?["$type"] {
      "app.bsky.richtext.facet#link" => { 
        let link = feature?["uri"] ?? "";
        if !link.is_empty() {
          for needle in URLS {
            if link.starts_with(needle) {
              return build_aturi(event);
            }
          }
        }
      }
      "app.bsky.richtext.facet#tag" => {
        let tag = feature?["tag"] ?? "";
        let tag_normalized = tag.to_lower();
        for token in BAIL {
          if token in tag_normalized {
            return false;
          }
        }
        for needle in TAGS {
          if sequence_matches(needle, tag_normalized) {
            return build_aturi(event);
          }
        }
      }
      _ => {}
    }
  }
}


false
