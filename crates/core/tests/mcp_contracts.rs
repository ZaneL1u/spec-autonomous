use serde_json::{Value, json};
use spec_autonomous_core::{
    capabilities::{catalog, views},
    mcp::{Session, serve},
};
use std::io::Cursor;
#[test]
fn malformed_messages_and_unknown_notifications_cannot_invoke_tools() {
    let root = tempfile::tempdir().unwrap();
    let mut s = Session::new(root.path(), false).unwrap();
    assert_eq!(
        s.handle(json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}))
            .unwrap()["error"]["code"],
        -32002
    );
    assert_eq!(s.handle(json!([])).unwrap()["error"]["code"], -32600);
    assert!(
        s.handle(json!({"jsonrpc":"2.0","method":"unknown/notification"}))
            .is_none()
    );
    let response=s.handle(json!({"jsonrpc":"2.0","id":2,"method":"initialize","params":{"protocolVersion":"future","capabilities":{},"clientInfo":{"name":"test","version":"1"}}})).unwrap();
    assert_eq!(response["result"]["protocolVersion"], "2025-11-25");
    s.handle(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
    let list = s
        .handle(json!({"jsonrpc":"2.0","id":3,"method":"tools/list"}))
        .unwrap();
    assert_eq!(list["result"]["tools"].as_array().unwrap().len(), 9);
    assert!(std::fs::read_dir(root.path()).unwrap().next().is_none());
}
#[test]
fn stdio_has_bounded_input_and_clean_eof() {
    let root = tempfile::tempdir().unwrap();
    let mut out = vec![];
    serve(
        root.path(),
        false,
        Cursor::new(b"not json\n{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"ping\"}\n"),
        &mut out,
    )
    .unwrap();
    let rows: Vec<Value> = String::from_utf8(out)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["error"]["code"], -32700);
    assert_eq!(rows[1]["id"], 4);
    let mut out = vec![];
    serve(
        root.path(),
        false,
        Cursor::new(vec![b'x'; 2 * 1024 * 1024 + 1]),
        &mut out,
    )
    .unwrap();
    assert!(
        String::from_utf8(out)
            .unwrap()
            .contains("request_too_large")
    );
}
#[test]
fn compact_views_keep_totals_and_paginate_without_exposing_run_credentials() {
    let mut v = json!({"verified_tasks":83,"active_workers":4,"worktrees":(0..120).map(|n|json!({"id":n})).collect::<Vec<_>>()});
    views::select_page(&mut v, &json!({"limit":20,"offset":40})).unwrap();
    assert_eq!(v["verified_tasks"], 83);
    assert_eq!(v["worktrees"].as_array().unwrap().len(), 20);
    assert_eq!(v["pagination"]["worktrees"]["total"], 120);
    assert_eq!(v["pagination"]["worktrees"]["next_offset"], 60);
    assert!(catalog::validate("prepare", &json!({"goal":"g","fields":["id"]})).is_err());
    assert!(catalog::validate("prepare", &json!({"goal":"g","max_workers":33})).is_err());
    let all = catalog::all();
    let ids: std::collections::BTreeSet<_> = all.iter().map(|c| &c.id).collect();
    assert_eq!(ids.len(), all.len());
    assert_eq!(all.iter().filter(|c| c.primary).count(), 8);
}

#[test]
fn default_catalog_names_hidden_capabilities_and_revise_items_have_a_schema() {
    let primary = catalog::list(false);
    let shown: Vec<&str> = primary["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    let hidden: Vec<&str> = primary["hidden_capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .map(|id| id.as_str().unwrap())
        .collect();
    assert!(hidden.contains(&"run.revise") && hidden.contains(&"work.claim"));
    assert!(hidden.iter().all(|id| !shown.contains(id)));
    assert_eq!(shown.len() + hidden.len(), catalog::all().len());
    assert!(
        primary["hint"]
            .as_str()
            .unwrap()
            .contains("tools list --all")
    );
    let everything = catalog::list(true);
    assert!(everything.get("hidden_capabilities").is_none());
    assert_eq!(
        everything["capabilities"].as_array().unwrap().len(),
        catalog::all().len()
    );

    let revise = catalog::get("run.revise").unwrap().input_schema;
    let item = &revise["properties"]["task_checks"]["items"];
    assert_eq!(item["required"], json!(["phase_id", "task_id", "checks"]));
    assert_eq!(
        item["properties"]["checks"]["items"]["required"],
        json!(["argv"])
    );
    let check = json!({"argv":["node","--test","tests/a.test.mjs"],"cwd":"."});
    assert!(
        catalog::validate(
            "run.revise",
            &json!({"run_id":"r","reason":"fix","task_checks":[{"phase_id":"P001","task_id":"t","checks":[check]}],"phase_checks":[{"phase_id":"P001","checks":[check]}],"milestone_checks":[check]})
        )
        .is_ok()
    );
    for bad in [
        json!({"run_id":"r","reason":"fix","task_checks":[{"phase_id":"P001","checks":[check]}]}),
        json!({"run_id":"r","reason":"fix","task_checks":[{"phase_id":"P001","task_id":"t","checks":[{"cmd":"x"}]}]}),
        json!({"run_id":"r","reason":"fix","phase_checks":[{"phase_id":"P001","checks":[check],"extra":1}]}),
        json!({"run_id":"r","reason":"fix","milestone_checks":[{"argv":"node"}]}),
    ] {
        assert!(catalog::validate("run.revise", &bad).is_err(), "{bad}");
    }
}
