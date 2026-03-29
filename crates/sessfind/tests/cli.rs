use assert_cmd::Command;
use predicates::prelude::*;

fn sessfind() -> Command {
    Command::cargo_bin("sessfind").unwrap()
}

// ── Help & basic CLI ──

#[test]
fn help_flag() {
    sessfind()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Search past AI coding assistant sessions",
        ));
}

#[test]
fn version_flag() {
    sessfind().arg("--version").assert().success();
}

// ── Stats ──

#[test]
fn stats_runs() {
    sessfind()
        .arg("stats")
        .assert()
        .success()
        .stdout(predicate::str::contains("Indexed sessions"));
}

// ── Index ──

#[test]
fn index_all() {
    sessfind()
        .args(["index", "--source", "all"])
        .assert()
        .success();
}

#[test]
fn index_unknown_source() {
    sessfind()
        .args(["index", "--source", "nonexistent"])
        .assert()
        .success(); // warns but doesn't fail
}

// ── Search ──

#[test]
fn search_no_results() {
    sessfind()
        .args(["search", "zzz_nonexistent_query_xyz_12345"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No results found"));
}

#[test]
fn search_with_date_filter() {
    sessfind()
        .args(["search", "test", "--after", "2099-01-01"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No results found"));
}

#[test]
fn search_invalid_date() {
    sessfind()
        .args(["search", "test", "--after", "not-a-date"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid date"));
}

#[test]
fn search_with_source_filter() {
    sessfind()
        .args(["search", "test", "--source", "claude"])
        .assert()
        .success();
}

#[test]
fn search_with_method_flag() {
    sessfind()
        .args(["search", "test", "--method", "fts"])
        .assert()
        .success();
}

// ── Show ──

#[test]
fn show_nonexistent_session() {
    sessfind()
        .args(["show", "nonexistent-session-id-12345"])
        .assert()
        .success()
        .stderr(predicate::str::contains("No session found"));
}

// ── Dump chunks ──

#[test]
fn dump_chunks_outputs_jsonl() {
    let output = sessfind().arg("dump-chunks").output().unwrap();
    assert!(output.status.success());
    // Output should be empty or valid JSONL
    let stdout = String::from_utf8(output.stdout).unwrap();
    for line in stdout.lines() {
        if !line.is_empty() {
            assert!(
                serde_json::from_str::<serde_json::Value>(line).is_ok(),
                "Invalid JSON line: {line}"
            );
        }
    }
}

// ── Index flag ──

#[test]
fn index_flag_accepted() {
    sessfind()
        .arg("--index")
        .arg("--help") // just check flag is parsed, don't launch TUI
        .assert()
        .success();
}
