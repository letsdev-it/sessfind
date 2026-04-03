use std::path::PathBuf;

/// Determine the filesystem root from a dash-encoded path string.
///
/// AI tools (Claude Code, Cursor) encode project directory paths by replacing
/// the path separator with `-`.  This function detects the root portion so the
/// caller can reconstruct the original path.
///
/// `strip_leading_dash` — set `true` for Claude Code (encoding starts with `-`)
/// and `false` for Cursor (no leading dash).
///
/// # Unix
///
/// ```text
/// strip_leading_dash = true:   "-Users-m-repos-foo"  → ("/", "Users-m-repos-foo")
/// strip_leading_dash = false:  "Users-m-repos-foo"   → ("/", "Users-m-repos-foo")
/// ```
///
/// # Windows
///
/// ```text
/// "C-Users-m-repos-foo"  → ("C:\", "Users-m-repos-foo")
/// ```
pub fn decode_path_root(encoded: &str, strip_leading_dash: bool) -> (&str, String) {
    #[cfg(unix)]
    {
        if strip_leading_dash && encoded.starts_with('-') {
            (&encoded[1..], String::from("/"))
        } else if strip_leading_dash {
            // No leading dash — return as-is with no root prefix
            (encoded, String::new())
        } else {
            // Cursor style: root is implicit
            (encoded, String::from("/"))
        }
    }

    #[cfg(windows)]
    {
        let bytes = encoded.as_bytes();
        // Drive letter followed by dash: "C-Users-..." → "C:\"
        if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b'-' {
            let drive = bytes[0] as char;
            (&encoded[2..], format!("{}:\\", drive))
        } else if strip_leading_dash && encoded.starts_with('-') {
            // Unix-style encoded path opened on Windows (fallback)
            (&encoded[1..], String::from("/"))
        } else if strip_leading_dash {
            (encoded, String::new())
        } else {
            (encoded, String::from("/"))
        }
    }
}

/// Reconstruct a filesystem path from dash-separated segments by probing the
/// filesystem.
///
/// Segments containing dashes (e.g. `session-seek`) are detected by testing
/// whether progressively joined candidates exist on disk.
pub fn reconstruct_path(segments: &[&str], root: &str) -> String {
    let sep = std::path::MAIN_SEPARATOR;
    let mut path = root.to_string();
    let mut i = 0;

    while i < segments.len() {
        let candidate = format!("{}{}", path, segments[i]);
        if std::path::Path::new(&candidate).exists() {
            path = format!("{}{}", candidate, sep);
            i += 1;
        } else {
            let mut found = false;
            for j in (i + 1..segments.len()).rev() {
                let joined = segments[i..=j].join("-");
                let candidate = format!("{}{}", path, joined);
                if std::path::Path::new(&candidate).exists() {
                    path = format!("{}{}", candidate, sep);
                    i = j + 1;
                    found = true;
                    break;
                }
            }
            if !found {
                let remaining = segments[i..].join("-");
                path = format!("{}{}", path, remaining);
                break;
            }
        }
    }

    path.trim_end_matches(sep).to_string()
}

/// Base directory for the OpenCode database.
///
/// OpenCode uses `XDG_DATA_HOME` (`~/.local/share` on Unix, `%LOCALAPPDATA%`
/// on Windows).
pub fn opencode_data_dir() -> PathBuf {
    #[cfg(unix)]
    {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".local")
            .join("share")
            .join("opencode")
    }

    #[cfg(windows)]
    {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("opencode")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── decode_path_root ───────────────────────────────────────

    #[cfg(unix)]
    mod unix_root {
        use super::*;

        #[test]
        fn claude_style_leading_dash() {
            let (remaining, root) = decode_path_root("-Users-m-repos-foo", true);
            assert_eq!(root, "/");
            assert_eq!(remaining, "Users-m-repos-foo");
        }

        #[test]
        fn claude_style_no_dash_returns_empty_root() {
            let (remaining, root) = decode_path_root("plain-name", true);
            assert_eq!(root, "");
            assert_eq!(remaining, "plain-name");
        }

        #[test]
        fn cursor_style_implicit_root() {
            let (remaining, root) = decode_path_root("Users-m-repos-foo", false);
            assert_eq!(root, "/");
            assert_eq!(remaining, "Users-m-repos-foo");
        }
    }

    #[cfg(windows)]
    mod windows_root {
        use super::*;

        #[test]
        fn drive_letter_with_strip() {
            let (remaining, root) = decode_path_root("C-Users-m-repos-foo", true);
            assert_eq!(root, "C:\\");
            assert_eq!(remaining, "Users-m-repos-foo");
        }

        #[test]
        fn drive_letter_without_strip() {
            let (remaining, root) = decode_path_root("D-Projects-app", false);
            assert_eq!(root, "D:\\");
            assert_eq!(remaining, "Projects-app");
        }

        #[test]
        fn unix_style_fallback_on_windows() {
            let (remaining, root) = decode_path_root("-Users-m-repos", true);
            assert_eq!(root, "/");
            assert_eq!(remaining, "Users-m-repos");
        }

        #[test]
        fn no_drive_no_dash_with_strip() {
            let (remaining, root) = decode_path_root("plain-name", true);
            assert_eq!(root, "");
            assert_eq!(remaining, "plain-name");
        }

        #[test]
        fn no_drive_no_dash_without_strip() {
            let (remaining, root) = decode_path_root("plain-name", false);
            assert_eq!(root, "/");
            assert_eq!(remaining, "plain-name");
        }
    }

    // ── reconstruct_path ───────────────────────────────────────

    #[test]
    fn reconstruct_nonexistent_path_joins_all_with_dashes() {
        // When no segments exist on disk, all remaining segments are joined
        let result = reconstruct_path(&["zzz_fake", "path", "that", "does", "not", "exist"], "/");
        assert_eq!(result, "/zzz_fake-path-that-does-not-exist");
    }

    #[test]
    fn reconstruct_empty_segments() {
        let result = reconstruct_path(&[], "/some/root/");
        // Trailing separator is trimmed
        assert_eq!(result, "/some/root");
    }

    #[test]
    fn reconstruct_resolves_existing_dirs() {
        // Create a temp directory tree to probe against
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        std::fs::create_dir_all(base.join("aaa").join("bbb")).unwrap();

        let root = format!("{}/", base.display());
        let result = reconstruct_path(&["aaa", "bbb"], &root);

        let expected = base.join("aaa").join("bbb");
        assert_eq!(result, expected.to_string_lossy());
    }

    #[test]
    fn reconstruct_resolves_dashed_dir_name() {
        // Simulate a directory named "my-project" (contains a dash)
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        std::fs::create_dir_all(base.join("my-project").join("src")).unwrap();

        let root = format!("{}/", base.display());
        // Segments "my", "project", "src" should resolve "my-project" then "src"
        let result = reconstruct_path(&["my", "project", "src"], &root);

        let expected = base.join("my-project").join("src");
        assert_eq!(result, expected.to_string_lossy());
    }

    // ── opencode_data_dir ──────────────────────────────────────

    #[test]
    fn opencode_data_dir_ends_with_opencode() {
        let dir = opencode_data_dir();
        assert!(
            dir.ends_with("opencode"),
            "expected path ending with 'opencode', got: {}",
            dir.display()
        );
    }

    #[cfg(unix)]
    #[test]
    fn opencode_data_dir_under_local_share() {
        let dir = opencode_data_dir();
        let s = dir.to_string_lossy();
        assert!(
            s.contains(".local/share/opencode"),
            "expected .local/share/opencode in path, got: {s}"
        );
    }
}
