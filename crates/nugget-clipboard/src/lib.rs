use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::{Duration as ChronoDuration, Utc};
use nugget_core::{CaptureMethod, Confidence, Domain, InboxItem, KnowledgeType};
use nugget_inbox::Inbox;
use nugget_store::BrainStore;
use regex::Regex;
use serde::{Deserialize, Serialize};

// ── Types ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub capture_urls: bool,
    #[serde(default)]
    pub capture_text: bool,
    #[serde(default = "default_poll_interval")]
    pub poll_interval_ms: u64,
    #[serde(default = "default_ignore_domains")]
    pub ignore_domains: Vec<String>,
}

/// Top-level brain.yaml structure with optional clipboard section.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BrainConfig {
    #[serde(default)]
    pub clipboard: Option<ClipboardConfig>,
}

/// The clipboard monitor that polls for changes and creates inbox items.
pub struct ClipboardMonitor {
    config: ClipboardConfig,
    store: BrainStore,
    running: Arc<AtomicBool>,
}

// ── Helpers ──

fn default_true() -> bool {
    true
}

fn default_poll_interval() -> u64 {
    500
}

fn default_ignore_domains() -> Vec<String> {
    vec![
        "localhost".to_string(),
        "mail.google.com".to_string(),
        "accounts.google.com".to_string(),
    ]
}

impl Default for ClipboardConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            capture_urls: true,
            capture_text: false,
            poll_interval_ms: default_poll_interval(),
            ignore_domains: default_ignore_domains(),
        }
    }
}

/// Drop text shorter than 20 characters.
fn length_check(text: &str) -> bool {
    text.len() >= 20
}

/// Compute Shannon entropy of the text. Drop if entropy > 4.5 and text
/// contains no spaces (likely a password or token).
fn entropy_check(text: &str) -> bool {
    if text.contains(' ') {
        return true;
    }

    let entropy = shannon_entropy(text);
    entropy <= 4.5
}

/// Compute Shannon entropy of a string.
fn shannon_entropy(text: &str) -> f64 {
    let len = text.len() as f64;
    if len == 0.0 {
        return 0.0;
    }

    let mut freq: HashMap<char, usize> = HashMap::new();
    for c in text.chars() {
        *freq.entry(c).or_insert(0) += 1;
    }

    freq.values().fold(0.0, |acc, &count| {
        let p = count as f64 / len;
        acc - p * p.log2()
    })
}

/// Drop text that looks like code.
///
/// Heuristics:
/// - Lines starting with common code keywords
/// - High bracket density (ratio of `{}[]()` chars to total > 0.1)
fn code_detection(text: &str) -> bool {
    let code_prefixes = [
        "fn ",
        "def ",
        "class ",
        "import ",
        "const ",
        "let ",
        "var ",
        "function ",
    ];

    for line in text.lines() {
        let trimmed = line.trim_start();
        for prefix in &code_prefixes {
            if trimmed.starts_with(prefix) {
                return false;
            }
        }
    }

    let total = text.len() as f64;
    if total == 0.0 {
        return true;
    }
    let bracket_count = text
        .chars()
        .filter(|c| matches!(c, '{' | '}' | '[' | ']' | '(' | ')'))
        .count() as f64;
    if bracket_count / total > 0.1 {
        return false;
    }

    true
}

/// Extract the first URL from the text using a regex.
/// Returns `None` if no URL is found.
fn extract_url(text: &str) -> Option<String> {
    // unwrap is safe: this is a compile-time constant regex pattern
    let re = Regex::new(r#"https?://[^\s<>"{}|\\^\[\]`]+"#).unwrap();
    re.find(text).map(|m| m.as_str().to_string())
}

/// Check whether the URL's host is in the ignore list.
/// Returns `true` if the URL should be kept (not ignored).
fn domain_filter(url: &str, ignore_domains: &[String]) -> bool {
    let host = extract_host(url);
    match host {
        Some(h) => !ignore_domains
            .iter()
            .any(|d| h == *d || h.ends_with(&format!(".{}", d))),
        None => true,
    }
}

/// Extract the host portion from a URL string without a full URL parser.
fn extract_host(url: &str) -> Option<String> {
    let without_scheme = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;

    let host_port = without_scheme.split('/').next()?;
    let host = host_port.split(':').next()?;

    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
}

/// Check whether the URL already exists in the inbox within the last 24 hours.
/// Returns `true` if the URL is new (not a duplicate).
fn dedup_check(url: &str, inbox: &Inbox) -> Result<bool> {
    let cutoff = Utc::now() - ChronoDuration::hours(24);
    let entries = inbox.list()?;

    let is_dup = entries.iter().any(|entry| {
        entry.item.captured_at > cutoff && entry.item.source.as_ref().is_some_and(|s| s == url)
    });

    Ok(!is_dup)
}

/// Load clipboard config from brain.yaml, falling back to defaults.
pub fn load_config(brain_root: &Path) -> Result<ClipboardConfig> {
    let config_path = brain_root.join("brain.yaml");
    if !config_path.exists() {
        return Ok(ClipboardConfig::default());
    }

    let content = fs::read_to_string(&config_path)
        .with_context(|| format!("reading brain.yaml: {}", config_path.display()))?;

    if content.trim().is_empty() || content.trim().starts_with('#') {
        return Ok(ClipboardConfig::default());
    }

    let brain_config: BrainConfig = serde_yaml::from_str(&content).unwrap_or_default();

    Ok(brain_config.clipboard.unwrap_or_default())
}

// ── Public API ──

/// Run the full filter pipeline on clipboard text.
///
/// Returns the extracted URL if all filters pass, or `None` if any filter
/// drops the text.
pub fn run_filter_pipeline(
    text: &str,
    config: &ClipboardConfig,
    inbox: &Inbox,
) -> Result<Option<String>> {
    if !length_check(text) {
        log::debug!("filter pipeline: dropped by length check");
        return Ok(None);
    }

    if !entropy_check(text) {
        log::debug!("filter pipeline: dropped by entropy check");
        return Ok(None);
    }

    if !code_detection(text) {
        log::debug!("filter pipeline: dropped by code detection");
        return Ok(None);
    }

    let url = match extract_url(text) {
        Some(u) => u,
        None => {
            log::debug!("filter pipeline: no url found");
            return Ok(None);
        }
    };

    if !domain_filter(&url, &config.ignore_domains) {
        log::debug!("filter pipeline: dropped by domain filter");
        return Ok(None);
    }

    if !dedup_check(&url, inbox)? {
        log::debug!("filter pipeline: dropped by dedup check");
        return Ok(None);
    }

    Ok(Some(url))
}

impl ClipboardMonitor {
    pub fn new(config: ClipboardConfig, store: BrainStore) -> Self {
        Self {
            config,
            store,
            running: Arc::new(AtomicBool::new(true)),
        }
    }

    /// Return a handle to the running flag so the monitor can be stopped
    /// externally.
    pub fn running_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.running)
    }

    /// Run the clipboard monitor loop. Polls the system clipboard at the
    /// configured interval and creates inbox items for captured URLs.
    pub fn run(&self) -> Result<()> {
        let mut clipboard =
            arboard::Clipboard::new().context("failed to initialize clipboard access")?;
        let inbox = Inbox::new(self.store.clone());
        let interval = Duration::from_millis(self.config.poll_interval_ms);

        let mut last_seen = String::new();

        log::info!(
            "clipboard monitor started, poll_interval_ms={}",
            self.config.poll_interval_ms
        );

        while self.running.load(Ordering::Relaxed) {
            thread::sleep(interval);

            let text = match clipboard.get_text() {
                Ok(t) => t,
                Err(_) => continue,
            };

            if text == last_seen {
                continue;
            }
            last_seen.clone_from(&text);

            log::debug!("clipboard changed, running filter pipeline");

            match run_filter_pipeline(&text, &self.config, &inbox) {
                Ok(Some(url)) => {
                    let mut item = InboxItem::new(
                        KnowledgeType::Concept,
                        Domain("inbox".to_string()),
                        CaptureMethod::ClipboardUrl,
                        url.clone(),
                    );
                    item.source = Some(url.clone());
                    item.tags = Vec::new();
                    item.confidence = Confidence::new(0.5);

                    match inbox.add(&item) {
                        Ok(path) => {
                            log::info!("captured url to inbox: {}", path.display());
                        }
                        Err(e) => {
                            log::error!("failed to add inbox item: {}", e);
                        }
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    log::error!("filter pipeline error: {}", e);
                }
            }
        }

        log::info!("clipboard monitor stopped");
        Ok(())
    }
}

/// Return the path to the PID file for the clipboard daemon.
pub fn pid_file_path(brain_root: &Path) -> PathBuf {
    brain_root.join(".nugget").join("clipboard.pid")
}

/// Start the clipboard daemon as a background process.
///
/// Spawns the current executable with `daemon run --brain <path>` and writes
/// the child PID to the pid file.
pub fn daemon_start(brain_root: &Path) -> Result<()> {
    let pid_path = pid_file_path(brain_root);

    if pid_path.exists() {
        if daemon_status(brain_root)? {
            anyhow::bail!(
                "daemon is already running (pid file: {})",
                pid_path.display()
            );
        }
        // Stale PID file — remove it
        fs::remove_file(&pid_path)
            .with_context(|| format!("removing stale pid file: {}", pid_path.display()))?;
    }

    let nugget_dir = brain_root.join(".nugget");
    fs::create_dir_all(&nugget_dir)
        .with_context(|| format!("creating .nugget directory: {}", nugget_dir.display()))?;

    let exe = std::env::current_exe().context("determining current executable path")?;
    let brain_str = brain_root
        .to_str()
        .context("brain root path is not valid UTF-8")?;

    let child = std::process::Command::new(exe)
        .args(["daemon", "run", "--brain", brain_str])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("spawning daemon process")?;

    let pid = child.id();
    fs::write(&pid_path, pid.to_string())
        .with_context(|| format!("writing pid file: {}", pid_path.display()))?;

    println!("daemon started (pid {})", pid);
    Ok(())
}

/// Stop the clipboard daemon by reading the PID file and sending SIGTERM.
pub fn daemon_stop(brain_root: &Path) -> Result<()> {
    let pid_path = pid_file_path(brain_root);

    if !pid_path.exists() {
        anyhow::bail!("no pid file found — daemon does not appear to be running");
    }

    let pid_str = fs::read_to_string(&pid_path)
        .with_context(|| format!("reading pid file: {}", pid_path.display()))?;
    let pid = pid_str
        .trim()
        .parse::<u32>()
        .with_context(|| format!("invalid pid in file: '{}'", pid_str.trim()))?;

    let status = std::process::Command::new("kill")
        .arg(pid.to_string())
        .output()
        .context("sending SIGTERM to daemon")?;

    if !status.status.success() {
        let stderr = String::from_utf8_lossy(&status.stderr);
        log::debug!("kill failed: {}", stderr);
    }

    fs::remove_file(&pid_path)
        .with_context(|| format!("removing pid file: {}", pid_path.display()))?;

    println!("daemon stopped (pid {})", pid);
    Ok(())
}

/// Check whether the clipboard daemon is running.
///
/// Returns `true` if the process identified by the PID file is alive.
pub fn daemon_status(brain_root: &Path) -> Result<bool> {
    let pid_path = pid_file_path(brain_root);

    if !pid_path.exists() {
        return Ok(false);
    }

    let pid_str = fs::read_to_string(&pid_path)
        .with_context(|| format!("reading pid file: {}", pid_path.display()))?;
    let pid = pid_str
        .trim()
        .parse::<u32>()
        .with_context(|| format!("invalid pid in file: '{}'", pid_str.trim()))?;

    let output = std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .output()
        .context("checking if daemon process is alive")?;

    Ok(output.status.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_length_check() {
        assert!(!length_check("short"));
        assert!(!length_check("less than twenty"));
        assert!(length_check(
            "this string is definitely longer than twenty characters"
        ));
    }

    #[test]
    fn test_entropy_check() {
        // Normal English text with spaces should always pass
        assert!(entropy_check("hello world this is a normal sentence"));

        // Low-entropy no-space text (repeated chars) should pass
        assert!(entropy_check("aaaaaaaaaaaaaaaa"));

        // High-entropy no-space text (random-looking) should be dropped
        assert!(!entropy_check("aB3$xZ9!qW7&mK2@pL5^rT8#vN4%dE6"));

        // High-entropy but with spaces should pass (spaces bypass the check)
        assert!(entropy_check("aB3$ xZ9! qW7& mK2@ pL5^ rT8#"));
    }

    #[test]
    fn test_code_detection() {
        // Code snippets should be dropped
        assert!(!code_detection("fn main() {\n    println!(\"hello\");\n}"));
        assert!(!code_detection(
            "def process_data(items):\n    return items"
        ));
        assert!(!code_detection("import os\nimport sys"));
        assert!(!code_detection("const FOO = 42;\nlet bar = 10;"));
        assert!(!code_detection("class MyWidget extends StatelessWidget {"));
        assert!(!code_detection("function doSomething() { return true; }"));

        // High bracket density should be dropped
        assert!(!code_detection("{{[]()}}{{[]()}}"));

        // Normal text should pass
        assert!(code_detection(
            "Check out this article about Rust programming language"
        ));
    }

    #[test]
    fn test_extract_url() {
        assert_eq!(
            extract_url("Check out https://example.com/article today"),
            Some("https://example.com/article".to_string())
        );
        assert_eq!(
            extract_url("http://foo.bar/baz?q=1&r=2"),
            Some("http://foo.bar/baz?q=1&r=2".to_string())
        );
        assert_eq!(extract_url("no urls here, just plain text"), None);
        assert_eq!(extract_url(""), None);
    }

    #[test]
    fn test_domain_filter() {
        let ignore = default_ignore_domains();

        assert!(domain_filter("https://example.com/page", &ignore));
        assert!(!domain_filter("https://localhost/api", &ignore));
        assert!(!domain_filter("https://mail.google.com/inbox", &ignore));
        assert!(!domain_filter(
            "https://accounts.google.com/signin",
            &ignore
        ));
        assert!(domain_filter("https://docs.google.com/doc", &ignore));
    }

    #[test]
    fn test_config_defaults() {
        let config = ClipboardConfig::default();
        assert!(config.enabled);
        assert!(config.capture_urls);
        assert!(!config.capture_text);
        assert_eq!(config.poll_interval_ms, 500);
        assert_eq!(
            config.ignore_domains,
            vec![
                "localhost".to_string(),
                "mail.google.com".to_string(),
                "accounts.google.com".to_string(),
            ]
        );
    }

    #[test]
    fn test_filter_pipeline() {
        let tmp = tempfile::tempdir().unwrap();
        let store = BrainStore::new(tmp.path().join("brain"));
        store.init().unwrap();
        let inbox = Inbox::new(store);
        let config = ClipboardConfig::default();

        // A valid URL passes the pipeline
        let result = run_filter_pipeline(
            "Check out this link: https://example.com/article/123",
            &config,
            &inbox,
        )
        .unwrap();
        assert_eq!(result, Some("https://example.com/article/123".to_string()));

        // Short text is dropped
        let result = run_filter_pipeline("short", &config, &inbox).unwrap();
        assert!(result.is_none());

        // Code is dropped
        let result = run_filter_pipeline(
            "fn main() { println!(\"hello\"); } and some more text to pass length",
            &config,
            &inbox,
        )
        .unwrap();
        assert!(result.is_none());

        // Text without a URL is dropped
        let result = run_filter_pipeline(
            "This is just some regular text without any URL in it at all",
            &config,
            &inbox,
        )
        .unwrap();
        assert!(result.is_none());

        // Ignored domain is dropped
        let result = run_filter_pipeline(
            "Check this out: https://mail.google.com/inbox/message/12345",
            &config,
            &inbox,
        )
        .unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_host() {
        assert_eq!(
            extract_host("https://example.com/path"),
            Some("example.com".to_string())
        );
        assert_eq!(
            extract_host("http://localhost:8080/api"),
            Some("localhost".to_string())
        );
        assert_eq!(
            extract_host("https://sub.domain.com"),
            Some("sub.domain.com".to_string())
        );
        assert_eq!(extract_host("not-a-url"), None);
    }

    #[test]
    fn test_shannon_entropy() {
        // Single repeated character has zero entropy
        let e = shannon_entropy("aaaa");
        assert!((e - 0.0).abs() < 0.001);

        // Two equally distributed characters have entropy of 1.0
        let e = shannon_entropy("aabb");
        assert!((e - 1.0).abs() < 0.001);

        // Empty string has zero entropy
        assert_eq!(shannon_entropy(""), 0.0);
    }

    #[test]
    fn test_pid_file_path() {
        let path = pid_file_path(Path::new("/tmp/brain"));
        assert_eq!(path, PathBuf::from("/tmp/brain/.nugget/clipboard.pid"));
    }
}
