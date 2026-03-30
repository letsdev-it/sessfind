use std::sync::mpsc;
use std::thread;

pub fn check_latest_version_async() -> mpsc::Receiver<Option<String>> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let result = check_latest_version();
        let _ = tx.send(result);
    });
    rx
}

fn check_latest_version() -> Option<String> {
    let body = ureq::get("https://index.crates.io/se/ss/sessfind")
        .call()
        .ok()?
        .body_mut()
        .read_to_string()
        .ok()?;

    body.lines()
        .rev()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find(|v| !v["yanked"].as_bool().unwrap_or(true))
        .and_then(|v| v["vers"].as_str().map(String::from))
}
