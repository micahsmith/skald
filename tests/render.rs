mod common;

use std::collections::BTreeSet;
use std::path::Path;

use assert_cmd::assert::OutputAssertExt;
use common::{INTERFACE_TOON, MODEL_TOON, ORIGIN, Sandbox, submit_id};
use scraper::{ElementRef, Html, Selector};
use serde_json::{Value, json};

fn submit_example(sandbox: &Sandbox, dir: &Path) -> String {
    submit_id(sandbox, dir, "data-model", "orders", MODEL_TOON)
}

/// Renders design `id` to `out` with `--json`, asserts success, and returns the output.
fn render(sandbox: &Sandbox, dir: &Path, id: &str, extra: &[&str], out: &str) -> Value {
    let mut args = vec!["design", "render", id, "--out", out];
    args.extend(extra);
    sandbox.json(dir, &args)
}

/// Renders design `id` and returns the page.
fn page(sandbox: &Sandbox, dir: &Path, id: &str, extra: &[&str]) -> String {
    let rendered = render(sandbox, dir, id, extra, "page.html");
    std::fs::read_to_string(rendered["rendered"]["path"].as_str().unwrap()).unwrap()
}

fn select<'a>(html: &'a Html, selector: &str) -> Vec<ElementRef<'a>> {
    let selector = Selector::parse(selector).unwrap();
    html.select(&selector).collect()
}

fn select_in<'a>(element: &ElementRef<'a>, selector: &str) -> Vec<ElementRef<'a>> {
    let selector = Selector::parse(selector).unwrap();
    element.select(&selector).collect()
}

fn text(element: &ElementRef<'_>) -> String {
    element.text().collect()
}

fn data_id(element: &ElementRef<'_>) -> String {
    element.value().attr("data-id").unwrap().to_owned()
}

/// Asserts that `page` is one standalone document that loads nothing from elsewhere.
fn assert_self_contained(page: &str) {
    assert!(page.starts_with("<!doctype html>"), "page:\n{page}");
    for banned in ["<script", "<link", " src=", "http://", "https://"] {
        assert!(!page.contains(banned), "{banned} in page:\n{page}");
    }
    assert_eq!(page.matches("<style").count(), 1, "page:\n{page}");
}

/// Every element id in a stored data-model record.
fn record_ids(record: &Value) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    for entity in record["entities"].as_array().unwrap() {
        ids.insert(entity["id"].as_str().unwrap().to_owned());
        for key in ["fields", "relations"] {
            for element in entity[key].as_array().unwrap() {
                ids.insert(element["id"].as_str().unwrap().to_owned());
            }
        }
    }
    ids
}

#[test]
fn render_is_byte_identical_across_runs() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    let id = submit_example(&sandbox, &repo);

    let assert = sandbox
        .cmd_in(&repo)
        .args(["design", "render", &id, "--out", "a.html"])
        .assert()
        .success();
    let toon = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let a = std::fs::read(repo.join("a.html")).unwrap();
    assert_eq!(
        toon,
        format!(
            "rendered:\n  design: {id}\n  version: 1\n  path: {}\n  bytes: {}\n",
            repo.join("a.html").display(),
            a.len()
        )
    );

    let rendered = render(&sandbox, &repo, &id, &[], "b.html");
    let b = std::fs::read(repo.join("b.html")).unwrap();
    assert_eq!(a, b);
    assert_eq!(
        rendered,
        json!({
            "rendered": {
                "design": id,
                "version": 1,
                "path": repo.join("b.html").to_str().unwrap(),
                "bytes": b.len(),
            }
        })
    );
}

#[test]
fn nodes_and_edges_carry_record_ids() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    let id = submit_example(&sandbox, &repo);
    let record = sandbox.json(&repo, &["design", "get", &id])["record"].clone();

    let html = Html::parse_document(&page(&sandbox, &repo, &id, &[]));
    assert_eq!(select(&html, r#"[data-kind="entity"]"#).len(), 2);
    assert_eq!(select(&html, r#"[data-kind="relation"]"#).len(), 1);
    assert_eq!(select(&html, r#"[data-kind="field"]"#).len(), 3);
    let page_ids: BTreeSet<String> = select(&html, "[data-id]")
        .iter()
        .map(|element| element.value().attr("data-id").unwrap().to_owned())
        .collect();
    assert_eq!(page_ids, record_ids(&record));
    let labels = select(&html, r#"[data-kind="relation"] + text"#);
    assert_eq!(labels.len(), 1);
    assert_eq!(text(&labels[0]), "1");
}

#[test]
fn page_has_no_external_references() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    let id = submit_example(&sandbox, &repo);

    let page = page(&sandbox, &repo, &id, &[]);
    assert_self_contained(&page);
    assert_eq!(page.matches("<svg").count(), 1, "page:\n{page}");
}

#[test]
fn record_text_is_escaped() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    let record = json!({
        "entities": [{"name": "<b>Bold</b>", "description": "a & b", "fields": []}]
    });
    let id = submit_id(
        &sandbox,
        &repo,
        "data-model",
        "escaped",
        &record.to_string(),
    );

    let page = page(&sandbox, &repo, &id, &[]);
    assert!(page.contains("&lt;b&gt;Bold&lt;/b&gt;"), "page:\n{page}");
    assert!(page.contains("a &amp; b"), "page:\n{page}");
    assert!(!page.contains("<b>Bold</b>"), "page:\n{page}");
}

#[test]
fn nullable_fields_show_a_marker() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    let record = json!({
        "entities": [{
            "name": "Order",
            "fields": [
                {"name": "id", "type": "uuid"},
                {"name": "note", "type": "text", "nullable": true}
            ]
        }]
    });
    let id = submit_id(
        &sandbox,
        &repo,
        "data-model",
        "nullable",
        &record.to_string(),
    );

    let html = Html::parse_document(&page(&sandbox, &repo, &id, &[]));
    let fields: Vec<String> = select(&html, r#"[data-kind="field"]"#)
        .iter()
        .map(text)
        .collect();
    assert_eq!(fields, ["id: uuid", "note: text?"]);
}

#[test]
fn older_version_renders_the_older_record() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    let first = json!({"entities": [{"name": "Invoice"}]});
    let id = submit_id(&sandbox, &repo, "data-model", "billing", &first.to_string());
    let second = json!({"entities": [{"name": "Bill"}]});
    assert_eq!(
        submit_id(
            &sandbox,
            &repo,
            "data-model",
            "billing",
            &second.to_string()
        ),
        id
    );

    let older = page(&sandbox, &repo, &id, &["--version", "1"]);
    assert!(older.contains("Invoice"), "page:\n{older}");
    assert!(!older.contains("Bill"), "page:\n{older}");
    let newest = page(&sandbox, &repo, &id, &[]);
    assert!(newest.contains("Bill"), "page:\n{newest}");
    assert!(!newest.contains("Invoice"), "page:\n{newest}");
}

#[test]
fn render_to_stdout_writes_the_page_alone() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    let id = submit_example(&sandbox, &repo);
    render(&sandbox, &repo, &id, &[], "a.html");

    let assert = sandbox
        .cmd_in(&repo)
        .args(["design", "render", &id, "--out", "-"])
        .assert()
        .success();
    let file = std::fs::read(repo.join("a.html")).unwrap();
    assert_eq!(assert.get_output().stdout, file);
    assert!(!repo.join("-").exists());
}

#[test]
fn layout_is_a_grid() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    let names = ["A", "B", "C", "D", "E"];
    let entities: Vec<Value> = names.iter().map(|name| json!({"name": name})).collect();
    let record = json!({"entities": entities});
    let id = submit_id(&sandbox, &repo, "data-model", "grid", &record.to_string());

    let html = Html::parse_document(&page(&sandbox, &repo, &id, &[]));
    let ys: Vec<f64> = select(&html, r#"[data-kind="entity"] > rect"#)
        .iter()
        .map(|rect| rect.value().attr("y").unwrap().parse().unwrap())
        .collect();
    assert_eq!(ys.len(), 5);
    assert_eq!(ys[0], ys[1]);
    assert_eq!(ys[1], ys[2]);
    assert!(ys[3] > ys[2], "{ys:?}");
    assert_eq!(ys[3], ys[4]);
}

#[test]
fn unknown_design_or_version_is_not_found() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    let id = submit_example(&sandbox, &repo);

    for args in [
        &["design", "render", "dnope00000000", "--out", "a.html"][..],
        &["design", "render", &id, "--version", "2", "--out", "a.html"],
    ] {
        let (code, error) = sandbox.json_with_code(&repo, args);
        assert_eq!(code, 1, "{args:?} stdout:\n{error:#}");
        assert_eq!(error["error"]["code"], "not-found");
        assert!(!error["help"].as_array().unwrap().is_empty());
    }
    assert!(!repo.join("a.html").exists());
}

#[test]
fn interface_operations_render_as_rows_with_ids() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    let id = submit_id(&sandbox, &repo, "interface", "orders-api", INTERFACE_TOON);
    let record = sandbox.json(&repo, &["design", "get", &id])["record"].clone();
    let operation_ids: Vec<&str> = record["operations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|operation| operation["id"].as_str().unwrap())
        .collect();

    let html = Html::parse_document(&page(&sandbox, &repo, &id, &[]));
    assert_eq!(select(&html, "table").len(), 1);
    let columns: Vec<String> = select(&html, "th").iter().map(text).collect();
    assert_eq!(
        columns,
        ["Operation", "Description", "Inputs", "Outputs", "Errors"]
    );
    let rows = select(&html, r#"tr[data-kind="operation"]"#);
    assert_eq!(rows.iter().map(data_id).collect::<Vec<_>>(), operation_ids);
    for row in &rows {
        assert_eq!(select_in(row, "td").len(), 5);
    }
}

#[test]
fn interface_inputs_outputs_and_errors_carry_ids() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    let id = submit_id(&sandbox, &repo, "interface", "orders-api", INTERFACE_TOON);
    let record = sandbox.json(&repo, &["design", "get", &id])["record"].clone();
    let create = &record["operations"][0];
    assert_eq!(create["name"], "createOrder");

    let html = Html::parse_document(&page(&sandbox, &repo, &id, &[]));
    let row = select(
        &html,
        &format!(r#"tr[data-id="{}"]"#, create["id"].as_str().unwrap()),
    );
    assert_eq!(row.len(), 1);
    for (kind, key) in [
        ("input", "inputs"),
        ("output", "outputs"),
        ("error", "errors"),
    ] {
        let items = select_in(&row[0], &format!(r#"li[data-kind="{kind}"]"#));
        assert_eq!(items.len(), 1, "{kind} items");
        assert_eq!(data_id(&items[0]), create[key][0]["id"].as_str().unwrap());
    }
    let error = select_in(&row[0], r#"li[data-kind="error"]"#);
    assert_eq!(
        text(&error[0]),
        "CustomerNotFound: no customer has customer_id"
    );
}

#[test]
fn interface_render_is_deterministic_and_self_contained() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    let id = submit_id(&sandbox, &repo, "interface", "orders-api", INTERFACE_TOON);

    let assert = sandbox
        .cmd_in(&repo)
        .args(["design", "render", &id, "--out", "a.html"])
        .assert()
        .success();
    let toon = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let a = std::fs::read(repo.join("a.html")).unwrap();
    assert_eq!(
        toon,
        format!(
            "rendered:\n  design: {id}\n  version: 1\n  path: {}\n  bytes: {}\n",
            repo.join("a.html").display(),
            a.len()
        )
    );
    render(&sandbox, &repo, &id, &[], "b.html");
    assert_eq!(a, std::fs::read(repo.join("b.html")).unwrap());

    let page = String::from_utf8(a).unwrap();
    assert_self_contained(&page);
    assert_eq!(page.matches("<table").count(), 1, "page:\n{page}");
}

#[test]
fn empty_interface_cells_say_none() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    let record = json!({
        "operations": [{
            "name": "listOrders",
            "outputs": [{"name": "orders", "type": "Order[]", "nullable": true}]
        }]
    });
    let id = submit_id(&sandbox, &repo, "interface", "listing", &record.to_string());

    let html = Html::parse_document(&page(&sandbox, &repo, &id, &[]));
    let rows = select(&html, r#"tr[data-kind="operation"]"#);
    assert_eq!(rows.len(), 1);
    let cells: Vec<String> = select_in(&rows[0], "td").iter().map(text).collect();
    assert_eq!(
        cells,
        ["listOrders", "none", "none", "orders: Order[]?", "none"]
    );
}
