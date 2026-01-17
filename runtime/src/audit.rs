use sha2::{Digest, Sha256};
use serde_json::Value;

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

pub fn hash_json(value: &Value) -> String {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    hash_bytes(&bytes)
}

pub fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}
