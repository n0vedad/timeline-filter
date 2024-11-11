
let result = new_matcher_result();

let rtype = event?.commit?.record["$type"];

switch rtype {
  "app.bsky.feed.like" => {
    result.matched = true;
  }
  // noop
  _ => { }
}

if result.matched {
  result.aturi = build_aturi(event);
}


result
