use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use chrono::Utc;
use nugget_core::{Domain, InboxItem};
use nugget_store::BrainStore;

// ── Types ──

/// An inbox entry: the parsed item plus its file path on disk.
#[derive(Debug)]
pub struct InboxEntry {
    pub item: InboxItem,
    pub path: PathBuf,
}

/// Manages the inbox directory within a brain.
#[derive(Debug, Clone)]
pub struct Inbox {
    store: BrainStore,
}

// ── Helpers ──

/// Split a markdown file into YAML frontmatter and body.
fn split_frontmatter(content: &str) -> (Option<&str>, &str) {
    if !content.starts_with("---\n") {
        return (None, content);
    }
    if let Some(end) = content[4..].find("\n---") {
        let yaml = &content[4..4 + end];
        let body_start = 4 + end + 4;
        let body = if body_start < content.len() {
            content[body_start..].trim_start_matches('\n')
        } else {
            ""
        };
        (Some(yaml), body)
    } else {
        (None, content)
    }
}

fn inbox_filename(item: &InboxItem) -> String {
    let ts = item.captured_at.format("%Y%m%d-%H%M%S");
    let short_id = &item.id[..8];
    format!("{}-{}.md", ts, short_id)
}

fn render_inbox_file(item: &InboxItem) -> Result<String> {
    let frontmatter = serde_yaml::to_string(item).context("serializing inbox frontmatter")?;
    Ok(format!("---\n{}---\n\n{}\n", frontmatter, item.body))
}

fn render_knowledge_file(item: &InboxItem, domain: &Domain) -> Result<String> {
    let unit = item.clone().into_knowledge_unit(domain.clone());
    let frontmatter = serde_yaml::to_string(&unit).context("serializing knowledge frontmatter")?;
    Ok(format!("---\n{}---\n\n{}\n", frontmatter, unit.body))
}

// ── Public API ──

impl Inbox {
    pub fn new(store: BrainStore) -> Self {
        Self { store }
    }

    /// Write a new inbox item to disk.
    pub fn add(&self, item: &InboxItem) -> Result<PathBuf> {
        let inbox_dir = self.store.inbox_path();
        fs::create_dir_all(&inbox_dir).context("creating inbox directory")?;

        let filename = inbox_filename(item);
        let path = inbox_dir.join(&filename);
        let content = render_inbox_file(item)?;
        fs::write(&path, content)
            .with_context(|| format!("writing inbox item: {}", path.display()))?;

        Ok(path)
    }

    /// List all inbox items, sorted by captured_at (oldest first).
    pub fn list(&self) -> Result<Vec<InboxEntry>> {
        let inbox_dir = self.store.inbox_path();
        if !inbox_dir.exists() {
            return Ok(Vec::new());
        }

        let mut entries = Vec::new();
        for entry in fs::read_dir(&inbox_dir).context("reading inbox directory")? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            match self.read_inbox_file(&path) {
                Ok(item) => entries.push(InboxEntry { item, path }),
                Err(e) => {
                    eprintln!("warning: skipping malformed inbox item {}: {}", path.display(), e);
                }
            }
        }

        entries.sort_by(|a, b| a.item.captured_at.cmp(&b.item.captured_at));
        Ok(entries)
    }

    /// Accept an inbox item: move it from inbox/ to the target domain path.
    pub fn accept(&self, entry: &InboxEntry) -> Result<PathBuf> {
        let domain = &entry.item.suggested_domain;
        let domain_dir = self.store.root.join(&domain.0);
        fs::create_dir_all(&domain_dir)
            .with_context(|| format!("creating domain dir: {}", domain_dir.display()))?;

        // Generate a filename from the item
        let slug = slug_from_body(&entry.item.body);
        let filename = format!("{}.md", slug);
        let dest = domain_dir.join(&filename);

        let content = render_knowledge_file(&entry.item, domain)?;
        fs::write(&dest, content)
            .with_context(|| format!("writing accepted file: {}", dest.display()))?;

        fs::remove_file(&entry.path)
            .with_context(|| format!("removing inbox file: {}", entry.path.display()))?;

        Ok(dest)
    }

    /// Reject an inbox item: delete it from the inbox.
    pub fn reject(&self, entry: &InboxEntry) -> Result<()> {
        fs::remove_file(&entry.path)
            .with_context(|| format!("removing rejected inbox file: {}", entry.path.display()))?;
        Ok(())
    }

    /// Accept multiple inbox items by their 1-based indices.
    pub fn accept_by_indices(&self, indices: &[usize]) -> Result<Vec<PathBuf>> {
        let entries = self.list()?;
        let mut accepted = Vec::new();

        for &idx in indices {
            if idx == 0 || idx > entries.len() {
                bail!("index {} out of range (1-{})", idx, entries.len());
            }
            let dest = self.accept(&entries[idx - 1])?;
            accepted.push(dest);
        }

        Ok(accepted)
    }

    /// Reject multiple inbox items by their 1-based indices.
    pub fn reject_by_indices(&self, indices: &[usize]) -> Result<()> {
        let entries = self.list()?;
        // Process in reverse order so indices remain valid after deletion.
        let mut sorted_indices: Vec<usize> = indices.to_vec();
        sorted_indices.sort_unstable();
        sorted_indices.dedup();

        for &idx in sorted_indices.iter().rev() {
            if idx == 0 || idx > entries.len() {
                bail!("index {} out of range (1-{})", idx, entries.len());
            }
            self.reject(&entries[idx - 1])?;
        }

        Ok(())
    }

    fn read_inbox_file(&self, path: &Path) -> Result<InboxItem> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("reading inbox file: {}", path.display()))?;

        let (frontmatter, body) = split_frontmatter(&content);
        let yaml = frontmatter.context("no frontmatter found in inbox file")?;

        let mut item: InboxItem = serde_yaml::from_str(yaml).context("parsing inbox frontmatter")?;
        item.body = body.to_string();

        Ok(item)
    }
}

/// Generate a slug from the first line of the body text.
fn slug_from_body(body: &str) -> String {
    let first_line = body.lines().next().unwrap_or("untitled");
    let slug: String = first_line
        .chars()
        .take(60)
        .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect();
    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        let ts = Utc::now().format("%Y%m%d-%H%M%S");
        format!("item-{}", ts)
    } else {
        slug
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nugget_core::*;

    fn make_test_item(body: &str) -> InboxItem {
        InboxItem::new(
            KnowledgeType::Concept,
            Domain("coding/rust".to_string()),
            CaptureMethod::Manual,
            body.to_string(),
        )
    }

    #[test]
    fn test_add_and_list() {
        let tmp = tempfile::tempdir().unwrap();
        let store = BrainStore::new(tmp.path().join("brain"));
        store.init().unwrap();
        let inbox = Inbox::new(store);

        let item = make_test_item("Rust ownership rules");
        inbox.add(&item).unwrap();

        let entries = inbox.list().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].item.body, "Rust ownership rules\n");
    }

    #[test]
    fn test_accept() {
        let tmp = tempfile::tempdir().unwrap();
        let store = BrainStore::new(tmp.path().join("brain"));
        store.init().unwrap();
        let inbox = Inbox::new(store);

        let item = make_test_item("Rust ownership rules");
        inbox.add(&item).unwrap();

        let entries = inbox.list().unwrap();
        let dest = inbox.accept(&entries[0]).unwrap();
        assert!(dest.exists());

        let remaining = inbox.list().unwrap();
        assert!(remaining.is_empty());
    }

    #[test]
    fn test_reject() {
        let tmp = tempfile::tempdir().unwrap();
        let store = BrainStore::new(tmp.path().join("brain"));
        store.init().unwrap();
        let inbox = Inbox::new(store);

        let item = make_test_item("Something not useful");
        inbox.add(&item).unwrap();

        let entries = inbox.list().unwrap();
        inbox.reject(&entries[0]).unwrap();

        let remaining = inbox.list().unwrap();
        assert!(remaining.is_empty());
    }

    #[test]
    fn test_slug_from_body() {
        assert_eq!(slug_from_body("Hello World!"), "hello-world");
        // All-whitespace input produces an empty slug, which falls back to a timestamped name.
        assert!(slug_from_body("  ").starts_with("item-"));
    }
}
