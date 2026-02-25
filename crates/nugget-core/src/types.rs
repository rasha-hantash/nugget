// ── Types ──

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KnowledgeUnit {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: KnowledgeType,
    pub domain: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default = "default_confidence")]
    pub confidence: f64,
    #[serde(default)]
    pub source: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related: Vec<Relation>,
    pub created: NaiveDate,
    pub last_modified: NaiveDate,

    /// Body is NOT part of frontmatter — handled separately by parse/serialize
    #[serde(skip)]
    pub body: String,
}

fn default_confidence() -> f64 {
    0.8
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeType {
    Pattern,
    Concept,
    Decision,
    Bug,
    Belief,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Relation {
    pub id: String,
    pub relation: RelationType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RelationType {
    Uses,
    Implements,
    RequiresUnderstandingOf,
    InformedBy,
    OftenCombinedWith,
}
