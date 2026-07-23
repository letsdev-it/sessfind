# VS Code Extension and JSON API

## Boundary

The VS Code extension is a thin client of the `sessfind` executable. It does
not read assistant storage or the Tantivy index directly. The extension checks
`sessfind capabilities`, requires an exact supported JSON API version, and
shows only optional actions advertised in `capabilities.features`.
The hub requires the source-qualified identity, catalog reconciliation,
source-freshness, and session-grouped-search capabilities.

JSON structs shared with frontends live in `sessfind-common`. Changes are
additive while `json_api_version` remains unchanged; breaking changes require a
version bump and coordinated client update.

## Session identity and metadata

Native session IDs are unique only within an assistant source. The CLI emits
`session_key` as `<source>:<native id>`, and frontends use that key for result
ranking, filtering, previews, and mutations. CLI commands that target one
session accept `--source` to resolve collisions. Existing metadata stored under
legacy native IDs remains readable as a compatibility fallback; new metadata
is source-qualified.

`SessionSummary.tags` contains the effective union of direct session tags and
inherited project-directory tags. `SessionSummary.direct_tags` identifies only
tags that can be removed from the session itself.

## Hub behavior

The extension host owns CLI processes, settings, and caches. The webview owns
rendering and transient UI state. Search is cancellable: a new query terminates
the previous process, uses the configured result limit, and reports failures in
the hub. The configured default method applies until the user explicitly
selects and persists another method.

Hover controls reserve their layout space and change only visibility, so list
rows and section headers do not resize when actions appear.

The extension fetches source freshness with the catalog data. Stale and failed
sources remain browsable from their last successful sync and produce a visible
warning. If the configured default search method is unavailable, the hub warns
and uses full-text search for that activation.

Resume and new-session commands prefer VS Code shell integration. If integration
does not become available, the extension asks before typing into the terminal.

## LLM data flow

LLM search sends the user's search intent to the chosen installed AI CLI.
LLM processes time out after 180 seconds, and extension-side search
cancellation terminates superseded processes. Project summary generation is not
currently exposed by the VS Code extension.

## Packaging and checks

VSIX packaging runs the extension build through `vscode:prepublish`, so a fresh
checkout does not depend on an ignored local `dist/` directory. Pull requests
that touch the extension, CLI JSON producer, or shared JSON types run
typechecking, unit tests, build, and package creation.

Release automation mirrors the Cargo `release-plz` lifecycle. Conventional
commits affecting the CLI or extension create or update a VS Code release PR;
merging it updates the manifest, lockfile, version marker, and changelog, then
creates a `vscode-v<version>` GitHub Release. That release publishes the
already-tested VSIX to the Marketplace. Publishing is isolated in the
`vscode-marketplace` GitHub environment and authenticates through the
`VSCE_PAT` repository or environment secret.
