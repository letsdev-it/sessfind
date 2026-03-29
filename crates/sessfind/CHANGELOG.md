# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.1](https://github.com/letsdev-it/sessfind/compare/sessfind-v0.4.0...sessfind-v0.4.1) - 2026-03-29

### Other

- release

## [0.4.0](https://github.com/letsdev-it/sessfind/compare/sessfind-v0.3.3...sessfind-v0.4.0) - 2026-03-29

### Added

- *(tui)* widen help popup and add scroll support
- *(indexer)* add English stemming to FTS tokenizer

### Fixed

- resolve clippy warning for map_or → is_some_and
- *(indexer)* fix prefix queries using PhrasePrefixQuery
- *(tui)* improve light mode visibility and narrow left panel

### Other

- bump sessfind version to 0.4.0
- *(tui)* shorten Shift+Tab hint in status bar

## [0.3.3](https://github.com/letsdev-it/sessfind/compare/sessfind-v0.3.2...sessfind-v0.3.3) - 2026-03-29

### Added

- add `sessfind watch` for automatic session indexing

### Fixed

- resolve clippy warnings for Rust 1.94
- bump MSRV to 1.88 and correct sessfind-common dep version
- resolve merge conflict with main

### Other

- Merge pull request #27 from letsdev-it/fix/msrv-and-deps
- add automatic indexing page to mkdocs nav

## [0.3.2](https://github.com/letsdev-it/sessfind/compare/sessfind-v0.3.1...sessfind-v0.3.2) - 2026-03-29

### Other

- add MkDocs Material site with GitHub Pages deployment

## [0.3.1](https://github.com/letsdev-it/sessfind/compare/sessfind-v0.3.0...sessfind-v0.3.1) - 2026-03-29

### Other

- update Cargo.lock dependencies

## [0.2.0](https://github.com/letsdev-it/sessfind/compare/sessfind-v0.1.3...sessfind-v0.2.0) - 2026-03-29

### Added

- enhance text extraction and semantic indexing
- semantic search

### Fixed

- make sessfind-common publishable and use versioned deps for crates.io

### Other

- Bump version from 0.1.3 to 0.2.0
- apply rustfmt for CI
- bump sessfind version to 0.1.3 to match latest release
