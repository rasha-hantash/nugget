use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use nugget_core::{CaptureMethod, Domain, InboxItem, KnowledgeType};
use nugget_inbox::Inbox;
use nugget_store::BrainStore;
use regex::Regex;

// ── Types ──

/// Summary of a Notion import operation.
#[derive(Debug)]
pub struct ImportSummary {
    pub imported: usize,
    pub skipped: usize,
}

// ── Helpers ──

/// Strip Notion's UUID suffix from a filename (e.g., "My Page abc123...def" -> "My Page").
pub fn clean_notion_title(filename: &str) -> String {
    // Notion appends a space + 32-char hex ID to filenames
    let re = Regex::new(r"\s+[a-f0-9]{32}$").unwrap(); // safe: literal regex
    re.replace(filename, "").to_string()
}

/// Derive a suggested domain from the folder structure relative to the export root.
pub fn suggest_domain_from_path(path: &Path, root: &Path) -> String {
    let relative = path
        .parent()
        .and_then(|p| p.strip_prefix(root).ok())
        .unwrap_or(Path::new(""));

    if relative.as_os_str().is_empty() {
        return "imported".to_string();
    }

    let parts: Vec<String> = relative
        .components()
        .map(|c| {
            let s = c.as_os_str().to_string_lossy().to_string();
            let cleaned = clean_notion_title(&s);
            cleaned.to_lowercase().replace(' ', "-")
        })
        .collect();

    format!("imported/{}", parts.join("/"))
}

/// Extract a title from markdown content, falling back to the cleaned filename.
pub fn extract_title(content: &str, filename: &str) -> String {
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(heading) = trimmed.strip_prefix("# ") {
            let title = heading.trim();
            if !title.is_empty() {
                return title.to_string();
            }
        }
    }
    clean_notion_title(filename)
}

// ── Public API ──

/// Import markdown files from a Notion export directory into the brain inbox.
pub fn import_notion(store: &BrainStore, export_dir: &Path) -> Result<ImportSummary> {
    let export_dir = export_dir
        .canonicalize()
        .with_context(|| format!("resolving export directory: {}", export_dir.display()))?;

    let inbox = Inbox::new(store.clone());
    let mut imported = 0;
    let mut skipped = 0;

    for entry in walkdir::WalkDir::new(&export_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }

        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };

        // Skip empty/near-empty Notion pages
        if content.trim().len() < 10 {
            skipped += 1;
            continue;
        }

        let filename = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("untitled");

        let title = extract_title(&content, filename);
        let domain = suggest_domain_from_path(path, &export_dir);
        let source = format!(
            "notion:{}",
            path.strip_prefix(&export_dir)
                .unwrap_or(path)
                .display()
        );

        let body = format!("# {}\n\n{}", title, content);

        let mut item = InboxItem::new(
            KnowledgeType::Concept,
            Domain(domain),
            CaptureMethod::Import,
            body,
        );
        item.source = Some(source);

        inbox
            .add(&item)
            .with_context(|| format!("adding inbox item for: {}", path.display()))?;
        imported += 1;
    }

    Ok(ImportSummary { imported, skipped })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_notion_title() {
        assert_eq!(
            clean_notion_title("My Page a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4"),
            "My Page"
        );
        assert_eq!(
            clean_notion_title("Simple Title"),
            "Simple Title"
        );
        // Exact 32 hex chars after a space
        assert_eq!(
            clean_notion_title("Doc aabbccdd112233445566778899001122"),
            "Doc"
        );
        // Not 32 hex chars — no change
        assert_eq!(
            clean_notion_title("Doc aabb"),
            "Doc aabb"
        );
    }

    #[test]
    fn test_suggest_domain_from_path() {
        let root = Path::new("/export");

        assert_eq!(
            suggest_domain_from_path(Path::new("/export/page.md"), root),
            "imported"
        );
        assert_eq!(
            suggest_domain_from_path(Path::new("/export/Work/page.md"), root),
            "imported/work"
        );
        assert_eq!(
            suggest_domain_from_path(
                Path::new("/export/Work/Engineering/page.md"),
                root
            ),
            "imported/work/engineering"
        );
        // Notion UUID folders get cleaned
        assert_eq!(
            suggest_domain_from_path(
                Path::new("/export/Work aabbccdd112233445566778899001122/page.md"),
                root
            ),
            "imported/work"
        );
    }

    #[test]
    fn test_extract_title() {
        assert_eq!(
            extract_title("# My Great Page\n\nSome content", "fallback"),
            "My Great Page"
        );
        assert_eq!(
            extract_title("No heading here\nJust text", "My File a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4"),
            "My File"
        );
        assert_eq!(
            extract_title("## Not an h1\nContent", "Default Title"),
            "Default Title"
        );
        // Blank heading falls through to filename
        assert_eq!(
            extract_title("# \n\nContent", "Filename"),
            "Filename"
        );
    }

    #[test]
    fn test_import_notion() {
        let tmp = tempfile::tempdir().unwrap();
        let brain_dir = tmp.path().join("brain");
        let store = BrainStore::new(&brain_dir);
        store.init().unwrap();

        // Create a mock Notion export
        let export_dir = tmp.path().join("notion-export");
        let work_dir = export_dir.join("Work");
        let eng_dir = work_dir.join("Engineering");
        fs::create_dir_all(&eng_dir).unwrap();

        // Page with heading
        fs::write(
            export_dir.join("Getting Started aabbccdd112233445566778899001122.md"),
            "# Getting Started\n\nThis is a guide to getting started with Notion.\n",
        )
        .unwrap();

        // Nested page
        fs::write(
            eng_dir.join("Rust Notes.md"),
            "# Rust Notes\n\nOwnership, borrowing, and lifetimes are key concepts.\n",
        )
        .unwrap();

        // Page without heading (falls back to filename)
        fs::write(
            work_dir.join("Quick Thoughts.md"),
            "Just some random thoughts about the project direction.\n",
        )
        .unwrap();

        // Empty page (should be skipped)
        fs::write(export_dir.join("Empty.md"), "").unwrap();

        let summary = import_notion(&store, &export_dir).unwrap();
        assert_eq!(summary.imported, 3);
        assert_eq!(summary.skipped, 1);

        // Verify items are in the inbox
        let inbox = Inbox::new(store);
        let entries = inbox.list().unwrap();
        assert_eq!(entries.len(), 3);

        // Check that domains were derived from folder structure
        let domains: Vec<String> = entries
            .iter()
            .map(|e| e.item.suggested_domain.0.clone())
            .collect();
        assert!(domains.contains(&"imported".to_string()));
        assert!(domains.contains(&"imported/work".to_string()));
        assert!(domains.contains(&"imported/work/engineering".to_string()));

        // All should be Import capture method
        for entry in &entries {
            assert_eq!(entry.item.capture_method, CaptureMethod::Import);
            assert!(entry.item.source.as_ref().unwrap().starts_with("notion:"));
        }
    }
}
