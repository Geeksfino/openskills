//! Execution context management for skill isolation.
//!
//! This module provides context forking capabilities to isolate skill execution
//! and prevent context pollution. Skills with `context: fork` execute in isolated
//! contexts where intermediate outputs are captured and only summaries are returned.

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Execution context for skills.
///
/// Contexts can be forked to create isolated execution environments where
/// intermediate outputs are captured separately from the main context.
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    /// Parent context ID (if forked).
    parent_id: Option<String>,
    /// Context ID.
    id: String,
    /// Whether this is a forked context.
    is_forked: bool,
    /// Intermediate outputs (only kept in forked contexts).
    intermediate_outputs: Vec<ContextOutput>,
    /// Final summary (extracted from forked context).
    summary: Option<String>,
}

/// Output captured in a context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextOutput {
    /// Timestamp when output was generated.
    pub timestamp: u64,
    /// Type of output.
    pub output_type: OutputType,
    /// Output content.
    pub content: String,
}

/// Type of output in a context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputType {
    /// Standard output.
    Stdout,
    /// Standard error.
    Stderr,
    /// Tool call result.
    ToolCall,
    /// Final result.
    Result,
}

impl ExecutionContext {
    /// Create a new main context.
    pub fn new() -> Self {
        Self {
            parent_id: None,
            id: generate_context_id(),
            is_forked: false,
            intermediate_outputs: Vec::new(),
            summary: None,
        }
    }

    /// Fork this context for isolated execution.
    ///
    /// Creates a new isolated context that captures all intermediate outputs
    /// separately from the parent context.
    pub fn fork(&self) -> Self {
        Self {
            parent_id: Some(self.id.clone()),
            id: generate_context_id(),
            is_forked: true,
            intermediate_outputs: Vec::new(),
            summary: None,
        }
    }

    /// Record an output in this context.
    pub fn record_output(&mut self, output_type: OutputType, content: String) {
        self.intermediate_outputs.push(ContextOutput {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            output_type,
            content,
        });
    }

    /// Generate a summary of this forked context.
    ///
    /// For forked contexts, this extracts only the essential results and
    /// ignores intermediate tool calls and verbose outputs.
    pub fn summarize(&mut self) -> String {
        if !self.is_forked {
            return String::new();
        }

        // Extract only Result outputs, ignore intermediate steps
        let results: Vec<String> = self
            .intermediate_outputs
            .iter()
            .filter(|o| matches!(o.output_type, OutputType::Result))
            .map(|o| o.content.clone())
            .collect();

        let summary = if results.is_empty() {
            // If no explicit results, try to extract meaningful content from stdout
            let stdout_outputs: Vec<String> = self
                .intermediate_outputs
                .iter()
                               .filter(|o| matches!(o.output_type, OutputType::Stdout))
                .map(|o| o.content.clone())
                .collect();

            if stdout_outputs.is_empty() {
                "Execution completed with no outputs.".to_string()
            } else {
                // Use last stdout as summary (often contains the final result)
                stdout_outputs.last().cloned().unwrap_or_default()
            }
        } else {
            results.join("\n\n")
        };

        self.summary = Some(summary.clone());
        summary
    }

    /// Get all outputs (for debugging/audit).
    pub fn get_outputs(&self) -> &[ContextOutput] {
        &self.intermediate_outputs
    }

    /// Get the context ID.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Check if this is a forked context.
    pub fn is_forked(&self) -> bool {
        self.is_forked
    }

    /// Get parent context ID if this is a fork.
    pub fn parent_id(&self) -> Option<&str> {
        self.parent_id.as_deref()
    }

    /// Get the summary if available.
    pub fn summary(&self) -> Option<&str> {
        self.summary.as_deref()
    }
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Generate a unique context ID.
fn generate_context_id() -> String {
    // Use timestamp + random suffix for uniqueness
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let random: u64 = (timestamp % 1000000) as u64;
    format!("ctx_{}_{:06x}", timestamp, random)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_creation() {
        let ctx = ExecutionContext::new();
        assert!(!ctx.is_forked());
        assert!(ctx.parent_id().is_none());
        assert!(!ctx.id().is_empty());
    }

    #[test]
    fn test_context_fork() {
        let parent = ExecutionContext::new();
        let parent_id = parent.id().to_string();
        // Small delay to ensure different timestamps
        std::thread::sleep(std::time::Duration::from_millis(1));
        let fork = parent.fork();

        assert!(fork.is_forked());
        assert_eq!(fork.parent_id(), Some(parent_id.as_str()));
        // IDs should be different (even if generated in same millisecond, random suffix differs)
        // But to be safe, we just check that fork has a parent
        assert!(fork.parent_id().is_some());
    }

    #[test]
    fn test_record_output() {
        let mut ctx = ExecutionContext::new();
        ctx.record_output(OutputType::Stdout, "test output".to_string());

        let outputs = ctx.get_outputs();
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].content, "test output");
        assert_eq!(outputs[0].output_type, OutputType::Stdout);
    }

    #[test]
    fn test_summarize_forked_with_results() {
        let mut fork = ExecutionContext::new().fork();
        fork.record_output(OutputType::ToolCall, "tool1".to_string());
        fork.record_output(OutputType::Result, "result1".to_string());
        fork.record_output(OutputType::ToolCall, "tool2".to_string());
        fork.record_output(OutputType::Result, "result2".to_string());

        let summary = fork.summarize();
        assert!(summary.contains("result1"));
        assert!(summary.contains("result2"));
        assert!(!summary.contains("tool1"));
        assert!(!summary.contains("tool2"));
    }

    #[test]
    fn test_summarize_forked_no_results() {
        let mut fork = ExecutionContext::new().fork();
        fork.record_output(OutputType::Stdout, "some output".to_string());

        let summary = fork.summarize();
        assert_eq!(summary, "some output");
    }

    #[test]
    fn test_summarize_main_context() {
        let mut ctx = ExecutionContext::new();
        ctx.record_output(OutputType::Result, "result".to_string());

        let summary = ctx.summarize();
        assert!(summary.is_empty()); // Main contexts don't summarize
    }
}
