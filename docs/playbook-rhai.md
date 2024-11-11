# Playbook: Rhai

The experimental rhai matcher uses the [rhai](https://rhai.rs/) scripting language to evaluate expressions.

## Builds

To use this feature, the `rhai` feature flag must be enabled at build time.

Locally:

```shell
cargo run --features rhai
```

Container:

```shell
docker build --build-arg=CARGO_FEATURES=rhai .
```

## Scripting

Rhai matchers evaluate a script that returns a `Match` object or a `string` containing the AT-URI of the post that has matched. Return values of `false` or `0` are considered not matched.

The `upsert_match(aturi)` function is available to create a new `Match` object. It has one parameter, the AT-URI of the post that is matched.

```rhai
let condition_thing = true;
// do some stuff ...

if condition_thing {
  return upsert_match();
}

false
```

## Provided Methods

The following methods are available to rhai scripts:

* `build_aturi(event: Event) -> String` - Build an AT-URI from an event. For feed posts, this composes an AT-URI from the event DID, commit collection, and commit rkey.

## Usage

The feed matcher type `rhai` is used with a `source` attribute that points to an rhai script file.

Rhai scripts are evaluated with scope that includes the following variables:

* `event` - The event to match against.

An example config file:

```yaml
feeds:
- uri: "at://did:plc:decafbad/app.bsky.feed.generator/Dcuz0bZP1"
  name: "rhai'ya doing"
  description: "This feed uses the rhai matcher to match against a complex expression."
  matchers:
  - script: "/opt/supercell/rhaiyadoin.rhai"
    type: rhai
```

An example rhai script:

```rhai
// Only match events from the bsky feed where the did is "did:plc:cbkjy5n7bk3ax2wplmtjofq2" (@ngerakines.me).
if event.did != "did:plc:cbkjy5n7bk3ax2wplmtjofq2" {
  return false;
}

// If the event has a commit that has a record that has a $type, set rtype. Otherwise the value will be ().
let rtype = event?.commit?.record["$type"];
switch rtype {
  "app.bsky.feed.post" => {
    // Compose the at-uri of the post that has matched.
    return build_aturi(event);
  }
  "app.bsky.feed.like" => {
    // Returns the subject uri of the like event or false if it doesn't exist.
    return event?.commit?.record?.subject?.uri ?? false;
  }
  _ => { }
}

// Nothing else matches
false
```

