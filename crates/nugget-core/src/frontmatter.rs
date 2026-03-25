// ── Frontmatter Parsing and Serialization ──
//
// Strategy: direct string slicing for perfect round-trip fidelity.
// - Find the opening `---\n`, find the closing `---\n`, extract YAML between them.
// - Everything after the closing `---\n` is the body (trimmed).
// - Serialization: `---\n{yaml}---\n\n{body}\n`

use crate::error::{NuggetError, Result};
use crate::types::KnowledgeUnit;

// ── Public API ──

/// Parse a markdown string with YAML frontmatter into a `KnowledgeUnit`.
pub fn parse(input: &str, path: &str) -> Result<KnowledgeUnit> {
    let (yaml, body) = split_frontmatter(input, path)?;

    let mut unit: KnowledgeUnit =
        serde_yaml_ng::from_str(yaml).map_err(|e| NuggetError::InvalidFrontmatter {
            path: path.to_string(),
            reason: e.to_string(),
        })?;

    unit.body = body.to_string();
    Ok(unit)
}

/// Serialize a `KnowledgeUnit` into a markdown string with YAML frontmatter.
pub fn serialize(unit: &KnowledgeUnit) -> Result<String> {
    let yaml = serde_yaml_ng::to_string(unit)?;

    let mut output = String::new();
    output.push_str("---\n");
    output.push_str(&yaml);
    output.push_str("---\n");

    if !unit.body.is_empty() {
        output.push('\n');
        output.push_str(&unit.body);
        if !unit.body.ends_with('\n') {
            output.push('\n');
        }
    }

    Ok(output)
}

// ── Helpers ──

/// Split a markdown string into frontmatter YAML and body.
/// Returns (yaml_content, body_content).
fn split_frontmatter<'a>(input: &'a str, path: &str) -> Result<(&'a str, &'a str)> {
    let trimmed = input.trim_start();

    if !trimmed.starts_with("---") {
        return Err(NuggetError::MissingFrontmatter {
            path: path.to_string(),
        });
    }

    // Skip the opening `---` and any trailing characters on that line
    let after_opening = match trimmed.strip_prefix("---") {
        Some(rest) => match rest.find('\n') {
            Some(pos) => &rest[pos + 1..],
            None => {
                return Err(NuggetError::MissingFrontmatter {
                    path: path.to_string(),
                })
            }
        },
        None => {
            return Err(NuggetError::MissingFrontmatter {
                path: path.to_string(),
            })
        }
    };

    // Find the closing `---`
    let closing_pos = find_closing_fence(after_opening).ok_or(NuggetError::MissingFrontmatter {
        path: path.to_string(),
    })?;

    let yaml = &after_opening[..closing_pos];
    let after_closing = &after_opening[closing_pos..];

    // Skip the closing `---` line
    let body = match after_closing.find('\n') {
        Some(pos) => after_closing[pos + 1..].trim_start_matches('\n'),
        None => "",
    };

    Ok((yaml, body))
}

/// Find the byte offset of the closing `---` fence in the content after the opening fence.
/// The closing fence must be at the start of a line.
fn find_closing_fence(content: &str) -> Option<usize> {
    let mut offset = 0;
    for line in content.lines() {
        if line.starts_with("---") {
            return Some(offset);
        }
        // +1 for the newline character
        offset += line.len() + 1;
    }
    None
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{KnowledgeType, Relation, RelationType};
    use chrono::NaiveDate;

    fn sample_markdown() -> &'static str {
        r#"---
id: pattern-error-handling-rust
type: pattern
domain: rust
tags:
  - error-handling
  - result-type
confidence: 0.9
source: direct-experience
related:
  - id: concept-option-type
    relation: often_combined_with
created: 2026-02-24
last_modified: 2026-02-24
---

# Error Handling in Rust

Use `Result<T, E>` for all fallible operations.

## Key Points

- Never use `unwrap()` in production code
- Use `thiserror` for library error types
"#
    }

    fn sample_unit() -> KnowledgeUnit {
        KnowledgeUnit {
            id: "pattern-error-handling-rust".to_string(),
            kind: KnowledgeType::Pattern,
            domain: "rust".to_string(),
            tags: vec!["error-handling".to_string(), "result-type".to_string()],
            confidence: 0.9,
            source: "direct-experience".to_string(),
            related: vec![Relation {
                id: "concept-option-type".to_string(),
                relation: RelationType::OftenCombinedWith,
            }],
            created: NaiveDate::from_ymd_opt(2026, 2, 24).unwrap(),
            last_modified: NaiveDate::from_ymd_opt(2026, 2, 24).unwrap(),
            body: "# Error Handling in Rust\n\nUse `Result<T, E>` for all fallible operations.\n\n## Key Points\n\n- Never use `unwrap()` in production code\n- Use `thiserror` for library error types\n".to_string(),
        }
    }

    #[test]
    fn test_parse_well_formed() {
        let unit = parse(sample_markdown(), "test.md").unwrap();
        insta::assert_yaml_snapshot!(unit, {
            ".body" => "[body]"
        });
        assert_eq!(unit.id, "pattern-error-handling-rust");
        assert_eq!(unit.kind, KnowledgeType::Pattern);
        assert_eq!(unit.domain, "rust");
        assert_eq!(unit.tags, vec!["error-handling", "result-type"]);
        assert_eq!(unit.confidence, 0.9);
        assert!(unit.body.contains("# Error Handling in Rust"));
    }

    #[test]
    fn test_serialize() {
        let unit = sample_unit();
        let output = serialize(&unit).unwrap();
        insta::assert_snapshot!(output);
    }

    #[test]
    fn test_round_trip() {
        let original = parse(sample_markdown(), "test.md").unwrap();
        let serialized = serialize(&original).unwrap();
        let reparsed = parse(&serialized, "test.md").unwrap();

        assert_eq!(original.id, reparsed.id);
        assert_eq!(original.kind, reparsed.kind);
        assert_eq!(original.domain, reparsed.domain);
        assert_eq!(original.tags, reparsed.tags);
        assert_eq!(original.confidence, reparsed.confidence);
        assert_eq!(original.source, reparsed.source);
        assert_eq!(original.related, reparsed.related);
        assert_eq!(original.created, reparsed.created);
        assert_eq!(original.last_modified, reparsed.last_modified);
        assert_eq!(original.body, reparsed.body);
    }

    #[test]
    fn test_missing_frontmatter() {
        let input = "# Just a heading\n\nNo frontmatter here.";
        let result = parse(input, "no-fm.md");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("missing frontmatter"));
    }

    #[test]
    fn test_body_with_horizontal_rules() {
        let input = r#"---
id: test-hr
type: concept
domain: testing
created: 2026-01-01
last_modified: 2026-01-01
---

Some text before.

---

Some text after the horizontal rule.

---

More text after another rule.
"#;
        let unit = parse(input, "hr-test.md").unwrap();
        assert_eq!(unit.id, "test-hr");
        assert!(unit.body.contains("---"));
        assert!(unit.body.contains("Some text before."));
        assert!(unit.body.contains("Some text after the horizontal rule."));
        assert!(unit.body.contains("More text after another rule."));
    }

    #[test]
    fn test_empty_body() {
        let input = "---\nid: empty-body\ntype: bug\ndomain: testing\ncreated: 2026-01-01\nlast_modified: 2026-01-01\n---\n";
        let unit = parse(input, "empty.md").unwrap();
        assert_eq!(unit.id, "empty-body");
        assert!(unit.body.is_empty());
    }

    #[test]
    fn test_default_tags_and_relations() {
        let input = "---\nid: minimal\ntype: belief\ndomain: testing\ncreated: 2026-01-01\nlast_modified: 2026-01-01\n---\n\nMinimal body.\n";
        let unit = parse(input, "minimal.md").unwrap();
        assert!(unit.tags.is_empty());
        assert!(unit.related.is_empty());
        assert_eq!(unit.confidence, 0.8); // default
    }
}
