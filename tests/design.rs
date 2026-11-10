mod common;

use std::collections::HashMap;
use std::path::Path;

use assert_cmd::assert::OutputAssertExt;
use common::{INTERFACE_TOON, MODEL_TOON, ORIGIN, Sandbox, keys, submit};
use rusqlite::Connection;
use serde_json::{Value, json};

fn model_json() -> Value {
    json!({
        "entities": [
            {
                "name": "Order",
                "description": "A customer order",
                "fields": [
                    {"name": "id", "type": "uuid", "nullable": false},
                    {"name": "total", "type": "decimal", "nullable": false}
                ],
                "relations": [{"target": "Customer", "cardinality": "one", "name": "customer"}]
            },
            {
                "name": "Customer",
                "fields": [{"name": "id", "type": "uuid", "nullable": false}]
            }
        ]
    })
}

fn interface_json() -> Value {
    json!({
        "operations": [
            {
                "name": "createOrder",
                "description": "Creates an order for a customer",
                "inputs": [{"name": "customer_id", "type": "uuid", "nullable": false}],
                "outputs": [{"name": "order", "type": "Order", "nullable": false}],
                "errors": [{"name": "CustomerNotFound", "condition": "no customer has customer_id"}]
            },
            {
                "name": "getOrder",
                "inputs": [{"name": "order_id", "type": "uuid", "nullable": false}],
                "outputs": [{"name": "order", "type": "Order", "nullable": false}],
                "errors": [{"name": "OrderNotFound", "condition": "no order has order_id"}]
            }
        ]
    })
}

fn submit_args<'a>(name: &'a str, file: &'a Path) -> [&'a str; 8] {
    submit_kind_args("data-model", name, file)
}

fn submit_kind_args<'a>(kind: &'a str, name: &'a str, file: &'a Path) -> [&'a str; 8] {
    let file = file.to_str().unwrap();
    [
        "design", "submit", "--kind", kind, "--name", name, "--file", file,
    ]
}

fn submit_json(sandbox: &Sandbox, dir: &Path, name: &str, record: &Value) -> Value {
    submit(sandbox, dir, "data-model", name, &record.to_string())
}

/// Submits `record` as JSON, asserts that it is rejected, and returns the error output.
fn rejected(sandbox: &Sandbox, dir: &Path, record: &Value) -> Value {
    rejected_as(sandbox, dir, "rejected", record)
}

fn rejected_as(sandbox: &Sandbox, dir: &Path, name: &str, record: &Value) -> Value {
    let file = sandbox.file("rejected.json", &record.to_string());
    let (code, value) = sandbox.json_with_code(dir, &submit_args(name, &file));
    assert_eq!(code, 1, "stdout:\n{value:#}");
    assert_eq!(value["error"]["code"], "invalid-submission");
    value
}

fn diff_with_code(sandbox: &Sandbox, dir: &Path, id: &str, from: u32, to: u32) -> (i32, Value) {
    let (from, to) = (from.to_string(), to.to_string());
    sandbox.json_with_code(dir, &["design", "diff", id, "--from", &from, "--to", &to])
}

fn diff(sandbox: &Sandbox, dir: &Path, id: &str, from: u32, to: u32) -> Value {
    let (code, value) = diff_with_code(sandbox, dir, id, from, to);
    assert_eq!(code, 0, "stdout:\n{value:#}");
    value["diff"].clone()
}

/// Submits the example record as version 1 of `orders` and returns its id and stored record.
fn first_version(sandbox: &Sandbox, dir: &Path) -> (String, Value) {
    let id = id_of(&submit(sandbox, dir, "data-model", "orders", MODEL_TOON)).to_owned();
    let record = get(sandbox, dir, &id)["record"].clone();
    (id, record)
}

fn newest_version(sandbox: &Sandbox, dir: &Path) -> Value {
    let list = sandbox.json(dir, &["design", "list"]);
    assert_eq!(list["count"], "1 of 1 total");
    list["designs"][0]["version"].clone()
}

fn codes(problems: &Value) -> Vec<&str> {
    let mut codes: Vec<&str> = problems
        .as_array()
        .unwrap()
        .iter()
        .map(|problem| problem["code"].as_str().unwrap())
        .collect();
    codes.sort_unstable();
    codes
}

fn get(sandbox: &Sandbox, dir: &Path, id: &str) -> Value {
    sandbox.json(dir, &["design", "get", id])
}

fn id_of(submitted: &Value) -> &str {
    submitted["design"]["id"].as_str().unwrap()
}

fn is_id(text: &str) -> bool {
    text.len() == 12
        && text[1..]
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
}

/// Replaces every id with a placeholder numbered by first appearance, so that two records with
/// different ids but the same structure compare equal.
fn without_ids(value: &Value, seen: &mut HashMap<String, usize>) -> Value {
    match value {
        Value::String(text) if is_id(text) => {
            let next = seen.len();
            json!(format!("<id{}>", seen.entry(text.clone()).or_insert(next)))
        }
        Value::Array(items) => items.iter().map(|item| without_ids(item, seen)).collect(),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, item)| (key.clone(), without_ids(item, seen)))
                .collect(),
        ),
        other => other.clone(),
    }
}

fn store_count(sandbox: &Sandbox, table: &str) -> i64 {
    Connection::open(sandbox.store_path())
        .unwrap()
        .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .unwrap()
}

#[test]
fn toon_submission_returns_id_and_version_1() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    let submitted = submit(&sandbox, &repo, "data-model", "orders", MODEL_TOON);
    let design = &submitted["design"];
    let id = id_of(&submitted);
    assert!(id.starts_with('d') && is_id(id), "id: {id}");
    assert_eq!(design["name"], "orders");
    assert_eq!(design["kind"], "data-model");
    assert_eq!(design["version"], 1);
    assert_eq!(design["branch"], "main");
    assert_eq!(design["status"], "created");
    assert_eq!(
        keys(design),
        ["id", "name", "kind", "version", "branch", "status"]
    );
    let help = submitted["help"].to_string();
    assert!(help.contains(&format!("skald design get {id}")), "{help}");
    assert!(help.contains(&format!("skald design open {id}")), "{help}");
}

#[test]
fn get_prints_the_stored_record_with_assigned_ids_in_toon_and_json() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    let id = id_of(&submit(&sandbox, &repo, "data-model", "orders", MODEL_TOON)).to_owned();

    let assert = sandbox
        .cmd_in(&repo)
        .args(["design", "get", &id])
        .assert()
        .success();
    let toon = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert!(toon.starts_with("design:\n"), "stdout:\n{toon}");
    assert!(
        toon.contains("\nrecord:\n  entities[2]:\n"),
        "stdout:\n{toon}"
    );
    let from_toon: Value = toon_format::decode_default(&toon).unwrap();
    let got = get(&sandbox, &repo, &id);
    assert_eq!(from_toon, got);

    assert_eq!(
        keys(&got),
        ["design", "record", "help"],
        "the detail view has the design, its record, and next steps"
    );
    assert_eq!(
        keys(&got["design"]),
        ["id", "name", "kind", "branch", "version", "submitted_at"]
    );
    let submitted_at = got["design"]["submitted_at"].as_str().unwrap();
    assert!(
        submitted_at.len() == 20 && submitted_at.ends_with('Z'),
        "{submitted_at}"
    );
    let order = &got["record"]["entities"][0];
    assert_eq!(
        keys(order),
        ["id", "name", "description", "fields", "relations"]
    );
    assert!(order["id"].as_str().unwrap().starts_with('e'));
    assert_eq!(
        keys(&order["fields"][0]),
        ["id", "name", "type", "nullable"]
    );
    assert!(order["fields"][0]["id"].as_str().unwrap().starts_with('f'));
    assert_eq!(
        keys(&order["relations"][0]),
        ["id", "target", "cardinality", "name"]
    );
    assert!(
        order["relations"][0]["id"]
            .as_str()
            .unwrap()
            .starts_with('r')
    );
}

#[test]
fn json_submission_equals_the_toon_submission() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);

    let from_toon = id_of(&submit(&sandbox, &repo, "data-model", "orders", MODEL_TOON)).to_owned();
    let from_json = id_of(&submit(
        &sandbox,
        &repo,
        "data-model",
        "orders-json",
        &model_json().to_string(),
    ))
    .to_owned();
    let toon_record = &get(&sandbox, &repo, &from_toon)["record"];
    let json_record = &get(&sandbox, &repo, &from_json)["record"];
    assert_ne!(toon_record, json_record, "ids differ before replacement");
    assert_eq!(
        without_ids(toon_record, &mut HashMap::new()),
        without_ids(json_record, &mut HashMap::new())
    );
}

#[test]
fn unknown_relation_target_is_rejected_and_nothing_is_stored() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    let record = json!({
        "entities": [{
            "name": "Order",
            "fields": [{"name": "id", "type": "uuid"}],
            "relations": [{"target": "Nowhere", "cardinality": "one"}]
        }]
    });

    let error = rejected(&sandbox, &repo, &record);
    assert_eq!(error["error"]["message"], "1 problem found");
    assert_eq!(
        error["problems"],
        json!([{
            "path": "/entities/0/relations/0",
            "id": null,
            "code": "unknown-target",
            "message": error["problems"][0]["message"]
        }])
    );
    assert_eq!(
        keys(&error["problems"][0]),
        ["path", "id", "code", "message"]
    );
    assert!(!error["help"].as_array().unwrap().is_empty());
    assert_eq!(store_count(&sandbox, "design"), 0);
    assert_eq!(store_count(&sandbox, "design_version"), 0);

    let list = sandbox.json(&repo, &["design", "list"]);
    assert_eq!(list["count"], "0 of 0 total");
}

#[test]
fn all_problems_are_reported_together() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    let record = json!({
        "entities": [
            {
                "name": "Order",
                "fields": [{"name": "id", "type": "uuid"}, {"name": "", "type": "decimal"}],
                "relations": [{"target": "Order", "cardinality": "lots"}]
            },
            {"name": "Order", "fields": [{"name": "id", "type": "uuid"}]}
        ]
    });

    let error = rejected(&sandbox, &repo, &record);
    assert_eq!(error["error"]["message"], "3 problems found");
    assert_eq!(
        codes(&error["problems"]),
        ["duplicate-name", "invalid-cardinality", "missing-name"]
    );
    let paths: Vec<&str> = error["problems"]
        .as_array()
        .unwrap()
        .iter()
        .map(|problem| problem["path"].as_str().unwrap())
        .collect();
    for path in [
        "/entities/1",
        "/entities/0/fields/1",
        "/entities/0/relations/0",
    ] {
        assert!(paths.contains(&path), "{path} missing from {paths:?}");
    }
}

#[test]
fn unknown_key_is_rejected() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    let record = json!({
        "entities": [{
            "name": "Order",
            "fields": [{"name": "id", "type": "uuid", "nullabel": true}]
        }]
    });

    let error = rejected(&sandbox, &repo, &record);
    let problems = error["problems"].as_array().unwrap();
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(problems[0]["code"], "unknown-key");
    let path = problems[0]["path"].as_str().unwrap();
    assert!(path.ends_with("/nullabel"), "path: {path}");
}

#[test]
fn undecodable_file_is_an_invalid_format_problem() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    let file = sandbox.file("broken.json", "{\"entities\": [");

    let (code, error) = sandbox.json_with_code(&repo, &submit_args("broken", &file));
    assert_eq!(code, 1, "stdout:\n{error:#}");
    assert_eq!(error["error"]["code"], "invalid-submission");
    assert_eq!(codes(&error["problems"]), ["invalid-format"]);
}

#[test]
fn nullable_defaults_to_false() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    let toon = "entities[1]:\n  - name: Order\n    fields[1]{name,type}:\n      id,uuid\n";

    let id = id_of(&submit(&sandbox, &repo, "data-model", "orders", toon)).to_owned();
    let field = &get(&sandbox, &repo, &id)["record"]["entities"][0]["fields"][0];
    assert_eq!(field["nullable"], false);
}

#[test]
fn relation_target_resolves_to_the_entity_id() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    let id = id_of(&submit(&sandbox, &repo, "data-model", "orders", MODEL_TOON)).to_owned();
    let entities = &get(&sandbox, &repo, &id)["record"]["entities"];
    assert_eq!(entities[1]["name"], "Customer");
    assert_eq!(entities[0]["relations"][0]["target"], entities[1]["id"]);
}

#[test]
fn submission_from_stdin() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);

    let assert = assert_cmd::Command::from_std(sandbox.cmd_in(&repo))
        .args([
            "design",
            "submit",
            "--kind",
            "data-model",
            "--name",
            "s",
            "--file",
            "-",
            "--json",
        ])
        .write_stdin(MODEL_TOON)
        .assert()
        .success();
    let submitted: Value = serde_json::from_slice(&assert.get_output().stdout).unwrap();
    assert_eq!(submitted["design"]["version"], 1);
}

#[test]
fn kind_must_be_known() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    let file = sandbox.file("model.toon", MODEL_TOON);

    let (code, error) = sandbox.json_with_code(
        &repo,
        &[
            "design",
            "submit",
            "--kind",
            "sketch",
            "--name",
            "orders",
            "--file",
            file.to_str().unwrap(),
        ],
    );
    assert_eq!(code, 2, "stdout:\n{error:#}");
    assert_eq!(error["error"]["code"], "invalid-kind");
    let help = error["help"].to_string();
    assert!(help.contains("data-model"), "help: {help}");
}

#[test]
fn list_shows_four_fields_by_default() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    submit(&sandbox, &repo, "data-model", "orders", MODEL_TOON);
    submit(&sandbox, &repo, "data-model", "billing", MODEL_TOON);

    let list = sandbox.json(&repo, &["design", "list"]);
    assert_eq!(list["count"], "2 of 2 total");
    let designs = list["designs"].as_array().unwrap();
    assert_eq!(designs.len(), 2);
    for item in designs {
        assert_eq!(keys(item), ["id", "name", "kind", "version"]);
    }
    assert_eq!(designs[0]["name"], "billing", "newest first");
    assert!(list.get("note").is_none());
    assert!(!list["help"].as_array().unwrap().is_empty());

    let list = sandbox.json(&repo, &["design", "list", "--fields", "branch,created_at"]);
    for item in list["designs"].as_array().unwrap() {
        assert_eq!(
            keys(item),
            ["id", "name", "kind", "version", "branch", "created_at"]
        );
        assert_eq!(item["branch"], "main");
    }

    let list = sandbox.json(&repo, &["design", "list", "--limit", "1"]);
    assert_eq!(list["count"], "1 of 2 total");
}

#[test]
fn unknown_list_field_is_a_usage_error() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);

    let (code, error) = sandbox.json_with_code(&repo, &["design", "list", "--fields", "colour"]);
    assert_eq!(code, 2, "stdout:\n{error:#}");
    assert_eq!(error["error"]["code"], "unknown-field");
    let help = error["help"].to_string();
    assert!(help.contains("submitted_at"), "help: {help}");
}

#[test]
fn empty_list_is_explicit() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);

    let assert = sandbox
        .cmd_in(&repo)
        .args(["design", "list"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    // TOON spec v4.4, which toon-format 0.7 follows, writes an empty array as `key: []`.
    assert!(stdout.starts_with("designs: []\n"), "stdout:\n{stdout}");
    assert!(
        stdout.contains("\ncount: 0 of 0 total\n"),
        "stdout:\n{stdout}"
    );
    let note = stdout
        .lines()
        .find(|line| line.starts_with("note: "))
        .unwrap_or_else(|| panic!("no note in:\n{stdout}"));
    assert!(note.contains("0 designs"), "note: {note}");
    assert!(note.contains("github.com/acme/orders"), "note: {note}");
}

#[test]
fn long_description_is_truncated_with_a_size_hint() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    let description = "x".repeat(1200);
    let record = json!({
        "entities": [{"name": "Order", "description": description, "fields": []}]
    });
    let id = id_of(&submit(
        &sandbox,
        &repo,
        "data-model",
        "orders",
        &record.to_string(),
    ))
    .to_owned();

    let short = get(&sandbox, &repo, &id);
    let text = short["record"]["entities"][0]["description"]
        .as_str()
        .unwrap();
    assert_eq!(
        text,
        format!("{} ... (truncated, 1200 chars total)", "x".repeat(500))
    );
    let help = short["help"].to_string();
    assert!(
        help.contains(&format!("skald design get {id} --full")),
        "help: {help}"
    );

    let full = sandbox.json(&repo, &["design", "get", &id, "--full"]);
    let text = full["record"]["entities"][0]["description"]
        .as_str()
        .unwrap();
    assert_eq!(text.chars().count(), 1200);
    assert!(!full["help"].to_string().contains("--full"));
}

#[test]
fn resubmitting_truncated_text_is_rejected() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    let record = json!({
        "entities": [{"name": "Order", "description": "x".repeat(800), "fields": []}]
    });
    let id = id_of(&submit(
        &sandbox,
        &repo,
        "data-model",
        "orders",
        &record.to_string(),
    ))
    .to_owned();

    let mut edited = get(&sandbox, &repo, &id)["record"].clone();
    edited["entities"][0]["name"] = json!("Purchase");
    let error = rejected_as(&sandbox, &repo, "orders", &edited);

    assert_eq!(codes(&error["problems"]), ["truncated-text"]);
    assert_eq!(error["problems"][0]["path"], "/entities/0/description");
    let message = error["problems"][0]["message"].as_str().unwrap();
    assert!(
        message.contains(&format!("skald design get {id} --full")),
        "{message}"
    );
}

#[test]
fn design_is_visible_from_another_worktree() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    let worktree = sandbox.worktree(&repo, "feature-x");
    let id = id_of(&submit(&sandbox, &repo, "data-model", "orders", MODEL_TOON)).to_owned();

    let list = sandbox.json(&worktree, &["design", "list"]);
    assert_eq!(list["count"], "1 of 1 total");
    assert_eq!(list["designs"][0]["id"], id.as_str());
    assert_eq!(get(&sandbox, &worktree, &id)["design"]["branch"], "main");
}

#[test]
fn get_unknown_id() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);

    let (code, error) = sandbox.json_with_code(&repo, &["design", "get", "dnope00000000"]);
    assert_eq!(code, 1, "stdout:\n{error:#}");
    assert_eq!(error["error"]["code"], "not-found");
    assert!(!error["help"].as_array().unwrap().is_empty());
}

#[test]
fn design_commands_need_a_repository() {
    let sandbox = Sandbox::new();
    let file = sandbox.file("model.toon", MODEL_TOON);

    for args in [
        &["design", "list"][..],
        &["design", "get", "dnope00000000"],
        &submit_args("orders", &file),
    ] {
        let (code, error) = sandbox.json_with_code(&sandbox.work, args);
        assert_eq!(code, 1, "{args:?} stdout:\n{error:#}");
        assert_eq!(error["error"]["code"], "not-a-repository");
    }
}

#[test]
fn design_commands_report_git_refusing_the_repository() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);

    let output = sandbox
        .cmd_in(&repo)
        .args(["design", "list", "--json"])
        .env("GIT_TEST_ASSUME_DIFFERENT_OWNER", "1")
        .output()
        .unwrap();
    let error: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();

    assert_eq!(output.status.code(), Some(1), "stdout:\n{error:#}");
    assert_eq!(error["error"]["code"], "git-failed");
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("dubious ownership"),
        "{error:#}"
    );
}

#[test]
fn home_view_lists_the_newest_designs() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    let names = ["d1", "d2", "d3", "d4", "d5", "d6", "d7"];
    for name in names {
        submit(&sandbox, &repo, "data-model", name, MODEL_TOON);
    }

    let home = sandbox.json(&repo, &[]);
    assert_eq!(home["designs_count"], 7);
    let designs = home["designs"].as_array().unwrap();
    let shown: Vec<&str> = designs
        .iter()
        .map(|item| item["name"].as_str().unwrap())
        .collect();
    assert_eq!(shown, ["d7", "d6", "d5", "d4", "d3"]);
    for item in designs {
        assert_eq!(keys(item), ["id", "name", "kind", "version"]);
    }
}

#[test]
fn identical_resubmission_is_a_no_op() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    let (id, record) = first_version(&sandbox, &repo);

    let again = submit_json(&sandbox, &repo, "orders", &record);
    let design = &again["design"];
    assert_eq!(design["id"], id.as_str());
    assert_eq!(design["status"], "unchanged");
    assert_eq!(design["version"], 1);
    assert!(!again["help"].as_array().unwrap().is_empty());
    assert_eq!(newest_version(&sandbox, &repo), 1);
    assert_eq!(store_count(&sandbox, "design_version"), 1);
}

#[test]
fn changed_resubmission_creates_version_2() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    let (id, mut record) = first_version(&sandbox, &repo);
    record["entities"][0]["fields"][1]["type"] = json!("money");

    let next = submit_json(&sandbox, &repo, "orders", &record);
    let design = &next["design"];
    assert_eq!(design["id"], id.as_str());
    assert_eq!(design["status"], "created");
    assert_eq!(design["version"], 2);
    let help = next["help"].to_string();
    assert!(
        help.contains(&format!("skald design diff {id} --from 1 --to 2")),
        "help: {help}"
    );
    assert_eq!(newest_version(&sandbox, &repo), 2);
}

#[test]
fn newest_version_is_the_default_read() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    let (id, mut record) = first_version(&sandbox, &repo);
    for r#type in ["money", "cents"] {
        record["entities"][0]["fields"][1]["type"] = json!(r#type);
        submit_json(&sandbox, &repo, "orders", &record);
    }
    let total_type = |got: &Value| got["record"]["entities"][0]["fields"][1]["type"].clone();

    let newest = get(&sandbox, &repo, &id);
    assert_eq!(newest["design"]["version"], 3);
    assert_eq!(total_type(&newest), "cents");
    let first = sandbox.json(&repo, &["design", "get", &id, "--version", "1"]);
    assert_eq!(first["design"]["version"], 1);
    assert_eq!(total_type(&first), "decimal");
    let second = sandbox.json(&repo, &["design", "get", &id, "--version", "2"]);
    assert_eq!(total_type(&second), "money");
}

/// Submits version 2 of `orders` with entity `Order` renamed, and returns the design id and the
/// entity id.
fn renamed(sandbox: &Sandbox, dir: &Path) -> (String, String) {
    let (id, mut record) = first_version(sandbox, dir);
    record["entities"][0]["name"] = json!("PurchaseOrder");
    assert_eq!(
        submit_json(sandbox, dir, "orders", &record)["design"]["version"],
        2
    );
    let entity = record["entities"][0]["id"].as_str().unwrap().to_owned();
    (id, entity)
}

#[test]
fn kept_id_with_a_rename_is_a_change_not_a_removal() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    let (id, entity) = renamed(&sandbox, &repo);

    let diff = diff(&sandbox, &repo, &id, 1, 2);
    assert_eq!(
        keys(&diff),
        ["design", "from", "to", "added", "removed", "changed"]
    );
    assert_eq!(diff["design"], id.as_str());
    assert_eq!(diff["from"], 1);
    assert_eq!(diff["to"], 2);
    assert_eq!(diff["added"], json!([]));
    assert_eq!(diff["removed"], json!([]));
    assert_eq!(
        diff["changed"],
        json!([{"kind": "entity", "id": entity, "name": "PurchaseOrder", "attributes": ["name"]}])
    );
}

#[test]
fn omitted_id_is_a_removal_plus_an_addition() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    let record = json!({"entities": [{"name": "Order"}]});
    let id = id_of(&submit_json(&sandbox, &repo, "orders", &record)).to_owned();
    let first = get(&sandbox, &repo, &id)["record"]["entities"][0]["id"].clone();

    let next = submit_json(&sandbox, &repo, "orders", &record);
    assert_eq!(next["design"]["status"], "created");
    assert_eq!(next["design"]["version"], 2);

    let diff = diff(&sandbox, &repo, &id, 1, 2);
    assert_eq!(
        diff["removed"],
        json!([{"kind": "entity", "id": first, "name": "Order"}])
    );
    let added = diff["added"].as_array().unwrap();
    assert_eq!(added.len(), 1, "{added:?}");
    assert_eq!(added[0]["kind"], "entity");
    assert_eq!(added[0]["name"], "Order");
    assert_ne!(added[0]["id"], first);
    assert_eq!(diff["changed"], json!([]));
}

#[test]
fn nullability_change_lists_only_that_field() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    let (id, mut record) = first_version(&sandbox, &repo);
    let total = &mut record["entities"][0]["fields"][1];
    total["nullable"] = json!(true);
    let total = total["id"].as_str().unwrap().to_owned();
    submit_json(&sandbox, &repo, "orders", &record);

    let diff = diff(&sandbox, &repo, &id, 1, 2);
    assert_eq!(diff["added"], json!([]));
    assert_eq!(diff["removed"], json!([]));
    assert_eq!(
        diff["changed"],
        json!([{"kind": "field", "id": total, "name": "total", "attributes": ["nullable"]}])
    );

    let assert = sandbox
        .cmd_in(&repo)
        .args(["design", "diff", &id, "--from", "1", "--to", "2"])
        .assert()
        .success();
    let toon = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let expected = format!(
        "diff:\n  design: {id}\n  from: 1\n  to: 2\n  added: []\n  removed: []\n  \
         changed[1]:\n    - kind: field\n      id: {total}\n      name: total\n      \
         attributes[1]: nullable\n"
    );
    assert!(toon.starts_with(&expected), "stdout:\n{toon}");
}

#[test]
fn unknown_id_is_rejected() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    first_version(&sandbox, &repo);
    let mut record = model_json();
    record["entities"][0]["id"] = json!("e00000000000");

    let error = rejected_as(&sandbox, &repo, "orders", &record);
    let problems = error["problems"].as_array().unwrap();
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(problems[0]["code"], "unknown-id");
    assert_eq!(problems[0]["path"], "/entities/0");
    assert_eq!(problems[0]["id"], "e00000000000");
    assert_eq!(newest_version(&sandbox, &repo), 1);
}

#[test]
fn id_supplied_on_first_submission_is_rejected() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    let mut record = model_json();
    record["entities"][0]["id"] = json!("eaaaaaaaaaaa");

    let error = rejected_as(&sandbox, &repo, "fresh", &record);
    assert_eq!(codes(&error["problems"]), ["unknown-id"]);
    assert_eq!(store_count(&sandbox, "design"), 0);
}

#[test]
fn id_used_for_the_wrong_kind_is_rejected() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    let (_, record) = first_version(&sandbox, &repo);
    let field = record["entities"][0]["fields"][0]["id"].clone();
    let next = json!({"entities": [{"id": field, "name": "Order"}]});

    let error = rejected_as(&sandbox, &repo, "orders", &next);
    assert_eq!(codes(&error["problems"]), ["unknown-id"]);
    assert_eq!(error["problems"][0]["path"], "/entities/0");
    assert_eq!(newest_version(&sandbox, &repo), 1);
}

#[test]
fn kind_mismatch_is_rejected() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    first_version(&sandbox, &repo);
    let file = sandbox.file("model.toon", MODEL_TOON);

    let (code, error) = sandbox.json_with_code(
        &repo,
        &[
            "design",
            "submit",
            "--kind",
            "interface",
            "--name",
            "orders",
            "--file",
            file.to_str().unwrap(),
        ],
    );
    assert_eq!(code, 1, "stdout:\n{error:#}");
    assert_eq!(error["error"]["code"], "kind-mismatch");
    let message = error["error"]["message"].as_str().unwrap();
    assert!(message.contains("data-model"), "message: {message}");
    assert!(!error["help"].as_array().unwrap().is_empty());
    assert_eq!(newest_version(&sandbox, &repo), 1);
}

#[test]
fn diff_on_a_missing_version() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    let (id, _) = first_version(&sandbox, &repo);

    let (code, error) = diff_with_code(&sandbox, &repo, &id, 1, 9);
    assert_eq!(code, 1, "stdout:\n{error:#}");
    assert_eq!(error["error"]["code"], "not-found");
    assert!(!error["help"].as_array().unwrap().is_empty());

    let (code, error) = diff_with_code(&sandbox, &repo, "dnope00000000", 1, 1);
    assert_eq!(code, 1, "stdout:\n{error:#}");
    assert_eq!(error["error"]["code"], "not-found");
}

#[test]
fn diff_is_symmetric_in_kind() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    let (id, entity) = renamed(&sandbox, &repo);

    let diff = diff(&sandbox, &repo, &id, 2, 1);
    assert_eq!(diff["added"], json!([]));
    assert_eq!(diff["removed"], json!([]));
    assert_eq!(
        diff["changed"],
        json!([{"kind": "entity", "id": entity, "name": "Order", "attributes": ["name"]}])
    );
}

fn submit_interface_json(sandbox: &Sandbox, dir: &Path, name: &str, record: &Value) -> Value {
    submit(sandbox, dir, "interface", name, &record.to_string())
}

/// Submits the interface example as version 1 of `orders-api` and returns its id and stored
/// record.
fn first_interface_version(sandbox: &Sandbox, dir: &Path) -> (String, Value) {
    let id = id_of(&submit(
        sandbox,
        dir,
        "interface",
        "orders-api",
        INTERFACE_TOON,
    ))
    .to_owned();
    let record = get(sandbox, dir, &id)["record"].clone();
    (id, record)
}

/// Submits `record` as an interface, asserts that it is rejected, and returns its problems.
fn rejected_interface(sandbox: &Sandbox, dir: &Path, record: &Value) -> Vec<Value> {
    let file = sandbox.file("rejected.json", &record.to_string());
    let (code, value) = sandbox.json_with_code(dir, &submit_kind_args("interface", "api", &file));
    assert_eq!(code, 1, "stdout:\n{value:#}");
    assert_eq!(value["error"]["code"], "invalid-submission");
    assert_eq!(store_count(sandbox, "design"), 0);
    value["problems"].as_array().unwrap().clone()
}

fn assert_id(value: &Value, prefix: char) {
    let id = value.as_str().unwrap_or_default();
    assert!(
        id.starts_with(prefix) && is_id(id),
        "{value} is not a `{prefix}` id"
    );
}

#[test]
fn interface_submission_returns_id_and_version_1() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    let submitted = submit(&sandbox, &repo, "interface", "orders-api", INTERFACE_TOON);
    let design = &submitted["design"];
    let id = id_of(&submitted).to_owned();
    assert_id(&design["id"], 'd');
    assert_eq!(design["kind"], "interface");
    assert_eq!(design["version"], 1);
    assert_eq!(design["status"], "created");

    let got = get(&sandbox, &repo, &id);
    assert_eq!(got["design"]["kind"], "interface");
    assert_eq!(keys(&got["record"]), ["operations"]);
    let operations = got["record"]["operations"].as_array().unwrap();
    let names: Vec<&str> = operations
        .iter()
        .map(|operation| operation["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["createOrder", "getOrder"]);
    assert_eq!(
        keys(&operations[0]),
        ["id", "name", "description", "inputs", "outputs", "errors"]
    );
    assert_eq!(
        keys(&operations[1]),
        ["id", "name", "inputs", "outputs", "errors"]
    );
    assert_eq!(
        keys(&operations[0]["inputs"][0]),
        ["id", "name", "type", "nullable"]
    );
    assert_eq!(
        keys(&operations[0]["errors"][0]),
        ["id", "name", "condition"]
    );
    for operation in operations {
        assert_id(&operation["id"], 'o');
        for (list, prefix) in [("inputs", 'p'), ("outputs", 'p'), ("errors", 'x')] {
            let items = operation[list].as_array().unwrap();
            assert_eq!(items.len(), 1, "{list} of {operation}");
            assert_id(&items[0]["id"], prefix);
        }
    }

    let assert = sandbox
        .cmd_in(&repo)
        .args(["design", "get", &id])
        .assert()
        .success();
    let toon = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert!(
        toon.contains("\nrecord:\n  operations[2]:\n    - id: o"),
        "stdout:\n{toon}"
    );
    let from_toon: Value = toon_format::decode_default(&toon).unwrap();
    assert_eq!(from_toon, got);
}

#[test]
fn json_and_toon_interface_records_are_equal() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    let from_toon = id_of(&submit(
        &sandbox,
        &repo,
        "interface",
        "api-toon",
        INTERFACE_TOON,
    ))
    .to_owned();
    let from_json = submit_interface_json(&sandbox, &repo, "api-json", &interface_json());
    let toon_record = &get(&sandbox, &repo, &from_toon)["record"];
    let json_record = &get(&sandbox, &repo, id_of(&from_json))["record"];
    assert_ne!(toon_record, json_record, "ids differ before replacement");
    assert_eq!(
        without_ids(toon_record, &mut HashMap::new()),
        without_ids(json_record, &mut HashMap::new())
    );
}

#[test]
fn duplicate_operation_name_is_rejected() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    let record = json!({"operations": [{"name": "getOrder"}, {"name": "getOrder"}]});

    let problems = rejected_interface(&sandbox, &repo, &record);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(problems[0]["code"], "duplicate-name");
    assert_eq!(problems[0]["path"], "/operations/1");
}

#[test]
fn missing_error_condition_is_rejected() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    let record = json!({
        "operations": [{"name": "getOrder", "errors": [{"name": "OrderNotFound"}]}]
    });

    let problems = rejected_interface(&sandbox, &repo, &record);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(problems[0]["code"], "missing-condition");
    assert_eq!(problems[0]["path"], "/operations/0/errors/0");
}

#[test]
fn input_nullability_change_diffs_to_one_input() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    let (id, mut record) = first_interface_version(&sandbox, &repo);
    let input = &mut record["operations"][1]["inputs"][0];
    input["nullable"] = json!(true);
    let input = input["id"].clone();
    let next = submit_interface_json(&sandbox, &repo, "orders-api", &record);
    assert_eq!(next["design"]["version"], 2);

    let diff = diff(&sandbox, &repo, &id, 1, 2);
    assert_eq!(diff["added"], json!([]));
    assert_eq!(diff["removed"], json!([]));
    assert_eq!(
        diff["changed"],
        json!([{"kind": "input", "id": input, "name": "order_id", "attributes": ["nullable"]}])
    );
}

#[test]
fn added_error_diffs_as_an_addition() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    let (id, mut record) = first_interface_version(&sandbox, &repo);
    record["operations"][1]["errors"]
        .as_array_mut()
        .unwrap()
        .push(json!({"name": "OrderArchived", "condition": "the order was archived"}));
    submit_interface_json(&sandbox, &repo, "orders-api", &record);

    let diff = diff(&sandbox, &repo, &id, 1, 2);
    assert_eq!(diff["changed"], json!([]));
    assert_eq!(diff["removed"], json!([]));
    let added = diff["added"].as_array().unwrap();
    assert_eq!(added.len(), 1, "{added:?}");
    assert_eq!(keys(&added[0]), ["kind", "id", "name"]);
    assert_eq!(added[0]["kind"], "error");
    assert_eq!(added[0]["name"], "OrderArchived");
    assert_id(&added[0]["id"], 'x');
}

#[test]
fn list_mixes_kinds() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    first_version(&sandbox, &repo);
    first_interface_version(&sandbox, &repo);

    let list = sandbox.json(&repo, &["design", "list"]);
    assert_eq!(list["count"], "2 of 2 total");
    let kinds: Vec<&str> = list["designs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["interface", "data-model"]);
}

#[test]
fn designs_survive_switching_origin_between_https_and_ssh() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    submit(&sandbox, &repo, "data-model", "orders", MODEL_TOON);

    sandbox.git(
        &repo,
        &[
            "remote",
            "set-url",
            "origin",
            "git@github.com:acme/orders.git",
        ],
    );

    let list = sandbox.json(&repo, &["design", "list"]);
    assert_eq!(list["count"], "1 of 1 total");
}

#[test]
fn design_name_must_be_one_short_token() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    let file = sandbox.file("model.toon", MODEL_TOON);

    for name in ["bad\nname", "bad name", "", "_lead", &"a".repeat(65)] {
        let (code, error) = sandbox.json_with_code(&repo, &submit_args(name, &file));
        assert_eq!(code, 2, "name {name:?}, stdout:\n{error:#}");
        assert_eq!(error["error"]["code"], "invalid-value", "name {name:?}");
    }
    let list = sandbox.json(&repo, &["design", "list"]);
    assert!(
        list["designs"]
            .as_array()
            .is_none_or(|designs| designs.is_empty())
    );

    let longest = "a".repeat(63) + "-";
    let (code, accepted) = sandbox.json_with_code(&repo, &submit_args(&longest, &file));
    assert_eq!(code, 0, "stdout:\n{accepted:#}");
    assert_eq!(accepted["design"]["name"], longest.as_str());
}
