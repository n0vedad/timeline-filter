if event.did != "did:plc:cbkjy5n7bk3ax2wplmtjofq2" {
  return false;
}

let rtype = event?.commit?.record?["$type"];
switch rtype {
  "app.bsky.feed.post" => {
    return build_aturi(event);
  }
  "app.bsky.feed.like" => {
    let like_uri = event?.commit?.record?.subject?.uri ?? "";
    if like_uri.starts_with("at://") {
      return update_match(like_uri);
    }
  }
  _ => { }
}

false
