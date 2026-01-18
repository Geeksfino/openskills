//! Skill execution session helpers for context fork behavior.
//!
//! This module provides a session wrapper that helps agents record tool calls
//! and intermediate outputs in forked contexts. It enables proper `context: fork`
//! behavior when skills are primarily instructional and tool calls are executed
//! by the agent rather than the runtime.

use crate::context::{ExecutionContext, OutputType};
use crate::LoadedSkill;
use serde_json::Value;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// Execution session for instruction-based skills.
#[derive(Debug, Clone)]
pub struct SkillExecutionSession {
    skill: LoadedSkill,
    is_forked: bool,
    input: Value,
    start_instant: Instant,
    start_epoch_ms: u64,
    permissions_used: Vec<String>,
    context: Option<ExecutionContext>,
}

impl SkillExecutionSession {
    pub fn new(
        skill: LoadedSkill,
        is_forked: bool,
        input: Value,
        context: Option<ExecutionContext>,
    ) -> Self {
        let start_epoch_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        Self {
            skill,
            is_forked,
            input,
            start_instant: Instant::now(),
            start_epoch_ms,
            permissions_used: Vec::new(),
            context,
        }
    }

    pub fn skill(&self) -> &LoadedSkill {
        &self.skill
    }

    pub fn input(&self) -> &Value {
        &self.input
    }

    pub fn is_forked(&self) -> bool {
        self.is_forked
    }

    pub fn context_id(&self) -> Option<&str> {
        self.context.as_ref().map(|ctx| ctx.id())
    }

    pub fn start_epoch_ms(&self) -> u64 {
        self.start_epoch_ms
    }

    pub fn elapsed_ms(&self) -> u64 {
        self.start_instant.elapsed().as_millis() as u64
    }

    pub fn permissions_used(&self) -> &[String] {
        &self.permissions_used
    }

    pub fn add_permission_used(&mut self, tool: &str) {
        if !self.permissions_used.iter().any(|t| t == tool) {
            self.permissions_used.push(tool.to_string());
        }
    }

    pub fn context(&self) -> Option<&ExecutionContext> {
        self.context.as_ref()
    }

    pub fn context_mut(&mut self) -> Option<&mut ExecutionContext> {
        self.context.as_mut()
    }

    pub fn record_tool_call(&mut self, tool: &str, output: &Value) {
        self.add_permission_used(tool);
        let Some(ctx) = self.context.as_mut() else {
            return;
        };
        let output_str = serde_json::to_string(output).unwrap_or_else(|_| "\"{}\"".to_string());
        let content = format!("{}: {}", tool, output_str);
        ctx.record_output(OutputType::ToolCall, content);
    }

    pub fn record_stdout_if_present(&mut self, stdout: &str) {
        if stdout.is_empty() {
            return;
        }
        if let Some(ctx) = self.context.as_mut() {
            ctx.record_output(OutputType::Stdout, stdout.to_string());
        }
    }

    pub fn record_stderr_if_present(&mut self, stderr: &str) {
        if stderr.is_empty() {
            return;
        }
        if let Some(ctx) = self.context.as_mut() {
            ctx.record_output(OutputType::Stderr, stderr.to_string());
        }
    }

    pub fn record_result(&mut self, result: &Value) {
        if let Some(ctx) = self.context.as_mut() {
            let result_str = if let Some(val) = result.as_str() {
                val.to_string()
            } else {
                serde_json::to_string(result).unwrap_or_else(|_| "{}".to_string())
            };
            ctx.record_output(OutputType::Result, result_str);
        }
    }

    pub fn summarize_fork(&mut self) -> String {
        self.context
            .as_mut()
            .map(|ctx| ctx.summarize())
            .unwrap_or_else(String::new)
    }
}
