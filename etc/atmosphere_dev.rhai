let rtype = event?.commit?.record?["$type"];

if rtype == "app.bsky.feed.like" {
  return update_match(build_aturi(event));
}

if rtype != "app.bsky.feed.post" {
  return false;
}

// Reject posts where the created at is more than 8 days ago.
// See https://docs.rs/duration-str/latest/duration_str/
if matcher_before_duration("-8d", event?.commit?.record?.createdAt ?? "") {
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

// Posts that reference one or more of these DIDs may be talking about something atmosphere related, but the weight isn't very high because sometimes posts are just trying to get attention for a feature request or support case.
const DIDS = [
  "did:plc:ewvi7nxzyoun6zhxrhs64oiz", // atproto.com
  "did:plc:oc6vwdlmk2kqyida5i74d3p5", // support.bsky.team
  "did:plc:lehcqqkwzcwvjvw66uthu5oq", // atprotocol.dev
  "did:plc:z72i7hdynmk6r22z27h6tvur", // bsky.app
  "did:plc:q6gjnaw2blty4crticxkmujt", // jaz.bsky.social
  "did:plc:oky5czdrnfjpqslsw2a5iclo", // jay.bsky.team
  "did:plc:ragtjsm2j2vknwkz3zp4oxrd", // pfrazee.com
  "did:plc:cbkjy5n7bk3ax2wplmtjofq2", // ngerakines.me
  "did:plc:tgudj2fjm77pzkuawquqhsxm", // smokesignal.events
  "did:plc:kkoqcj4msmlta4nr47g6pk4r", // aviary.domains
  "did:plc:klmr76mpewpv7rtm3xgpzd7x", // whtwnd.com
  "did:plc:klmr76mpewpv7rtm3xgpzd7x", // frontpage.fyi
  "did:plc:mdjhvva6vlrswsj26cftjttd", // laurenshof.online
  "did:plc:gttrfs4hfmrclyxvwkwcgpj7", // aparker.io
  "did:plc:w4xbfzo7kqfes5zb7r6qv3rw", // rudyfraser.com
  "did:plc:7mnpet2pvof2llhpcwattscf", // stellz.xyz
  "did:plc:by3jhwdqgbtrcc7q4tkkv3cf", // alice.mosphere.at
  "did:plc:fpruhuo22xkm5o7ttr2ktxdo", // danabra.mov
  "did:plc:tpg43qhh4lw4ksiffs4nbda3", // jacob.gold
  "did:plc:vpkhqolt662uhesyj6nxm7ys", // why.bsky.team
  "did:plc:2cxgdrgtsmrbqnjkwyplmp43", // bmann.ca
  "did:plc:cak4klqoj3bqgk5rj6b4f5do", // mmasnick.bsky.social
  "did:plc:44ybard66vv44zksje25o7dz", // bnewbold.net
];

let score = 0.0;

const KEYWORDS = [
  [["atproto"], 2.0],
  [["appview"], 1.0],
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
  [["labeler"], 0.3],
  [["ozone"], 0.3],
  [["bluesky", "infra"], 2.0],
];
const TAGS = [
  ["atproto", 2.0],
  ["atprotocol", 2.0],
  ["pds", 0.5],
  ["atmosphere", 0.5],
  ["ozone", 0.5],
];

const URLS = [
  "https://atproto.com/",
  "https://github.com/bluesky-social/",
  "https://docs.bsky.app/",
  "https://atprotocol.dev/",
  "https://docs.smokesignal.events/",
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

let has_did = false;

for facet in event?.commit?.record?.facets ?? [] {
  for feature in facet?.features ?? [] {
    switch feature?["$type"] {
      "app.bsky.richtext.facet#mention" => {
        let mention = feature?["did"] ?? "";
        if !has_did && !mention.is_empty() {
          if mention in DIDS {
              has_did = true;
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

if has_did {
  score += 0.5;
}
if score >= 1.0 {
  return build_aturi(event);
}

false
