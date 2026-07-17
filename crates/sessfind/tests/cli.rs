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

// ── JSON API ──

#[test]
fn capabilities_outputs_json() {
    let output = sessfind().arg("capabilities").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let caps: sessfind_common::Capabilities = serde_json::from_str(&stdout).unwrap();
    assert_eq!(caps.json_api_version, 1);
    assert!(caps.search_methods.fts);
    assert!(caps.features.iter().any(|f| f == "sessions-list"));
}

#[test]
fn search_json_no_results_prints_empty_array() {
    let output = sessfind()
        .args(["search", "zzz_nonexistent_query_xyz_12345", "--json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let results: Vec<sessfind_common::SearchResult> = serde_json::from_str(&stdout).unwrap();
    assert!(results.is_empty());
}

#[test]
fn sessions_list_json_parses() {
    let output = sessfind()
        .args(["sessions", "list", "--json", "--limit", "5"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let sessions: Vec<sessfind_common::SessionSummary> = serde_json::from_str(&stdout).unwrap();
    assert!(sessions.len() <= 5);
    for s in &sessions {
        assert!(!s.resume.args.is_empty());
        assert!(!s.new_session.args.is_empty());
    }
}

#[test]
fn sessions_list_invalid_sort_fails() {
    sessfind()
        .args(["sessions", "list", "--sort", "bogus"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid sort order"));
}

#[test]
fn projects_list_json_parses() {
    let output = sessfind()
        .args(["projects", "list", "--json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let projects: Vec<sessfind_common::ProjectGroup> = serde_json::from_str(&stdout).unwrap();
    for p in &projects {
        assert!(p.session_count > 0);
        assert!(!p.sources.is_empty());
    }
}

#[test]
fn show_json_nonexistent_session_fails() {
    sessfind()
        .args(["show", "nonexistent-session-id-12345", "--json"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No session found"));
}

#[test]
fn stats_json_parses() {
    let output = sessfind().args(["stats", "--json"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stats: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(stats["sessions"]["total"].is_number());
    assert!(stats["semantic"]["available"].is_boolean());
}

// ── Tags & user projects ──

#[test]
fn tag_add_unknown_session_fails() {
    sessfind()
        .args(["tag", "add", "definitely-not-a-real-session-id", "work"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No indexed session"));
}

#[test]
fn tag_list_json_parses() {
    let output = sessfind().args(["tag", "list", "--json"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let _tags: Vec<sessfind_common::TagCount> = serde_json::from_str(&stdout).unwrap();
}

#[test]
fn project_list_json_parses() {
    let output = sessfind()
        .args(["project", "list", "--json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let _projects: Vec<sessfind_common::UserProject> = serde_json::from_str(&stdout).unwrap();
}

#[test]
fn project_show_unknown_fails() {
    sessfind()
        .args(["project", "show", "no-such-project-xyz"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No project named"));
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
