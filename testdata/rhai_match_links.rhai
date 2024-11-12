let rtype = event?.commit?.record["$type"];

switch rtype {
  "app.bsky.feed.post" => {
    let links = [];
    for facet in event?.commit?.record?.facets ?? [] {
      for feature in facet?.features ?? [] {
        if feature?["$type"] == "app.bsky.richtext.facet#link" {
          links += feature?["uri"]
        }
      }
    }

    if "https://github.com/astrenoxcoop/supercell" in links {
      return build_aturi(event);
    }
  }
  // noop
  _ => { }
}

false
