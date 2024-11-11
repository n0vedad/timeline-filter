let rtype = event?.commit?.record["$type"];

switch rtype {
  "app.bsky.feed.post" => {
    return build_aturi(event);
  }
  // noop
  _ => { }
}

false
