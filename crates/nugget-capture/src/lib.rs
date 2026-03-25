use std::path::PathBuf;

use anyhow::Result;
use nugget_core::{CaptureMethod, Domain, InboxItem, KnowledgeType, Tag};
use nugget_inbox::Inbox;
use nugget_store::BrainStore;

// ── Types ──

// (no additional types needed)

// ── Helpers ──

/// Suggest a domain based on keywords found in the context string.
fn suggest_domain(context: Option<&str>) -> Domain {
    let ctx = match context {
        Some(s) => s.to_lowercase(),
        None => return Domain("general".to_string()),
    };

    if ctx.contains("rust") {
        Domain("coding/rust".to_string())
    } else if ctx.contains("python") {
        Domain("coding/python".to_string())
    } else if ctx.contains("javascript") || ctx.contains("typescript") || ctx.contains("react") {
        Domain("coding/javascript".to_string())
    } else if ctx.contains("golang")
        || ctx.contains(" go ")
        || ctx.starts_with("go ")
        || ctx.ends_with(" go")
    {
        Domain("coding/go".to_string())
    } else {
        Domain("general".to_string())
    }
}

// ── Public API ──

/// Capture learnings and decisions from an AI conversation session.
///
/// Creates inbox items for each learning (as Pattern) and each decision (as Decision),
/// tagging them with the session summary as capture context.
pub fn capture_from_conversation(
    store: &BrainStore,
    summary: &str,
    learnings: &[String],
    decisions: &[String],
    context: Option<&str>,
) -> Result<Vec<PathBuf>> {
    let inbox = Inbox::new(store.clone());
    let domain = suggest_domain(context);
    let mut paths = Vec::new();

    for learning in learnings {
        let mut item = InboxItem::new(
            KnowledgeType::Pattern,
            domain.clone(),
            CaptureMethod::AiSession,
            learning.clone(),
        );
        item.capture_context = Some(summary.to_string());
        item.source = context.map(String::from);
        let path = inbox.add(&item)?;
        paths.push(path);
    }

    for decision in decisions {
        let mut item = InboxItem::new(
            KnowledgeType::Decision,
            domain.clone(),
            CaptureMethod::AiSession,
            decision.clone(),
        );
        item.capture_context = Some(summary.to_string());
        item.source = context.map(String::from);
        let path = inbox.add(&item)?;
        paths.push(path);
    }

    Ok(paths)
}

/// Capture a knowledge item from a URL (web capture).
///
/// Creates a Concept inbox item with the URL as source, formatted body with
/// title and summary, and optional tags and domain.
pub fn capture_from_url(
    store: &BrainStore,
    url: &str,
    title: &str,
    summary: &str,
    tags: &[String],
    domain: Option<&str>,
) -> Result<PathBuf> {
    let inbox = Inbox::new(store.clone());
    let domain = Domain(domain.unwrap_or("general").to_string());
    let body = format!("# {}\n\n{}", title, summary);

    let mut item = InboxItem::new(
        KnowledgeType::Concept,
        domain,
        CaptureMethod::WebCapture,
        body,
    );
    item.source = Some(url.to_string());
    item.tags = tags.iter().map(|t| Tag(t.clone())).collect();

    inbox.add(&item)
}

/// Capture a plain text knowledge item.
///
/// Creates a Concept inbox item with the provided text, optional source, and
/// optional domain.
pub fn capture_from_text(
    store: &BrainStore,
    text: &str,
    source: Option<&str>,
    domain: Option<&str>,
) -> Result<PathBuf> {
    let inbox = Inbox::new(store.clone());
    let domain = Domain(domain.unwrap_or("general").to_string());

    let mut item = InboxItem::new(
        KnowledgeType::Concept,
        domain,
        CaptureMethod::Manual,
        text.to_string(),
    );
    item.source = source.map(String::from);

    inbox.add(&item)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_store() -> (tempfile::TempDir, BrainStore) {
        let tmp = tempfile::tempdir().expect("failed to create temp dir");
        let store = BrainStore::new(tmp.path().join("brain"));
        store.init().expect("failed to init store");
        (tmp, store)
    }

    #[test]
    fn test_capture_from_conversation() {
        let (_tmp, store) = setup_store();
        let learnings = vec![
            "Rust ownership prevents data races".to_string(),
            "Lifetimes track reference validity".to_string(),
        ];
        let decisions = vec!["Use Arc<Mutex<T>> for shared state".to_string()];

        let paths = capture_from_conversation(
            &store,
            "Discussed Rust concurrency",
            &learnings,
            &decisions,
            Some("rust programming session"),
        )
        .expect("capture failed");

        assert_eq!(paths.len(), 3);
        for path in &paths {
            assert!(path.exists());
        }

        // Verify items are in the inbox with correct types
        let inbox = Inbox::new(store);
        let entries = inbox.list().expect("list failed");
        assert_eq!(entries.len(), 3);

        // First two should be Pattern (learnings)
        assert_eq!(entries[0].item.knowledge_type, KnowledgeType::Pattern);
        assert_eq!(entries[1].item.knowledge_type, KnowledgeType::Pattern);
        // Last should be Decision
        assert_eq!(entries[2].item.knowledge_type, KnowledgeType::Decision);

        // All should have coding/rust domain because context contains "rust"
        assert_eq!(entries[0].item.suggested_domain.0, "coding/rust");
    }

    #[test]
    fn test_capture_from_url() {
        let (_tmp, store) = setup_store();
        let tags = vec!["rust".to_string(), "async".to_string()];

        let path = capture_from_url(
            &store,
            "https://example.com/article",
            "Understanding Async Rust",
            "A deep dive into async/await in Rust.",
            &tags,
            Some("coding/rust"),
        )
        .expect("capture failed");

        assert!(path.exists());

        let inbox = Inbox::new(store);
        let entries = inbox.list().expect("list failed");
        assert_eq!(entries.len(), 1);

        let item = &entries[0].item;
        assert_eq!(item.knowledge_type, KnowledgeType::Concept);
        assert_eq!(item.source.as_deref(), Some("https://example.com/article"));
        assert_eq!(item.tags.len(), 2);
        assert!(item.body.contains("# Understanding Async Rust"));
        assert!(item.body.contains("A deep dive into async/await in Rust."));
    }

    #[test]
    fn test_capture_from_text() {
        let (_tmp, store) = setup_store();

        let path = capture_from_text(
            &store,
            "Always handle errors with Result<T, E>",
            Some("code review feedback"),
            Some("coding/rust"),
        )
        .expect("capture failed");

        assert!(path.exists());

        let inbox = Inbox::new(store);
        let entries = inbox.list().expect("list failed");
        assert_eq!(entries.len(), 1);

        let item = &entries[0].item;
        assert_eq!(item.knowledge_type, KnowledgeType::Concept);
        assert_eq!(item.capture_method, CaptureMethod::Manual);
        assert_eq!(item.source.as_deref(), Some("code review feedback"));
        assert_eq!(item.suggested_domain.0, "coding/rust");
    }

    #[test]
    fn test_suggest_domain() {
        assert_eq!(suggest_domain(Some("rust programming")).0, "coding/rust");
        assert_eq!(suggest_domain(Some("python scripts")).0, "coding/python");
        assert_eq!(
            suggest_domain(Some("javascript frameworks")).0,
            "coding/javascript"
        );
        assert_eq!(
            suggest_domain(Some("typescript types")).0,
            "coding/javascript"
        );
        assert_eq!(
            suggest_domain(Some("react components")).0,
            "coding/javascript"
        );
        assert_eq!(suggest_domain(Some("golang concurrency")).0, "coding/go");
        assert_eq!(suggest_domain(Some("random topic")).0, "general");
        assert_eq!(suggest_domain(None).0, "general");
    }
}
