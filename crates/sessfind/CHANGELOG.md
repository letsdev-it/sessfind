# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.8.1](https://github.com/letsdev-it/sessfind/compare/sessfind-v0.8.0...sessfind-v0.8.1) - 2026-04-14

### Fixed

- *(tui)* make sort indicator visible and move Ctrl+S to results tab

### Other

- release

## [0.8.0](https://github.com/letsdev-it/sessfind/compare/sessfind-v0.7.3...sessfind-v0.8.0) - 2026-04-14

### Added

- *(tui)* add sort order toggle (Ctrl+S) with Newest first / Best match modes

## [0.7.3](https://github.com/letsdev-it/sessfind/compare/sessfind-v0.7.2...sessfind-v0.7.3) - 2026-03-31

### Added

- *(search)* implement real fuzzy search with Levenshtein distance
- add --mode flag, regenerate docs assets

### Other

- Merge pull request #50 from letsdev-it/chore/update-docs-assets

## [0.7.2](https://github.com/letsdev-it/sessfind/compare/sessfind-v0.7.1...sessfind-v0.7.2) - 2026-03-31

### Added

- *(tui)* add pane navigation, dynamic status bar, and F1 help

### Fixed

- *(tui)* prevent overflow in scroll_detail_bottom
- *(tui)* restrict mode switch to search, PgUp/PgDn jump to top/bottom
- *(indexer)* keep consecutive assistant messages in chunks

## [0.7.1](https://github.com/letsdev-it/sessfind/compare/sessfind-v0.7.0...sessfind-v0.7.1) - 2026-03-30

### Added

- *(tui)* show version in banner and check for updates in background

### Fixed

- collapse nested if to satisfy clippy collapsible_if

### Other

- Merge pull request #44 from letsdev-it/feat/version-check

## [0.7.0](https://github.com/letsdev-it/sessfind/compare/sessfind-v0.6.0...sessfind-v0.7.0) - 2026-03-30

### Added

- *(tui)* add resume confirmation dialog with directory choice

### Other

- Merge pull request #38 from letsdev-it/feat/codex-support
- add TUI screenshots and demo GIF (WebP + fallbacks)
- update TUI docs for resume confirmation dialog

## [0.6.0](https://github.com/letsdev-it/sessfind/compare/sessfind-v0.5.0...sessfind-v0.6.0) - 2026-03-29

### Added

- add Codex as session source

### Other

- Merge pull request #35 from letsdev-it/release-plz-2026-03-29T22-02-14Z
- bump all crate versions by minor

## [0.5.0](https://github.com/letsdev-it/sessfind/compare/sessfind-v0.4.1...sessfind-v0.5.0) - 2026-03-29

### Added

- add Cursor as session source

### Fixed

- resolve clippy redundant_closure warning

### Other

- Merge branch 'main' into feat/cursor-support
- bump workspace versions (minor)

## [0.4.1](https://github.com/letsdev-it/sessfind/compare/sessfind-v0.4.0...sessfind-v0.4.1) - 2026-03-29

### Fixed

- unify result sorting — score desc, then date desc

### Other

- Merge pull request #31 from letsdev-it/feat/light-mode-fix

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
