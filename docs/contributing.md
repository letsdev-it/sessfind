# Contributing

Contributions are welcome! Please open an issue or submit a pull request on [GitHub](https://github.com/letsdev-it/sessfind).

## Dev build

```bash
# Faster iteration — debug build
cargo build

# Run main binary
cargo run -p sessfind

# Run with arguments
cargo run -p sessfind -- search "query"
cargo run -p sessfind -- index --force
```

## Running the semantic plugin

```bash
cargo run -p sessfind-semantic -- index
cargo run -p sessfind-semantic -- search "query"
```

## CI checks

Before pushing, run the same checks that CI runs:

```bash
cargo fmt --all -- --check
cargo build --workspace
cargo test --workspace
```

!!! tip
    Run `cargo fmt --all` (without `--check`) to auto-format your code before committing.

## How to contribute

- **Bug reports** — open an [issue](https://github.com/letsdev-it/sessfind/issues) with steps to reproduce
- **Feature requests** — open an issue describing the feature and your use case
- **Pull requests** — fork the repo, create a branch, make your changes, and open a PR against `main`
