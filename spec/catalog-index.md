# Catalog Indexing and Freshness

## Identity and schema

Native session IDs are scoped to their assistant source. Tantivy stores an
internal `session_key` in `<source>:<native id>` form and all replacement and
deletion operations use that key. Session text is stored for previews, while a
separate indexed-only search field combines conversation content, title,
project, and extracted tool names.

The index schema is versioned by field compatibility. Opening an older schema
that lacks required fields rebuilds the searchable catalog from source data.
Opening also verifies that the source-qualified session keys in Tantivy and the
SQLite bookkeeping are identical. A mismatch clears the SQLite bookkeeping and the next
catalog pass reconciles stale documents and rebuilds from the source logs,
rather than treating missing documents as unchanged sessions.

## Reconciliation

An index pass first discovers a complete source snapshot. It compares the
snapshot with SQLite bookkeeping, writes new and changed sessions, deletes
source-qualified sessions absent from the snapshot, and commits Tantivy before
updating bookkeeping. A failed discovery or write never records a successful
sync and preserves the prior searchable catalog.

Indexing all sources attempts every source independently. Successful passes are
committed even when another source fails; callers aggregate the failures and
return a non-zero status after all attempts.

## Freshness

SQLite records each source's last attempt, last successful sync, and latest
error. The CLI derives `available`, `absent`, `stale`, or `failed` status from
that state and indexed counts. Human and JSON statistics expose the same
information, and frontends use it for non-destructive warnings.

The watcher uses the same reconciliation path. Any additions, updates, or
removals also trigger semantic-index refresh when the semantic plugin is
available.

## Search result boundary

Public search commands return at most one result per source-qualified session,
choosing the highest-ranked chunk. Metadata matches for custom titles and tags
are merged before this grouping. Session previews always resolve chunks by both
source and native ID.
