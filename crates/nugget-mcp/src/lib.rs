use anyhow::Result;
use nugget_inbox::Inbox;
use nugget_store::BrainStore;
use rmcp::model::*;
use rmcp::schemars::JsonSchema;
use rmcp::serde::{Deserialize, Serialize};
use rmcp::{tool, ServerHandler, ServiceExt};

// ── Types ──

/// Input for the capture_learnings tool.
#[derive(Debug, Deserialize, JsonSchema)]
struct CaptureLearningsInput {
    /// Summary of the AI conversation session
    summary: String,
    /// List of learnings/patterns discovered
    learnings: Vec<String>,
    /// List of decisions made
    decisions: Vec<String>,
    /// Optional context string (e.g., "rust programming session")
    context: Option<String>,
}

/// Input for the capture_url tool.
#[derive(Debug, Deserialize, JsonSchema)]
struct CaptureUrlInput {
    /// The URL being captured
    url: String,
    /// Title for the captured content
    title: String,
    /// Summary of the content
    summary: String,
    /// Optional tags to attach
    tags: Option<Vec<String>>,
    /// Optional domain to file under
    domain: Option<String>,
}

/// Input for the capture_text tool.
#[derive(Debug, Deserialize, JsonSchema)]
struct CaptureTextInput {
    /// The text content to capture
    text: String,
    /// Optional source attribution
    source: Option<String>,
    /// Optional domain to file under
    domain: Option<String>,
}

/// Input for the list_knowledge tool.
#[derive(Debug, Deserialize, JsonSchema)]
struct ListKnowledgeInput {
    /// The domain to list knowledge from (e.g., "coding", "devops")
    domain: String,
}

/// Input for the read_knowledge tool.
#[derive(Debug, Deserialize, JsonSchema)]
struct ReadKnowledgeInput {
    /// Relative path to the knowledge file within the brain (e.g., "coding/ownership.md")
    path: String,
}

/// A summary of a recent inbox item for status output.
#[derive(Debug, Serialize)]
struct RecentItem {
    id: String,
    #[serde(rename = "type")]
    knowledge_type: String,
    captured_at: String,
    preview: String,
}

/// The nugget MCP server.
#[derive(Clone)]
pub struct NuggetServer {
    store: BrainStore,
}

// ── Helpers ──

/// Truncate a string to the given max length, appending "..." if truncated.
fn truncate_preview(s: &str, max_len: usize) -> String {
    let first_line = s.lines().next().unwrap_or("");
    if first_line.len() > max_len {
        format!("{}...", &first_line[..max_len.saturating_sub(3)])
    } else {
        first_line.to_string()
    }
}

// ── Public API ──

#[tool(tool_box)]
impl NuggetServer {
    pub fn new(store: BrainStore) -> Self {
        Self { store }
    }

    #[tool(
        name = "capture_learnings",
        description = "Capture learnings and decisions from an AI conversation session"
    )]
    fn capture_learnings(
        &self,
        #[tool(aggr)] input: CaptureLearningsInput,
    ) -> Result<CallToolResult, rmcp::Error> {
        let paths = nugget_capture::capture_from_conversation(
            &self.store,
            &input.summary,
            &input.learnings,
            &input.decisions,
            input.context.as_deref(),
        )
        .map_err(|e| rmcp::Error::internal_error(format!("capture failed: {e}"), None))?;

        let result = serde_json::json!({ "captured": paths.len() });
        Ok(CallToolResult::success(vec![Content::text(
            result.to_string(),
        )]))
    }

    #[tool(
        name = "capture_url",
        description = "Capture a knowledge item from a URL"
    )]
    fn capture_url(
        &self,
        #[tool(aggr)] input: CaptureUrlInput,
    ) -> Result<CallToolResult, rmcp::Error> {
        let tags = input.tags.unwrap_or_default();
        let path = nugget_capture::capture_from_url(
            &self.store,
            &input.url,
            &input.title,
            &input.summary,
            &tags,
            input.domain.as_deref(),
        )
        .map_err(|e| rmcp::Error::internal_error(format!("capture failed: {e}"), None))?;

        // Read back the item to get its ID
        let inbox = Inbox::new(self.store.clone());
        let entries = inbox
            .list()
            .map_err(|e| rmcp::Error::internal_error(format!("list failed: {e}"), None))?;
        let item_id = entries
            .last()
            .map(|e| e.item.id.clone())
            .unwrap_or_default();

        let result = serde_json::json!({
            "item_id": item_id,
            "path": path.display().to_string(),
        });
        Ok(CallToolResult::success(vec![Content::text(
            result.to_string(),
        )]))
    }

    #[tool(
        name = "capture_text",
        description = "Capture a plain text knowledge item"
    )]
    fn capture_text(
        &self,
        #[tool(aggr)] input: CaptureTextInput,
    ) -> Result<CallToolResult, rmcp::Error> {
        let path = nugget_capture::capture_from_text(
            &self.store,
            &input.text,
            input.source.as_deref(),
            input.domain.as_deref(),
        )
        .map_err(|e| rmcp::Error::internal_error(format!("capture failed: {e}"), None))?;

        // Read back the item to get its ID
        let inbox = Inbox::new(self.store.clone());
        let entries = inbox
            .list()
            .map_err(|e| rmcp::Error::internal_error(format!("list failed: {e}"), None))?;
        let item_id = entries
            .last()
            .map(|e| e.item.id.clone())
            .unwrap_or_default();

        let result = serde_json::json!({
            "item_id": item_id,
            "path": path.display().to_string(),
        });
        Ok(CallToolResult::success(vec![Content::text(
            result.to_string(),
        )]))
    }

    #[tool(
        name = "inbox_status",
        description = "Show the current inbox status and recent items"
    )]
    fn inbox_status(&self) -> Result<CallToolResult, rmcp::Error> {
        let inbox = Inbox::new(self.store.clone());
        let entries = inbox
            .list()
            .map_err(|e| rmcp::Error::internal_error(format!("list failed: {e}"), None))?;

        let pending_count = entries.len();

        // Take last 10 items in reverse order (most recent first)
        let recent_items: Vec<RecentItem> = entries
            .iter()
            .rev()
            .take(10)
            .map(|entry| RecentItem {
                id: entry.item.id.clone(),
                knowledge_type: entry.item.knowledge_type.to_string(),
                captured_at: entry.item.captured_at.to_rfc3339(),
                preview: truncate_preview(&entry.item.body, 80),
            })
            .collect();

        let result = serde_json::json!({
            "pending_count": pending_count,
            "recent_items": recent_items,
        });
        Ok(CallToolResult::success(vec![Content::text(
            result.to_string(),
        )]))
    }

    #[tool(
        name = "get_brain_summary",
        description = "Get a quick overview of the brain: total domains, total knowledge units, and inbox count"
    )]
    fn get_brain_summary(&self) -> Result<CallToolResult, rmcp::Error> {
        let domains = self
            .store
            .list_domains()
            .map_err(|e| rmcp::Error::internal_error(format!("list domains failed: {e}"), None))?;

        let total_units: usize = domains.iter().map(|d| self.store.count_knowledge(d)).sum();

        let inbox = Inbox::new(self.store.clone());
        let inbox_count = inbox
            .list()
            .map_err(|e| rmcp::Error::internal_error(format!("list inbox failed: {e}"), None))?
            .len();

        let result = serde_json::json!({
            "total_domains": domains.len(),
            "domains": domains,
            "total_units": total_units,
            "inbox_count": inbox_count,
        });
        Ok(CallToolResult::success(vec![Content::text(
            result.to_string(),
        )]))
    }

    #[tool(
        name = "list_domains",
        description = "List all knowledge domains in the brain with item counts"
    )]
    fn list_domains(&self) -> Result<CallToolResult, rmcp::Error> {
        let domains = self
            .store
            .list_domains()
            .map_err(|e| rmcp::Error::internal_error(format!("list domains failed: {e}"), None))?;

        let domain_info: Vec<serde_json::Value> = domains
            .iter()
            .map(|name| {
                let count = self.store.count_knowledge(name);
                let description = self
                    .store
                    .read_domain_meta(name)
                    .ok()
                    .and_then(|m| m.description);
                serde_json::json!({
                    "name": name,
                    "item_count": count,
                    "description": description,
                })
            })
            .collect();

        let result = serde_json::json!({ "domains": domain_info });
        Ok(CallToolResult::success(vec![Content::text(
            result.to_string(),
        )]))
    }

    #[tool(
        name = "list_knowledge",
        description = "List knowledge summaries for a specific domain (id, type, tags, preview, path)"
    )]
    fn list_knowledge(
        &self,
        #[tool(aggr)] input: ListKnowledgeInput,
    ) -> Result<CallToolResult, rmcp::Error> {
        let summaries = self
            .store
            .list_knowledge(&input.domain)
            .map_err(|e| {
                rmcp::Error::internal_error(format!("list knowledge failed: {e}"), None)
            })?;

        let result = serde_json::json!({
            "domain": input.domain,
            "count": summaries.len(),
            "items": summaries,
        });
        Ok(CallToolResult::success(vec![Content::text(
            result.to_string(),
        )]))
    }

    #[tool(
        name = "read_knowledge",
        description = "Read the full content of a specific knowledge unit by its relative path"
    )]
    fn read_knowledge(
        &self,
        #[tool(aggr)] input: ReadKnowledgeInput,
    ) -> Result<CallToolResult, rmcp::Error> {
        // Validate path stays within the brain root (prevent path traversal)
        let requested = self.store.root.join(&input.path);
        let canonical = std::fs::canonicalize(&requested).map_err(|e| {
            rmcp::Error::internal_error(format!("invalid path '{}': {e}", input.path), None)
        })?;
        let root_canonical = std::fs::canonicalize(&self.store.root).map_err(|e| {
            rmcp::Error::internal_error(format!("cannot resolve brain root: {e}"), None)
        })?;
        if !canonical.starts_with(&root_canonical) {
            return Err(rmcp::Error::internal_error(
                "path traversal denied: path is outside the brain".to_string(),
                None,
            ));
        }

        let unit = self.store.read_knowledge(&canonical).map_err(|e| {
            rmcp::Error::internal_error(format!("read failed: {e}"), None)
        })?;

        let result = serde_json::json!({
            "id": unit.id,
            "type": unit.knowledge_type.to_string(),
            "domain": unit.domain.to_string(),
            "tags": unit.tags.iter().map(|t| t.to_string()).collect::<Vec<_>>(),
            "confidence": unit.confidence.0,
            "source": unit.source,
            "body": unit.body,
        });
        Ok(CallToolResult::success(vec![Content::text(
            result.to_string(),
        )]))
    }
}

#[tool(tool_box)]
impl ServerHandler for NuggetServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Nugget personal knowledge brain \u{2014} read and search your knowledge base, \
                 and capture new learnings, URLs, and text into your inbox for review. \
                 Use get_brain_summary to see what's in the brain, list_domains and \
                 list_knowledge to browse, and read_knowledge to read specific items. \
                 Use capture_learnings, capture_url, and capture_text to add new knowledge."
                    .into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation::from_build_env(),
            ..Default::default()
        }
    }
}

/// Start the nugget MCP server on stdio transport.
pub async fn run_mcp_server(store: BrainStore) -> Result<()> {
    eprintln!("nugget-mcp: serving brain at {}", store.root.display());
    let server = NuggetServer::new(store);
    let service = server
        .serve(rmcp::transport::io::stdio())
        .await
        .map_err(|e| anyhow::anyhow!("failed to start MCP server: {e}"))?;

    service.waiting().await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_construction() {
        let tmp = tempfile::tempdir().expect("failed to create temp dir");
        let store = BrainStore::new(tmp.path().join("brain"));
        store.init().expect("failed to init store");
        let server = NuggetServer::new(store);
        let info = server.get_info();
        assert!(info.instructions.is_some());
    }

    #[test]
    fn test_truncate_preview() {
        assert_eq!(truncate_preview("short text", 80), "short text");
        let long = "a".repeat(100);
        let result = truncate_preview(&long, 80);
        assert!(result.len() <= 80);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_truncate_preview_multiline() {
        let text = "first line\nsecond line\nthird line";
        assert_eq!(truncate_preview(text, 80), "first line");
    }
}
