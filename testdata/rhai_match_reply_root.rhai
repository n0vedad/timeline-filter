
let result = new_matcher_result();

let rtype = event?.commit?.record["$type"];

if rtype != "app.bsky.feed.post" {
  return result;
}

let root_uri = event?.commit?.record?.reply?.root?.uri;

result.matched = root_uri.starts_with("at://did:plc:cbkjy5n7bk3ax2wplmtjofq2/app.bsky.feed.post/");

result
