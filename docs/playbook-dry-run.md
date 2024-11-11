# Playbook: Dry Run

To test matchers, you can run in a dry-run mode using an in-memory database.

```shell
export DATABASE_URL=sqlite://file::memory:?cache=shared
export RUST_LOG=supercell=debug,warning
```

