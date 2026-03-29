# Dependencies

## sessfind (main binary)

| Crate | Purpose |
|-------|---------|
| [tantivy](https://github.com/quickwit-oss/tantivy) | Full-text search engine (BM25 ranking) |
| [ratatui](https://github.com/ratatui/ratatui) | Terminal UI framework |
| [crossterm](https://github.com/crossterm-rs/crossterm) | Cross-platform terminal handling |
| [clap](https://github.com/clap-rs/clap) | CLI argument parsing |
| [rusqlite](https://github.com/rusqlite/rusqlite) | SQLite (index state tracking + OpenCode sessions) |
| [serde](https://github.com/serde-rs/serde) / serde_json / serde_yaml | Serialization |
| [chrono](https://github.com/chronotope/chrono) | Date/time handling |
| [walkdir](https://github.com/BurntSushi/walkdir) | Directory traversal |
| [rayon](https://github.com/rayon-rs/rayon) | Parallel processing |
| [which](https://github.com/harryfei/which-rs) | Plugin/tool detection on PATH |

## sessfind-semantic (optional plugin)

| Crate | Purpose |
|-------|---------|
| [fastembed](https://github.com/Anush008/fastembed-rs) | ML embeddings (multilingual-e5-small via ONNX Runtime) |
| [sqlite-vec](https://github.com/asg017/sqlite-vec) | Vector similarity search in SQLite |
| [rusqlite](https://github.com/rusqlite/rusqlite) | SQLite database |
