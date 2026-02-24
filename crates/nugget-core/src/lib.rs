use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Types ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum KnowledgeType {
    Concept,
    Pattern,
    Decision,
    Bug,
    Belief,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum CaptureMethod {
    ClipboardUrl,
    ClipboardText,
    AiSession,
    WebCapture,
    Manual,
    Import,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Domain(pub String);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Tag(pub String);

/// Confidence score from 0.0 to 1.0.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Confidence(pub f64);

impl Confidence {
    pub fn new(value: f64) -> Self {
        Self(value.clamp(0.0, 1.0))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Relation {
    pub target_id: String,
    pub kind: String,
}

/// A knowledge unit — the atomic piece of knowledge in the brain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeUnit {
    pub id: String,
    #[serde(rename = "type")]
    pub knowledge_type: KnowledgeType,
    pub domain: Domain,
    pub tags: Vec<Tag>,
    pub confidence: Confidence,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related: Vec<Relation>,
    #[serde(skip)]
    pub body: String,
}

/// An inbox item — a proposed knowledge unit awaiting review.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboxItem {
    pub id: String,
    #[serde(rename = "type")]
    pub knowledge_type: KnowledgeType,
    pub tags: Vec<Tag>,
    pub confidence: Confidence,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related: Vec<Relation>,
    pub suggested_domain: Domain,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggested_path: Option<String>,
    pub captured_at: DateTime<Utc>,
    pub capture_method: CaptureMethod,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capture_context: Option<String>,
    #[serde(skip)]
    pub body: String,
}

// ── Helpers ──

impl KnowledgeUnit {
    pub fn new(knowledge_type: KnowledgeType, domain: Domain, body: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            knowledge_type,
            domain,
            tags: Vec::new(),
            confidence: Confidence::new(1.0),
            source: None,
            related: Vec::new(),
            body,
        }
    }
}

impl InboxItem {
    pub fn new(
        knowledge_type: KnowledgeType,
        suggested_domain: Domain,
        capture_method: CaptureMethod,
        body: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            knowledge_type,
            tags: Vec::new(),
            confidence: Confidence::new(0.7),
            source: None,
            related: Vec::new(),
            suggested_domain,
            suggested_path: None,
            captured_at: Utc::now(),
            capture_method,
            capture_context: None,
            body,
        }
    }

    /// Convert an accepted inbox item into a knowledge unit for the brain.
    pub fn into_knowledge_unit(self, domain: Domain) -> KnowledgeUnit {
        KnowledgeUnit {
            id: self.id,
            knowledge_type: self.knowledge_type,
            domain,
            tags: self.tags,
            confidence: self.confidence,
            source: self.source,
            related: self.related,
            body: self.body,
        }
    }
}

impl std::fmt::Display for Domain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::fmt::Display for Tag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::fmt::Display for KnowledgeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KnowledgeType::Concept => write!(f, "concept"),
            KnowledgeType::Pattern => write!(f, "pattern"),
            KnowledgeType::Decision => write!(f, "decision"),
            KnowledgeType::Bug => write!(f, "bug"),
            KnowledgeType::Belief => write!(f, "belief"),
        }
    }
}

impl std::fmt::Display for CaptureMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CaptureMethod::ClipboardUrl => write!(f, "clipboard-url"),
            CaptureMethod::ClipboardText => write!(f, "clipboard-text"),
            CaptureMethod::AiSession => write!(f, "ai-session"),
            CaptureMethod::WebCapture => write!(f, "web-capture"),
            CaptureMethod::Manual => write!(f, "manual"),
            CaptureMethod::Import => write!(f, "import"),
        }
    }
}
