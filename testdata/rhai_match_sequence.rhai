let rtype = event?.commit?.record["$type"];

switch rtype {
  "app.bsky.feed.post" => {
    let text = event?.commit?.record?.text ?? "";
    let found = sequence_matches(["feed", "generator"], text.to_lower());
    if found {
      return build_aturi(event);
    }
  }
  // noop
  _ => { }
}

false
