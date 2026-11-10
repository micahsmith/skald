mod common;

use assert_cmd::assert::OutputAssertExt;
use common::Sandbox;
use serde_json::Value;

fn usage_error(args: &[&str]) -> Value {
    let output = Sandbox::new().cmd().args(args).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(output.status.code(), Some(2), "stdout:\n{stdout}");
    assert!(output.stderr.is_empty(), "stderr is not empty");
    if args.contains(&"--json") {
        serde_json::from_str(&stdout).unwrap()
    } else {
        assert!(stdout.starts_with("error:"), "stdout:\n{stdout}");
        toon_format::decode_default(&stdout).unwrap()
    }
}

#[test]
fn version_prints_bare_version() {
    let sandbox = Sandbox::new();
    for flag in ["--version", "-V", "-v"] {
        sandbox
            .cmd()
            .arg(flag)
            .assert()
            .success()
            .stdout(concat!(env!("CARGO_PKG_VERSION"), "\n"));
    }
}

#[test]
fn help_lists_every_command() {
    let assert = Sandbox::new().cmd().arg("--help").assert().success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    for text in [
        "setup",
        "hook",
        "design submit",
        "design list",
        "design get",
        "design diff",
        "design render",
        "design open",
        "exit codes",
    ] {
        assert!(stdout.contains(text), "{text:?} missing from:\n{stdout}");
    }
}

#[test]
fn test_only_seed_command_is_hidden() {
    let sandbox = Sandbox::new();
    let assert = sandbox.cmd().arg("--help").assert().success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(!stdout.contains("dev"), "help lists dev:\n{stdout}");

    let value = usage_error(&["frobnicate", "--json"]);
    let help = value["help"].to_string();
    assert!(!help.contains("dev"), "help: {help}");

    let output = sandbox
        .cmd()
        .args(["dev", "seed", "--help"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
}

#[test]
fn unknown_flag_is_a_usage_error_on_stdout() {
    let value = usage_error(&["design", "list", "--bogus"]);
    assert_eq!(value["error"]["code"], "unknown-flag");
    let message = value["error"]["message"].as_str().unwrap();
    assert!(message.contains("--bogus"), "message: {message}");
    let help = value["help"].to_string();
    assert!(help.contains("--limit"), "help: {help}");
}

#[test]
fn unknown_flag_in_json_mode() {
    let value = usage_error(&["design", "list", "--bogus", "--json"]);
    assert_eq!(value["error"]["code"], "unknown-flag");
}

#[test]
fn unknown_command() {
    let value = usage_error(&["frobnicate"]);
    assert_eq!(value["error"]["code"], "unknown-command");
}

#[test]
fn command_help_describes_flags_and_root_help_hides_serve() {
    let sandbox = Sandbox::new();
    let assert = sandbox
        .cmd()
        .args(["design", "submit", "--help"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    for flag in ["--kind", "--name", "--file", "--json"] {
        let mut lines = stdout.lines();
        let line = lines
            .find(|line| line.trim_start().starts_with(flag))
            .unwrap_or_else(|| panic!("{flag} missing from:\n{stdout}"));
        let description = lines.next().unwrap_or_default();
        assert!(
            line.trim_start().starts_with(flag) && !description.trim().is_empty(),
            "{flag} has no description in:\n{stdout}"
        );
    }

    let assert = sandbox.cmd().arg("--help").assert().success();
    let root = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(!root.contains("serve"), "root help lists serve:\n{root}");
}
