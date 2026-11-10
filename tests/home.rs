mod common;

use std::collections::BTreeMap;

use assert_cmd::assert::OutputAssertExt;
use common::{ORIGIN, Sandbox};
use rusqlite::Connection;
use serde_json::Value;

fn repository_rows(sandbox: &Sandbox) -> BTreeMap<String, String> {
    let conn = Connection::open(sandbox.store_path()).unwrap();
    let mut statement = conn
        .prepare("SELECT identity, default_branch FROM repository")
        .unwrap();
    statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

#[test]
fn identity_is_the_normalized_origin_url() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);

    let home = sandbox.json(&repo, &[]);
    assert_eq!(home["repository"]["identity"], "github.com/acme/orders");
}

#[test]
fn identity_omits_credentials_from_the_origin_url() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin("https://alice:ghp_SECRET@github.com/acme/orders.git");

    let home = sandbox.json(&repo, &[]);
    assert_eq!(home["repository"]["identity"], "github.com/acme/orders");
    let list = sandbox
        .cmd_in(&repo)
        .args(["design", "list"])
        .output()
        .unwrap();
    let list = String::from_utf8_lossy(&list.stdout).into_owned();
    assert!(list.contains("github.com/acme/orders"), "{list}");
    assert!(!list.contains("ghp_SECRET"), "{list}");
    assert!(!home.to_string().contains("ghp_SECRET"), "{home}");
    let rows = repository_rows(&sandbox);
    assert_eq!(rows.len(), 1);
    assert!(rows.contains_key("github.com/acme/orders"), "{rows:?}");
}

#[test]
fn identity_is_the_root_path_without_origin() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_without_origin();
    let subdirectory = repo.join("src/nested");
    std::fs::create_dir_all(&subdirectory).unwrap();

    let home = sandbox.json(&subdirectory, &[]);
    assert_eq!(home["repository"]["identity"], repo.to_str().unwrap());
}

#[test]
fn worktrees_share_identity() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_without_origin();
    let worktree = sandbox.worktree(&repo, "feature-x");

    let main = sandbox.json(&repo, &[]);
    let feature = sandbox.json(&worktree, &[]);
    assert_eq!(main["repository"]["identity"], repo.to_str().unwrap());
    assert_eq!(
        main["repository"]["identity"],
        feature["repository"]["identity"]
    );
    assert_eq!(main["repository"]["branch"], "main");
    assert_eq!(feature["repository"]["branch"], "feature-x");
    assert_eq!(repository_rows(&sandbox).len(), 1);
}

#[test]
fn branch_is_the_checked_out_branch() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    sandbox.checkout_branch(&repo, "feature-x");

    let home = sandbox.json(&repo, &[]);
    assert_eq!(home["repository"]["branch"], "feature-x");
}

#[test]
fn detached_head_is_reported_as_detached() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    sandbox.git(&repo, &["checkout", "--quiet", "--detach"]);

    let home = sandbox.json(&repo, &[]);
    assert_eq!(home["repository"]["branch"], "detached");
}

#[test]
fn branch_without_commits_is_named() {
    let sandbox = Sandbox::new();
    let repo = sandbox.root.join("unborn");
    std::fs::create_dir_all(&repo).unwrap();
    sandbox.git(&repo, &["init", "--quiet", "--initial-branch=trunk"]);

    let home = sandbox.json(&repo, &[]);
    assert_eq!(home["repository"]["branch"], "trunk");
}

#[test]
fn default_branch_is_stored_on_first_sight() {
    let sandbox = Sandbox::new();
    let tracked = sandbox.repo_with_origin("https://github.com/acme/tracked.git");
    sandbox.git(
        &tracked,
        &["update-ref", "refs/remotes/origin/trunk", "HEAD"],
    );
    sandbox.git(
        &tracked,
        &[
            "symbolic-ref",
            "refs/remotes/origin/HEAD",
            "refs/remotes/origin/trunk",
        ],
    );
    let untracked = sandbox.repo_with_origin("https://github.com/acme/untracked.git");
    let legacy = sandbox.repo_with_origin("https://github.com/acme/legacy.git");
    sandbox.git(&legacy, &["branch", "--move", "main", "master"]);

    for repo in [&tracked, &untracked, &legacy] {
        sandbox.cmd_in(repo).assert().success();
    }
    sandbox.git(
        &tracked,
        &["update-ref", "refs/remotes/origin/main", "HEAD"],
    );
    sandbox.git(
        &tracked,
        &[
            "symbolic-ref",
            "refs/remotes/origin/HEAD",
            "refs/remotes/origin/main",
        ],
    );
    sandbox.cmd_in(&tracked).assert().success();

    let rows = repository_rows(&sandbox);
    assert_eq!(rows["github.com/acme/tracked"], "trunk");
    assert_eq!(rows["github.com/acme/untracked"], "main");
    assert_eq!(rows["github.com/acme/legacy"], "master");
}

#[test]
fn home_view_outside_a_repository() {
    let sandbox = Sandbox::new();

    let home = sandbox.json(&sandbox.work, &[]);
    assert_eq!(home["repository"], Value::Null);
    let note = home["note"].as_str().unwrap();
    assert!(note.contains("no git repository"), "note: {note}");
    assert!(!home["help"].as_array().unwrap().is_empty());
}

#[test]
fn home_view_empty_state_and_key_order() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);

    let assert = sandbox.cmd_in(&repo).assert().success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert!(stdout.starts_with("bin: "), "stdout:\n{stdout}");
    // TOON spec v4.4, which toon-format 0.7 follows, writes an empty array as `key: []`.
    for text in ["\ndesigns: []\n", "\ndesigns_count: 0\n", "\nhelp["] {
        assert!(stdout.contains(text), "{text:?} missing from:\n{stdout}");
    }

    let home = sandbox.json(&repo, &[]);
    let keys: Vec<&String> = home.as_object().unwrap().keys().collect();
    assert_eq!(
        keys,
        [
            "bin",
            "about",
            "version",
            "repository",
            "designs",
            "designs_count",
            "adapters",
            "plugin",
            "help"
        ]
    );
    assert_eq!(
        home["about"],
        "skald keeps design intent for a repository outside its working tree, keyed to its git \
         history"
    );
    assert_eq!(home["version"], env!("CARGO_PKG_VERSION"));
    let help: Vec<&str> = home["help"]
        .as_array()
        .unwrap()
        .iter()
        .map(|line| line.as_str().unwrap())
        .collect();
    assert!(help.contains(&"skald design submit --kind data-model --name <name> --file <path>"));
    assert!(help.contains(&"skald setup"));
}

#[test]
fn toon_and_json_carry_the_same_content() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);

    let assert = sandbox.cmd_in(&repo).assert().success();
    let toon = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let from_toon: Value = toon_format::decode_default(&toon).unwrap();
    assert_eq!(from_toon, sandbox.json(&repo, &[]));
}
