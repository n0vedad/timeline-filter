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

Rhai matchers evaluate a script that returns a `MatcherResult` object. The script must return an object that matches this type.

The `new_matcher_result()` function is available to create a new `MatcherResult` object.

```rhai
let result = new_matcher_result();

// do some stuff ...

result
```

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
  - source: "/opt/supercell/rhaiyadoin.rhai"
    type: rhai
```

An example rhai script:

```rhai
let result = new_matcher_result();

let rtype = event?.commit?.record["$type"];

if rtype != "app.bsky.feed.post" {
  return result;
}

let root_uri = event?.commit?.record?.reply?.root?.uri;

result.matched = root_uri.starts_with("at://did:plc:cbkjy5n7bk3ax2wplmtjofq2/app.bsky.feed.post/");

result
```

