// ── File I/O ──
//
// Read and write KnowledgeUnit files to disk.

use nugget_core::frontmatter;
use nugget_core::types::KnowledgeUnit;
use nugget_core::Result;
use std::fs;
use std::path::Path;

// ── Public API ──

/// Read a `.md` file and parse it into a `KnowledgeUnit`.
pub fn read_unit(path: &Path) -> Result<KnowledgeUnit> {
    let content = fs::read_to_string(path)?;
    let path_str = path.display().to_string();
    frontmatter::parse(&content, &path_str)
}

/// Serialize a `KnowledgeUnit` and write it to disk.
/// Creates parent directories if they don't exist.
pub fn write_unit(path: &Path, unit: &KnowledgeUnit) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = frontmatter::serialize(unit)?;
    fs::write(path, content)?;
    Ok(())
}

/// Walk the brain directory and parse all knowledge files.
/// Invalid files are skipped with a warning printed to stderr.
pub fn read_all_units(brain_path: &Path) -> Result<Vec<KnowledgeUnit>> {
    let files = crate::brain::walk_knowledge_files(brain_path)?;

    let mut units = Vec::new();
    for file_path in files {
        match read_unit(&file_path) {
            Ok(unit) => units.push(unit),
            Err(e) => {
                eprintln!("warning: skipping {}: {}", file_path.display(), e);
            }
        }
    }

    Ok(units)
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use nugget_core::types::{KnowledgeType, Relation, RelationType};
    use tempfile::TempDir;

    fn sample_unit() -> KnowledgeUnit {
        KnowledgeUnit {
            id: "test-unit".to_string(),
            kind: KnowledgeType::Pattern,
            domain: "rust".to_string(),
            tags: vec!["testing".to_string()],
            confidence: 0.9,
            source: "test".to_string(),
            related: vec![Relation {
                id: "other-unit".to_string(),
                relation: RelationType::Uses,
            }],
            created: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            last_modified: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            body: "# Test Unit\n\nThis is a test.\n".to_string(),
        }
    }

    #[test]
    fn test_write_and_read_unit() {
        let tmp = TempDir::new().unwrap();
        let file_path = tmp.path().join("domains/rust/test-unit.md");

        let original = sample_unit();
        write_unit(&file_path, &original).unwrap();

        let loaded = read_unit(&file_path).unwrap();
        assert_eq!(original.id, loaded.id);
        assert_eq!(original.kind, loaded.kind);
        assert_eq!(original.domain, loaded.domain);
        assert_eq!(original.tags, loaded.tags);
        assert_eq!(original.confidence, loaded.confidence);
        assert_eq!(original.body, loaded.body);
    }

    #[test]
    fn test_read_all_units() {
        let tmp = TempDir::new().unwrap();
        let brain_path = tmp.path().join("brain");
        crate::brain::init(&brain_path).unwrap();

        let unit1 = KnowledgeUnit {
            id: "unit-1".to_string(),
            domain: "rust".to_string(),
            ..sample_unit()
        };

        let unit2 = KnowledgeUnit {
            id: "unit-2".to_string(),
            domain: "python".to_string(),
            ..sample_unit()
        };

        write_unit(&brain_path.join("domains/rust/unit-1.md"), &unit1).unwrap();
        write_unit(&brain_path.join("domains/python/unit-2.md"), &unit2).unwrap();

        // Also write an invalid file to test graceful skipping
        std::fs::write(brain_path.join("domains/rust/bad.md"), "no frontmatter").unwrap();

        let units = read_all_units(&brain_path).unwrap();
        assert_eq!(units.len(), 2);
    }

    #[test]
    fn test_write_creates_parent_dirs() {
        let tmp = TempDir::new().unwrap();
        let file_path = tmp.path().join("deep/nested/dir/test.md");

        write_unit(&file_path, &sample_unit()).unwrap();
        assert!(file_path.exists());
    }
}
