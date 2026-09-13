use super::*;
pub fn compact(mut value: Value) -> Value {
    if let Some(report) = value.get_mut("verification_readiness")
        && report.is_object()
    {
        let checks = report["checks"].as_array().map_or(0, Vec::len);
        let diagnostics = report["diagnostics"].as_array().map_or(0, Vec::len);
        report["counts"] = json!({"checks":checks,"diagnostics":diagnostics});
        for (key, limit) in [("checks", 8), ("diagnostics", 16)] {
            if let Some(items) = report[key].as_array_mut() {
                items.truncate(limit);
            }
        }
        report["truncated"] = json!(checks > 8 || diagnostics > 16);
        report["full_view"] =
            json!("Use next or prepare with view: full for the complete readiness report.");
    }
    for key in ["run", "progress", "inventory"] {
        if let Some(v) = value.get_mut(key) {
            *v = compact(v.take());
        }
    }
    if value.get("attempts").is_some_and(Value::is_array) && value.get("milestone").is_some() {
        let run = value["id"].clone();
        let count = value["attempts"].as_array().map_or(0, Vec::len);
        let verified = value["completed_tasks"].as_array().map_or(0, Vec::len);
        let keep = [
            "id",
            "status",
            "stage",
            "milestone",
            "mode",
            "range",
            "selected_phases",
            "completed_phases",
            "current_phase",
            "blocker",
            "accepted_head",
            "integration",
            "integration_branch",
            "work",
            "host_action",
            "starts_agents",
            "receipt_replayed",
            "execution_model",
            "updated_at",
            "verification_readiness",
            "verification_revision_count",
            "recovery_options",
        ];
        value
            .as_object_mut()
            .unwrap()
            .retain(|k, _| keep.contains(&k.as_str()));
        let m = &mut value["milestone"];
        if let Some(map) = m.as_object_mut() {
            let phases = map
                .get("phases")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            map.retain(|k, _| ["id", "goal", "framework", "revision"].contains(&k.as_str()));
            map.insert("phase_count".into(), json!(phases));
        }
        value["counts"] = json!({"attempts":count,"verified_tasks":verified});
        value["details"] =
            json!({"capability":"state.get","arguments":{"run_id":run,"view":"full"}});
    }
    if let Some(files) = value.get_mut("context_files").and_then(Value::as_array_mut) {
        for file in files {
            let bytes = file["content"].as_str().map_or(0, str::len);
            if let Some(m) = file.as_object_mut() {
                m.remove("content");
                m.insert("bytes".into(), json!(bytes));
            }
        }
    }
    if value.get("context_files").is_some() {
        if let Some(action) = value.get_mut("next_action").and_then(Value::as_object_mut) {
            action.remove("instruction");
            action.insert(
                "instruction_capability".into(),
                json!("native.instructions"),
            );
        }
        value["view"] = json!("agent");
    }
    value
}
fn field<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = value;
    for part in path.split('.') {
        current = if let Ok(index) = part.parse::<usize>() {
            current.get(index)?
        } else {
            current.get(part)?
        };
    }
    Some(current)
}
pub fn select_page(value: &mut Value, args: &Value) -> Result<()> {
    let limit = args["limit"].as_u64().unwrap_or(50);
    let offset = args["offset"].as_u64().unwrap_or(0);
    if limit == 0 || limit > 200 || offset > usize::MAX as u64 {
        bail!("invalid_pagination: limit must be 1..200");
    }
    let mut pages = serde_json::Map::new();
    if let Some(map) = value.as_object_mut() {
        for name in [
            "worktrees",
            "runs",
            "sources",
            "events",
            "tasks",
            "capabilities",
            "decisions",
            "summaries",
            "findings",
            "references",
        ] {
            if let Some(items) = map.get_mut(name).and_then(Value::as_array_mut) {
                let total = items.len();
                let end = (offset as usize).saturating_add(limit as usize).min(total);
                let start = (offset as usize).min(total);
                let page = items[start..end].to_vec();
                *items = page;
                pages.insert(name.into(),json!({"total":total,"offset":offset,"limit":limit,"next_offset":if end<total{Some(end)}else{None}}));
            }
        }
        if !pages.is_empty() {
            map.insert("pagination".into(), Value::Object(pages));
        }
    }
    if let Some(fields) = args["fields"].as_array() {
        let mut selected = serde_json::Map::new();
        for key in fields {
            let key = key.as_str().context("invalid_fields")?;
            selected.insert(
                key.into(),
                field(value, key)
                    .with_context(|| format!("field_not_found: {key}"))?
                    .clone(),
            );
        }
        *value = Value::Object(selected);
    }
    Ok(())
}
