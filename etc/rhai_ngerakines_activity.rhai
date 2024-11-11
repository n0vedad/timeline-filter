if event.did != "did:plc:cbkjy5n7bk3ax2wplmtjofq2" {
  return false;
}

let rtype = event?.commit?.record["$type"];
switch rtype {
  "app.bsky.feed.post" => {
    return build_aturi(event);
  }
  "app.bsky.feed.like" => {
    return event?.commit?.record?.subject?.uri ?? false;
  }
  _ => { }
}

false
