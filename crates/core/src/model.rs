use crate::Framework;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SCHEMA: u32 = 1;
fn one() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Check {
    pub argv: Vec<String>,
    #[serde(default = "dot")]
    pub cwd: String,
}
fn dot() -> String {
    ".".into()
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    pub kind: String,
    /// OpenSpec change ID or repository-relative Spec Kit feature path.
    pub selector: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Phase {
    pub id: String,
    pub label: String,
    pub title: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
    pub source: Source,
    /// Optional stable coordination metadata; it is not a release/version field.
    #[serde(default)]
    pub workstream: Option<String>,
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub external_ids: Vec<String>,
    #[serde(default)]
    pub verification: Vec<Check>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Milestone {
    #[serde(default = "one")]
    pub schema_version: u32,
    pub id: String,
    pub goal: String,
    pub framework: Framework,
    #[serde(default = "one")]
    pub revision: u32,
    pub phases: Vec<Phase>,
    #[serde(default)]
    pub verification: Vec<Check>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Range {
    pub from: Option<String>,
    pub to: Option<String>,
    pub only: Option<String>,
}
impl Range {
    pub fn bounded(&self) -> bool {
        self.from.is_some() || self.to.is_some() || self.only.is_some()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceTask {
    pub id: String,
    pub description: String,
    pub done: bool,
    pub source_path: String,
    pub source_hash: String,
    pub text_hash: String,
    pub line: usize,
    pub checkbox_byte: usize,
    pub phase: String,
    pub story: Option<String>,
    pub parallel: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextFile {
    pub path: String,
    pub hash: String,
    pub content: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeAction {
    pub kind: String,
    pub artifact: String,
    pub instruction: String,
    #[serde(default)]
    pub outputs: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub schema_version: u32,
    pub framework: Framework,
    pub selector: String,
    pub source_dir: String,
    pub tracking_file: Option<String>,
    pub planning_ready: bool,
    pub next_action: NativeAction,
    pub tasks: Vec<SourceTask>,
    pub context_files: Vec<ContextFile>,
    pub diagnostics: Vec<String>,
    pub source_hash: String,
    pub metadata: serde_json::Value,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub description: String,
    pub source_ids: Vec<String>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub reads: Vec<String>,
    #[serde(default)]
    pub writes: Vec<String>,
    #[serde(default)]
    pub verification: Vec<Check>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    #[serde(default = "one")]
    pub schema_version: u32,
    pub phase_id: String,
    pub source_hash: String,
    pub tasks: Vec<Task>,
    #[serde(default)]
    pub milestone: Option<Milestone>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerInput {
    pub schema_version: u32,
    pub run_id: String,
    pub task_id: String,
    pub attempt_id: String,
    pub kind: String,
    pub goal: String,
    pub framework: Framework,
    pub base_commit: String,
    pub instruction: String,
    pub task: Option<Task>,
    pub snapshot: Option<Snapshot>,
    pub milestone: Option<Milestone>,
    pub failure: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditItem {
    pub requirement: String,
    pub evidence: String,
    pub passed: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerResult {
    pub schema_version: u32,
    pub run_id: String,
    pub task_id: String,
    pub attempt_id: String,
    pub status: String,
    pub summary: String,
    #[serde(default)]
    pub blockers: Vec<String>,
    pub milestone: Option<Milestone>,
    pub plan: Option<Plan>,
    #[serde(default)]
    pub audit: Vec<AuditItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    pub argv: Vec<String>,
    pub cwd: String,
    pub revision: String,
    pub exit_code: i32,
    pub log: String,
    pub log_hash: String,
    pub finished_at: String,
    #[serde(default)]
    pub tree_unchanged: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attempt {
    pub id: String,
    pub task_id: String,
    pub phase_id: String,
    pub kind: String,
    pub worktree: String,
    pub branch: String,
    pub base_commit: String,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub summary: String,
    pub error: Option<String>,
    pub pid: Option<u32>,
    pub process_identity: Option<String>,
    #[serde(default)]
    pub evidence: Vec<Evidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<serde_json::Value>,
    /// sha256 of the worker patch against base_commit, persisted before its worktree is
    /// reclaimed so retry fingerprints never need the checkout again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch_hash: Option<String>,
    /// `reclaimed` once the worktree directory was removed; the branch and patch stay.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree_state: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationIntent {
    pub id: String,
    pub attempt_id: String,
    pub task_id: String,
    pub worktree: String,
    pub expected_head: String,
    pub candidate_head: Option<String>,
    pub final_head: Option<String>,
    pub state: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Run {
    #[serde(default)]
    pub verification_reuses: u64,
    pub schema_version: u32,
    pub id: String,
    pub milestone: Milestone,
    pub mode: String,
    pub range: Range,
    pub selected_phases: Vec<String>,
    pub origin: String,
    pub origin_head: String,
    pub origin_branch: String,
    pub project_relative: String,
    pub integration: String,
    pub integration_branch: String,
    pub accepted_head: String,
    pub status: String,
    pub stage: String,
    pub current_phase: Option<String>,
    pub started_at: String,
    pub updated_at: String,
    pub elapsed_ms: u64,
    pub blocker: Option<String>,
    pub config: crate::config::Config,
    #[serde(default)]
    pub completed_phases: Vec<String>,
    /// Completed phases whose command gates must run again before delivery because the
    /// tree changed underneath them; their accepted work and plans are retained.
    #[serde(default)]
    pub stale_phases: Vec<String>,
    #[serde(default)]
    pub phase_hashes: BTreeMap<String, String>,
    #[serde(default)]
    pub plans: BTreeMap<String, Plan>,
    #[serde(default)]
    pub completed_tasks: Vec<String>,
    #[serde(default)]
    pub attempts: Vec<Attempt>,
    #[serde(default)]
    pub intents: Vec<IntegrationIntent>,
    #[serde(default)]
    pub origin_sources: BTreeMap<String, String>,
    #[serde(default)]
    pub evidence: Vec<Evidence>,
    #[serde(default)]
    pub repair_rounds: u32,
    #[serde(default)]
    pub hook_results: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<HostState>,
}
impl Run {
    pub fn terminal(&self) -> bool {
        matches!(
            self.status.as_str(),
            "completed" | "scope_completed" | "cancelled"
        )
    }
    pub fn key(phase: &str, task: &str) -> String {
        format!("{phase}/{task}")
    }
    pub fn has_outstanding_host_work(&self) -> bool {
        // Index the latest success per unit once instead of rescanning per attempt.
        let mut latest_success = BTreeMap::<(&str, &str, &str), usize>::new();
        for (index, attempt) in self.attempts.iter().enumerate() {
            if matches!(attempt.status.as_str(), "accepted" | "integrated") {
                latest_success.insert((&attempt.kind, &attempt.task_id, &attempt.phase_id), index);
            }
        }
        self.attempts.iter().enumerate().any(|(index, attempt)| {
            matches!(attempt.status.as_str(), "issued" | "claimed" | "receiving")
                || (attempt.status == "submitted"
                    && !latest_success
                        .get(&(
                            attempt.kind.as_str(),
                            attempt.task_id.as_str(),
                            attempt.phase_id.as_str(),
                        ))
                        .is_some_and(|&success| success > index))
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HostState {
    pub protocol_version: u32,
    pub requests: BTreeMap<String, WorkLease>,
    #[serde(default)]
    pub decisions: Vec<serde_json::Value>,
    #[serde(default)]
    pub blockers: BTreeMap<String, String>,
    #[serde(default)]
    pub pending_repair: Option<PendingRepair>,
    #[serde(default)]
    pub source_revisions: BTreeMap<String, String>,
    /// Reviewed run-local verification changes; baseline spec/plan files remain immutable.
    #[serde(default)]
    pub verification_revisions: Vec<serde_json::Value>,
    /// Earlier failures stay in history, but do not consume the revised task's retry budget.
    #[serde(default)]
    pub task_retry_epochs: BTreeMap<String, usize>,
    /// Passing checks keyed by tree, checkout class, command and environment identity.
    /// Only identical keys reuse evidence; failures are never stored.
    #[serde(default)]
    pub verification_cache: BTreeMap<String, VerificationPass>,
    /// Fully passed audits keyed by phase, scope, integration tree and acceptance ids.
    #[serde(default)]
    pub audit_cache: BTreeMap<String, Vec<AuditItem>>,
    /// The original source changed while host work was outstanding; merge it once that
    /// work has been received instead of revoking it.
    #[serde(default)]
    pub reconcile_pending: bool,
    /// Repair rounds per `phase:failure-signature`; the same failure recurring after a
    /// repair stops the loop instead of spending the whole budget on it.
    #[serde(default)]
    pub repair_signatures: BTreeMap<String, u32>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationPass {
    pub stage: String,
    pub wave_id: String,
    pub tree: String,
    pub revision: String,
    pub manifest: String,
    pub log: String,
    pub log_hash: String,
    pub finished_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkLease {
    pub token: String,
    pub input_hash: String,
    pub request_key: String,
    pub issued_at_ms: u64,
    pub heartbeat_at_ms: u64,
    pub owner: Option<HostIdentity>,
    pub receipt_hash: Option<String>,
    #[serde(default)]
    pub revoked: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HostIdentity {
    pub host_id: String,
    pub session_id: String,
    pub fresh_context: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingRepair {
    pub phase_id: String,
    pub source_hash: String,
    pub audit: Vec<AuditItem>,
}
