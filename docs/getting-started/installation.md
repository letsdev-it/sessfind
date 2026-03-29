# Installation

## From crates.io

```bash
cargo install sessfind
```

!!! note
    Requires Rust **1.85+** (edition 2024).

## From GitHub Releases

Each [release](https://github.com/letsdev-it/sessfind/releases) includes prebuilt archives for:

- Linux x86_64 / aarch64
- macOS Intel / Apple Silicon

Download the `tar.gz` for your platform, unpack, and put `sessfind` on your `PATH`. SHA256 checksums are included for verification.

## From source

```bash
git clone https://github.com/letsdev-it/sessfind.git
cd sessfind
cargo install --path crates/sessfind
```

## Build manually

```bash
cargo build --release
# Binaries at target/release/sessfind and target/release/sessfind-semantic
cp target/release/sessfind ~/.local/bin/
```

## Semantic search plugin (optional)

The semantic search plugin uses an embedded ML model (~450MB) to find sessions by meaning, not just keywords. It supports **Polish and English** (and 100+ other languages).

```bash
# From crates.io
cargo install sessfind-semantic

# Or from source
cargo install --path crates/sessfind-semantic
```

!!! info
    Once installed, `sessfind` automatically detects the plugin and enables semantic search mode in the TUI (`Shift+Tab` to cycle) and CLI (`--method semantic`). No extra configuration needed.
