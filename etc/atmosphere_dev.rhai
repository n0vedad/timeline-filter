let rtype = event?.commit?.record?["$type"];

if rtype == "app.bsky.feed.like" {
  return update_match(build_aturi(event));
}

if rtype != "app.bsky.feed.post" {
  return false;
}

const BAIL = [
  // For obvious reasons, keep the feed to PG
  "nsfw", "porn", "nsfl",
  // Used a lot with tags like "dogsofbluesky"
  "ofbluesky",
  "ofbsky",
  "onbsky",
  "onbluesky",
  // Stuff that is also "art" related is usually not developer releated.
  "art", "illustration",
];

const DIDS = [
  "did:plc:ewvi7nxzyoun6zhxrhs64oiz",
  "did:plc:oc6vwdlmk2kqyida5i74d3p5",
  "did:plc:lehcqqkwzcwvjvw66uthu5oq",
  "did:plc:z72i7hdynmk6r22z27h6tvur",
  "did:plc:q6gjnaw2blty4crticxkmujt",
  "did:plc:oky5czdrnfjpqslsw2a5iclo",
  "did:plc:ragtjsm2j2vknwkz3zp4oxrd",
  "did:plc:cbkjy5n7bk3ax2wplmtjofq2",
  "did:plc:tgudj2fjm77pzkuawquqhsxm",
  "did:plc:kkoqcj4msmlta4nr47g6pk4r",
];

let score = 0.0;

const KEYWORDS = [
  [["atproto"], 2.0],
  [["feed", "generator"], 2.0],
  [["pds"], 0.5],
  [["jetstream"], 0.5],
  [["firehose"], 0.5],
  [["lexicon"], 0.5],
  [["activitypub"], 0.5],
  [["federation"], 0.5],
  [["fediverse"], 0.5],
  [["handle"], 0.3],
  [["domain"], 0.3],
  [["mastodon"], 0.3],
  [["bluesky", "infra"], 2.0],
];
const TAGS = [
  ["atproto", 2.0],
  ["atprotocol", 2.0],
  ["pds", 0.5],
  ["atmosphere", 0.5],
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

for keyword in KEYWORDS {
  if sequence_matches(keyword[0], text_normalized) {
    score += keyword[1];
  }
}

let embed_external_url = event?.commit?.record?.embed?.external?.uri ?? "";
if !embed_external_url.is_empty() {
  for needle in URLS {
    if embed_external_url.starts_with(needle) {
      score += 1.0;
    }
  }
}

for facet in event?.commit?.record?.facets ?? [] {
  for feature in facet?.features ?? [] {
    switch feature?["$type"] {
      "app.bsky.richtext.facet#mention" => {
        let mention = feature?["did"] ?? "";
        if !mention.is_empty() {
          if mention in DIDS {
              score += 0.5;
          }
        }
      }
      "app.bsky.richtext.facet#link" => { 
        let link = feature?["uri"] ?? "";
        if !link.is_empty() {
          for needle in URLS {
            if link.starts_with(needle) {
              score += 1.0;
              break;
            }
          }
        }
      }
      "app.bsky.richtext.facet#tag" => {
        let tag = feature?["tag"] ?? "";
        if !tag.is_empty() {
          let tag_normalized = tag.to_lower();
          for token in BAIL {
            if token in tag_normalized {
              return false;
            }
          }
          for needle in TAGS {
            if needle[0] == tag_normalized {
              score += needle[1];
              break;
            }
          }
        }
      }
      _ => {}
    }
  }
}

if score >= 1.0 {
  return build_aturi(event);
}

false
