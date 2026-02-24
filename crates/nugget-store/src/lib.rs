use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use nugget_core::KnowledgeUnit;
use serde::{Deserialize, Serialize};

// ── Types ──

/// Metadata for a brain domain stored in `domain.yaml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainMeta {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Summary of a knowledge unit for listing (without full body).
#[derive(Debug, Clone, Serialize)]
pub struct KnowledgeSummary {
    pub id: String,
    pub knowledge_type: String,
    pub tags: Vec<String>,
    pub preview: String,
    pub relative_path: String,
}

/// Handle to a brain directory on disk.
#[derive(Debug, Clone)]
pub struct BrainStore {
    pub root: PathBuf,
}

// ── Helpers ──

/// Split a markdown file into YAML frontmatter and body.
fn split_frontmatter(content: &str) -> (Option<&str>, &str) {
    if !content.starts_with("---\n") {
        return (None, content);
    }
    if let Some(end) = content[4..].find("\n---") {
        let yaml = &content[4..4 + end];
        let body_start = 4 + end + 4; // skip past "\n---"
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

/// Render a knowledge unit as a markdown file with YAML frontmatter.
fn render_knowledge_file(unit: &KnowledgeUnit) -> Result<String> {
    let frontmatter = serde_yaml::to_string(unit).context("serializing frontmatter")?;
    Ok(format!("---\n{}---\n\n{}\n", frontmatter, unit.body))
}

// ── Public API ──

impl BrainStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Initialize a new brain directory structure.
    pub fn init(&self) -> Result<()> {
        let dirs = [
            self.root.clone(),
            self.root.join("inbox"),
        ];
        for dir in &dirs {
            fs::create_dir_all(dir)
                .with_context(|| format!("creating directory: {}", dir.display()))?;
        }

        let brain_yaml = self.root.join("brain.yaml");
        if !brain_yaml.exists() {
            fs::write(&brain_yaml, "# Nugget brain configuration\n")
                .context("writing brain.yaml")?;
        }

        Ok(())
    }

    /// Add a new domain folder with a `domain.yaml` metadata file.
    pub fn add_domain(&self, name: &str, description: Option<&str>) -> Result<PathBuf> {
        let domain_path = self.root.join(name);
        fs::create_dir_all(&domain_path)
            .with_context(|| format!("creating domain directory: {}", domain_path.display()))?;

        let meta = DomainMeta {
            name: name.to_string(),
            description: description.map(String::from),
        };
        let meta_yaml = serde_yaml::to_string(&meta).context("serializing domain meta")?;
        fs::write(domain_path.join("domain.yaml"), meta_yaml)
            .context("writing domain.yaml")?;

        Ok(domain_path)
    }

    /// List all domain names by scanning for directories containing `domain.yaml`.
    pub fn list_domains(&self) -> Result<Vec<String>> {
        let mut domains = Vec::new();
        let entries = fs::read_dir(&self.root).context("reading brain directory")?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() && path.join("domain.yaml").exists() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    domains.push(name.to_string());
                }
            }
        }
        domains.sort();
        Ok(domains)
    }

    /// Return the path to the inbox directory.
    pub fn inbox_path(&self) -> PathBuf {
        self.root.join("inbox")
    }

    /// Write a knowledge unit to the appropriate domain path.
    pub fn write_knowledge(&self, unit: &KnowledgeUnit, filename: &str) -> Result<PathBuf> {
        let domain_dir = self.root.join(&unit.domain.0);
        fs::create_dir_all(&domain_dir)
            .with_context(|| format!("creating domain dir: {}", domain_dir.display()))?;

        let file_path = domain_dir.join(filename);
        let content = render_knowledge_file(unit)?;
        fs::write(&file_path, content)
            .with_context(|| format!("writing knowledge file: {}", file_path.display()))?;

        Ok(file_path)
    }

    /// Read a knowledge unit from a markdown file with frontmatter.
    pub fn read_knowledge(&self, path: &Path) -> Result<KnowledgeUnit> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("reading file: {}", path.display()))?;

        let (frontmatter, body) = split_frontmatter(&content);
        let yaml = frontmatter.context("no frontmatter found in file")?;

        let mut unit: KnowledgeUnit =
            serde_yaml::from_str(yaml).context("parsing frontmatter")?;
        unit.body = body.to_string();

        Ok(unit)
    }

    /// List knowledge summaries for a domain by walking its directory.
    pub fn list_knowledge(&self, domain: &str) -> Result<Vec<KnowledgeSummary>> {
        let domain_dir = self.root.join(domain);
        if !domain_dir.exists() {
            return Ok(Vec::new());
        }

        let mut summaries = Vec::new();
        for entry in walkdir::WalkDir::new(&domain_dir)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }

            let content = match fs::read_to_string(path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let (frontmatter, body) = split_frontmatter(&content);
            let yaml = match frontmatter {
                Some(y) => y,
                None => continue,
            };

            let unit: KnowledgeUnit = match serde_yaml::from_str(yaml) {
                Ok(u) => u,
                Err(_) => continue,
            };

            let first_line = body.lines().next().unwrap_or("");
            let preview = if first_line.len() > 100 {
                format!("{}...", &first_line[..97])
            } else {
                first_line.to_string()
            };

            let relative_path = path
                .strip_prefix(&self.root)
                .unwrap_or(path)
                .to_string_lossy()
                .to_string();

            summaries.push(KnowledgeSummary {
                id: unit.id,
                knowledge_type: unit.knowledge_type.to_string(),
                tags: unit.tags.iter().map(|t| t.0.clone()).collect(),
                preview,
                relative_path,
            });
        }

        Ok(summaries)
    }

    /// Count the number of `.md` knowledge files in a domain directory.
    pub fn count_knowledge(&self, domain: &str) -> usize {
        let domain_dir = self.root.join(domain);
        if !domain_dir.exists() {
            return 0;
        }

        walkdir::WalkDir::new(&domain_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|ext| ext.to_str()) == Some("md"))
            .count()
    }

    /// Read the `domain.yaml` metadata for a domain.
    pub fn read_domain_meta(&self, domain: &str) -> Result<DomainMeta> {
        let meta_path = self.root.join(domain).join("domain.yaml");
        let content = fs::read_to_string(&meta_path)
            .with_context(|| format!("reading domain.yaml: {}", meta_path.display()))?;
        let meta: DomainMeta =
            serde_yaml::from_str(&content).context("parsing domain.yaml")?;
        Ok(meta)
    }

    /// Check if the brain directory has been initialized.
    pub fn is_initialized(&self) -> bool {
        self.root.join("brain.yaml").exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_frontmatter() {
        let content = "---\nid: abc\ntype: concept\n---\n\nHello world\n";
        let (fm, body) = split_frontmatter(content);
        assert_eq!(fm, Some("id: abc\ntype: concept"));
        assert_eq!(body, "Hello world\n");
    }

    #[test]
    fn test_split_frontmatter_no_frontmatter() {
        let content = "Just some text";
        let (fm, body) = split_frontmatter(content);
        assert!(fm.is_none());
        assert_eq!(body, "Just some text");
    }

    #[test]
    fn test_init_and_add_domain() {
        let tmp = tempfile::tempdir().unwrap();
        let store = BrainStore::new(tmp.path().join("brain"));
        store.init().unwrap();

        assert!(store.is_initialized());
        assert!(store.inbox_path().exists());

        store.add_domain("coding/rust", Some("Rust knowledge")).unwrap();
        // "coding/rust" creates a nested dir, not a direct child with domain.yaml
        // at root level. Check the nested path directly.
        assert!(store.root.join("coding/rust/domain.yaml").exists());
    }

    #[test]
    fn test_list_knowledge() {
        use nugget_core::{Confidence, Domain, KnowledgeType, Tag};

        let tmp = tempfile::tempdir().unwrap();
        let store = BrainStore::new(tmp.path().join("brain"));
        store.init().unwrap();
        store.add_domain("coding", Some("coding knowledge")).unwrap();

        let unit1 = KnowledgeUnit {
            id: "aaa-111".to_string(),
            knowledge_type: KnowledgeType::Concept,
            domain: Domain("coding".to_string()),
            tags: vec![Tag("rust".to_string())],
            confidence: Confidence::new(1.0),
            source: None,
            related: Vec::new(),
            body: "Ownership rules in Rust".to_string(),
        };
        let unit2 = KnowledgeUnit {
            id: "bbb-222".to_string(),
            knowledge_type: KnowledgeType::Pattern,
            domain: Domain("coding".to_string()),
            tags: vec![],
            confidence: Confidence::new(1.0),
            source: None,
            related: Vec::new(),
            body: "Builder pattern for complex structs".to_string(),
        };

        store.write_knowledge(&unit1, "ownership.md").unwrap();
        store.write_knowledge(&unit2, "builder.md").unwrap();

        let summaries = store.list_knowledge("coding").unwrap();
        assert_eq!(summaries.len(), 2);

        let ids: Vec<&str> = summaries.iter().map(|s| s.id.as_str()).collect();
        assert!(ids.contains(&"aaa-111"));
        assert!(ids.contains(&"bbb-222"));

        let rust_summary = summaries.iter().find(|s| s.id == "aaa-111").unwrap();
        assert_eq!(rust_summary.knowledge_type, "concept");
        assert_eq!(rust_summary.tags, vec!["rust"]);
        assert!(rust_summary.preview.contains("Ownership"));
    }

    #[test]
    fn test_count_knowledge() {
        use nugget_core::{Domain, KnowledgeType};

        let tmp = tempfile::tempdir().unwrap();
        let store = BrainStore::new(tmp.path().join("brain"));
        store.init().unwrap();
        store.add_domain("coding", None).unwrap();

        assert_eq!(store.count_knowledge("coding"), 0);
        assert_eq!(store.count_knowledge("nonexistent"), 0);

        let unit = KnowledgeUnit::new(
            KnowledgeType::Concept,
            Domain("coding".to_string()),
            "Test body".to_string(),
        );
        store.write_knowledge(&unit, "test.md").unwrap();

        assert_eq!(store.count_knowledge("coding"), 1);
    }

    #[test]
    fn test_read_domain_meta() {
        let tmp = tempfile::tempdir().unwrap();
        let store = BrainStore::new(tmp.path().join("brain"));
        store.init().unwrap();
        store.add_domain("coding", Some("All about code")).unwrap();

        let meta = store.read_domain_meta("coding").unwrap();
        assert_eq!(meta.name, "coding");
        assert_eq!(meta.description, Some("All about code".to_string()));
    }
}
