
let result = new_matcher_result();

let rtype = event?.commit?.record["$type"];

switch rtype {
  "app.bsky.feed.post" => {
    result.matched = event.did == "did:plc:cbkjy5n7bk3ax2wplmtjofq2";
  }
  _ => { }
}

if result.matched {
  result.aturi = build_aturi(event);
}

result
