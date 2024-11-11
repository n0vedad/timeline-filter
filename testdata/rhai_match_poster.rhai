let rtype = event?.commit?.record["$type"];
switch rtype {
  "app.bsky.feed.post" => {
    if event.did == "did:plc:cbkjy5n7bk3ax2wplmtjofq2" {
      return build_aturi(event);
    }
  }
  _ => { }
}
false
