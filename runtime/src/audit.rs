use sha2::{Digest, Sha256};
use serde_json::Value;
use crate::errors::OpenSkillError;

#[derive(Debug, Clone)]
pub enum ExecutionStatus {
    Success,
    Failed(String),
    Timeout,
    PermissionDenied,
}

#[derive(Debug, Clone)]
pub struct AuditRecord {
    pub skill_id: String,
    pub version: String,
    pub input_hash: String,
    pub output_hash: String,
    pub start_time_ms: u64,
    pub duration_ms: u64,
    pub permissions_used: Vec<String>,
    pub exit_status: ExecutionStatus,
    pub stdout: String,
    pub stderr: String,
}

pub trait AuditSink {
    fn record(&self, record: &AuditRecord);
}

pub struct NoopAuditSink;

impl AuditSink for NoopAuditSink {
    fn record(&self, _record: &AuditRecord) {}
}

pub fn hash_json(value: &Value) -> Result<String, OpenSkillError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|e| OpenSkillError::InvalidManifest(format!("Failed to serialize value for hashing: {}", e)))?;
    Ok(hash_bytes(&bytes))
}

/// Hash JSON value, returning empty string on error (for backwards compatibility)
pub fn hash_json_or_default(value: &Value) -> String {
    hash_json(value).unwrap_or_default()
}

/// Hash JSON value, returning the hash directly (for simple use cases)
pub fn hash_json_value(value: &Value) -> String {
    hash_json_or_default(value)
}

pub fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}
