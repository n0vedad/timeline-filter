let rtype = event?.commit?.record["$type"];

if rtype != "app.bsky.feed.post" {
  return false;
}

let root_uri = event?.commit?.record?.reply?.root?.uri;

if `${root_uri}`.starts_with("at://did:plc:cbkjy5n7bk3ax2wplmtjofq2/app.bsky.feed.post/") {
  return build_aturi(event);
}

false
