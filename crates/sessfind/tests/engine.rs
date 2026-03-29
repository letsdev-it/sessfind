use sessfind_common::Source;

// We need to test the engine through the binary since IndexEngine is not public.
// Use dump-chunks + search CLI commands to verify indexing works end-to-end.

#[test]
fn index_and_search_roundtrip() {
    use assert_cmd::Command;

    // Index first
    Command::cargo_bin("sessfind")
        .unwrap()
        .args(["index", "--source", "all"])
        .assert()
        .success();

    // Dump chunks should produce valid JSONL
    let output = Command::cargo_bin("sessfind")
        .unwrap()
        .arg("dump-chunks")
        .output()
        .unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    let chunk_count = stdout.lines().filter(|l| !l.is_empty()).count();

    // If there are chunks, search should find something with a common word
    if chunk_count > 0 {
        // Parse first chunk to get a word to search for
        let first_line = stdout.lines().next().unwrap();
        let chunk: serde_json::Value = serde_json::from_str(first_line).unwrap();
        let text = chunk["text"].as_str().unwrap_or("");

        // Get first meaningful word (>3 chars)
        if let Some(word) = text
            .split_whitespace()
            .find(|w| w.len() > 3 && w.chars().all(|c| c.is_alphanumeric()))
        {
            let search_out = Command::cargo_bin("sessfind")
                .unwrap()
                .args(["search", word, "--limit", "1"])
                .output()
                .unwrap();
            assert!(search_out.status.success());
        }
    }
}

#[test]
fn dump_chunks_are_valid_dump_chunk_structs() {
    use assert_cmd::Command;

    let output = Command::cargo_bin("sessfind")
        .unwrap()
        .arg("dump-chunks")
        .output()
        .unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    for line in stdout.lines() {
        if line.is_empty() {
            continue;
        }
        let chunk: sessfind_common::DumpChunk = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("Failed to parse DumpChunk: {e}\nLine: {line}"));

        // Validate structure
        assert!(!chunk.chunk_id.is_empty());
        assert!(!chunk.session_id.is_empty());
        assert!(!chunk.text.is_empty());
        // Source should be valid
        assert!(matches!(
            chunk.source,
            Source::ClaudeCode | Source::OpenCode | Source::Copilot | Source::Cursor
        ));
    }
}
