use serde_json::json;
use spec_autonomous_core::{
    model::{AuditItem, Snapshot},
    provider,
};

fn snapshot() -> Snapshot {
    serde_json::from_value(json!({
        "schema_version":1,"framework":"openspec","selector":"todo-list","source_dir":"openspec/changes/todo-list","tracking_file":null,"planning_ready":true,
        "next_action":{"kind":"implement","artifact":"tasks","instruction":"","outputs":[]},"tasks":[],"context_files":[],"diagnostics":[],"source_hash":"hash","metadata":{"acceptance":[{"id":"req-1"},{"id":"req-2"}]}
    })).unwrap()
}
#[test]
fn audit_contract_reports_missing_unexpected_duplicate_and_empty_evidence_ids() {
    let error = provider::check_audit(
        &snapshot(),
        &[
            AuditItem {
                requirement: "req-1".into(),
                evidence: "evidence".into(),
                passed: true,
            },
            AuditItem {
                requirement: "verify: guessed".into(),
                evidence: "evidence".into(),
                passed: false,
            },
            AuditItem {
                requirement: "req-1".into(),
                evidence: "".into(),
                passed: false,
            },
        ],
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("missing=[\"req-2\"]"));
    assert!(error.contains("unexpected=[\"verify: guessed\"]"));
    assert!(error.contains("duplicate=[\"req-1\"]"));
    assert!(error.contains("no_evidence=[\"req-1\"]"));
}
