mod common;

use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use assert_cmd::assert::OutputAssertExt;
use common::{ORIGIN, Sandbox, keys};
use rusqlite::Connection;
use serde_json::{Value, json};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const MARKETPLACE_ADD: &str =
    "plugin marketplace add https://github.com/micahsmith/skald.git --scope user";
const PLUGIN_INSTALL: &str = "plugin install code-skillset@skald --scope user";

fn settings_path(sandbox: &Sandbox) -> PathBuf {
    sandbox.config.join("settings.json")
}

fn marker_path(sandbox: &Sandbox) -> PathBuf {
    sandbox.data.join("adapters/claude-code.json")
}

fn plugins_dir(sandbox: &Sandbox) -> PathBuf {
    sandbox.config.join("plugins")
}

fn claude_log(sandbox: &Sandbox) -> PathBuf {
    sandbox.root.join("claude-calls.log")
}

fn read_json(path: &Path) -> Value {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("cannot read {}: {err}", path.display()));
    serde_json::from_str(&text).unwrap()
}

fn write_json(path: &Path, value: &Value) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, value.to_string()).unwrap();
}

fn help(value: &Value) -> Vec<&str> {
    value["help"]
        .as_array()
        .unwrap()
        .iter()
        .map(|line| line.as_str().unwrap())
        .collect()
}

/// Every SessionStart hook command that names skald's Claude Code hook.
fn skald_commands(settings: &Value) -> Vec<String> {
    settings["hooks"]["SessionStart"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|entry| entry["hooks"].as_array().unwrap())
        .filter_map(|hook| hook["command"].as_str())
        .filter(|command| command.ends_with(" hook claude-code"))
        .map(String::from)
        .collect()
}

fn step<'a>(setup: &'a Value, name: &str) -> &'a Value {
    setup["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|step| step["step"] == name)
        .unwrap_or_else(|| panic!("no {name} step in:\n{setup:#}"))
}

fn installed_plugins(key: &str, install_path: &Path) -> Value {
    json!({
        "version": 2,
        "plugins": {key: [{"scope": "user", "installPath": install_path, "version": "0.1.0"}]}
    })
}

/// Writes the plugin's `plugin.json` into a plugin cache directory and returns that directory.
fn plugin_cache(sandbox: &Sandbox, key: &str, min_version: Option<&str>) -> PathBuf {
    let install_path = sandbox.root.join("plugin-cache").join(key);
    let mut manifest = json!({"name": "code-skillset"});
    if let Some(min_version) = min_version {
        manifest["metadata"] = json!({"skaldMinVersion": min_version});
    }
    write_json(&install_path.join(".claude-plugin/plugin.json"), &manifest);
    install_path
}

/// Records plugin `key` as installed in `plugins`, with `min_version` as its minimum skald version.
fn install_plugin(sandbox: &Sandbox, plugins: &Path, key: &str, min_version: Option<&str>) {
    let install_path = plugin_cache(sandbox, key, min_version);
    write_json(
        &plugins.join("installed_plugins.json"),
        &installed_plugins(key, &install_path),
    );
}

fn write_claude(sandbox: &Sandbox, body: &str) {
    let path = sandbox.bin.join("claude");
    std::fs::write(&path, format!("#!/usr/bin/env bash\n{body}")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// A fake `claude` that logs its arguments and, on `plugin install`, records code-skillset@skald
/// the way Claude Code does.
fn fake_claude(sandbox: &Sandbox) {
    let install_path = plugin_cache(sandbox, "code-skillset@skald", Some(VERSION));
    let record = installed_plugins("code-skillset@skald", &install_path);
    write_claude(
        sandbox,
        &format!(
            "printf '%s\\n' \"$*\" >> '{log}'\n\
             if [ \"$1 $2\" = 'plugin install' ]; then\n\
               mkdir -p '{plugins}'\n\
               printf '%s' '{record}' > '{plugins}/installed_plugins.json'\n\
             fi\n",
            log = claude_log(sandbox).display(),
            plugins = plugins_dir(sandbox).display(),
        ),
    );
}

fn failing_claude(sandbox: &Sandbox) {
    write_claude(
        sandbox,
        &format!(
            "printf '%s\\n' \"$*\" >> '{log}'\necho boom >&2\nexit 1\n",
            log = claude_log(sandbox).display()
        ),
    );
}

fn claude_calls(sandbox: &Sandbox) -> Vec<String> {
    std::fs::read_to_string(claude_log(sandbox))
        .unwrap()
        .lines()
        .map(String::from)
        .collect()
}

/// The contents and modification time of every file under `dirs`.
fn snapshot(dirs: &[&Path]) -> BTreeMap<PathBuf, (Vec<u8>, SystemTime)> {
    let mut files = BTreeMap::new();
    let mut pending: Vec<PathBuf> = dirs.iter().map(|dir| dir.to_path_buf()).collect();
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
                files.insert(path.clone(), (std::fs::read(&path).unwrap(), modified));
            }
        }
    }
    files
}

fn submit_design(sandbox: &Sandbox, repo: &Path, name: &str) {
    let model = sandbox.file(
        "model.json",
        r#"{"entities": [{"name": "Order", "fields": [{"name": "id", "type": "uuid"}]}]}"#,
    );
    sandbox.json(
        repo,
        &[
            "design",
            "submit",
            "--kind",
            "data-model",
            "--name",
            name,
            "--file",
            model.to_str().unwrap(),
        ],
    );
}

#[test]
fn setup_installs_the_hook() {
    let sandbox = Sandbox::new();

    let setup = sandbox.json(&sandbox.work, &["setup", "--no-plugin"]);
    assert_eq!(keys(&setup), ["setup", "steps", "help"]);
    assert_eq!(setup["setup"], json!({"runtime": "claude-code"}));
    let steps: Vec<&Value> = setup["steps"].as_array().unwrap().iter().collect();
    assert_eq!(
        steps.iter().map(|step| keys(step)).collect::<Vec<_>>(),
        [["step", "status", "detail"]; 3]
    );
    assert_eq!(step(&setup, "hook")["status"], "installed");
    assert_eq!(step(&setup, "marker")["status"], "installed");
    assert_eq!(step(&setup, "plugin")["status"], "skipped");

    let settings = read_json(&settings_path(&sandbox));
    let entries = settings["hooks"]["SessionStart"].as_array().unwrap();
    assert_eq!(entries.len(), 1);
    let commands = skald_commands(&settings);
    assert_eq!(commands.len(), 1);
    let exe = Path::new(env!("CARGO_BIN_EXE_skald"))
        .canonicalize()
        .unwrap();
    assert!(
        commands[0].contains(exe.to_str().unwrap()),
        "command: {}",
        commands[0]
    );
    assert_eq!(
        entries[0],
        json!({"hooks": [{"type": "command", "command": commands[0], "timeout": 10}]})
    );

    let marker = read_json(&marker_path(&sandbox));
    assert_eq!(
        keys(&marker),
        ["runtime", "version", "command", "settings", "installed_at"]
    );
    assert_eq!(marker["runtime"], "claude-code");
    assert_eq!(marker["version"], "0.1.0");
    assert_eq!(marker["command"], commands[0]);
    assert_eq!(
        marker["settings"],
        settings_path(&sandbox).to_str().unwrap()
    );
    let installed_at = marker["installed_at"].as_str().unwrap();
    assert!(
        installed_at.len() == 20 && installed_at.ends_with('Z'),
        "installed_at: {installed_at}"
    );
}

#[test]
fn hook_prints_the_home_view() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    submit_design(&sandbox, &repo, "order-intake");
    sandbox.json(&sandbox.work, &["setup", "--no-plugin"]);
    let command = skald_commands(&read_json(&settings_path(&sandbox))).remove(0);

    let output = sandbox.bash(&repo, &command);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success(), "stdout:\n{stdout}");
    assert!(stdout.starts_with("bin: "), "stdout:\n{stdout}");
    assert!(stdout.contains("order-intake"), "stdout:\n{stdout}");
}

#[test]
fn setup_is_idempotent() {
    let sandbox = Sandbox::new();
    sandbox.json(&sandbox.work, &["setup", "--no-plugin"]);
    let dirs = [sandbox.config.as_path(), sandbox.data.as_path()];
    let before = snapshot(&dirs);
    assert!(before.contains_key(&settings_path(&sandbox)));
    assert!(before.contains_key(&marker_path(&sandbox)));

    let setup = sandbox.json(&sandbox.work, &["setup", "--no-plugin"]);
    for step in setup["steps"].as_array().unwrap() {
        assert!(
            step["status"] == "unchanged" || step["status"] == "skipped",
            "step: {step}"
        );
    }
    assert_eq!(snapshot(&dirs), before);
}

#[test]
fn settings_file_is_private() {
    let sandbox = Sandbox::new();
    let mode = |sandbox: &Sandbox| {
        std::fs::metadata(settings_path(sandbox))
            .unwrap()
            .permissions()
            .mode()
            & 0o777
    };

    sandbox.json(&sandbox.work, &["setup", "--no-plugin"]);
    assert_eq!(mode(&sandbox), 0o600);

    write_json(&settings_path(&sandbox), &json!({"model": "opus"}));
    std::fs::set_permissions(settings_path(&sandbox), PermissionsExt::from_mode(0o600)).unwrap();
    sandbox.json(&sandbox.work, &["setup", "--no-plugin"]);
    assert_eq!(mode(&sandbox), 0o600);
}

#[test]
fn existing_settings_are_preserved() {
    let sandbox = Sandbox::new();
    let pre_tool_use = json!([
        {"matcher": "Bash", "hooks": [{"type": "command", "command": "echo checked"}]}
    ]);
    write_json(
        &settings_path(&sandbox),
        &json!({"model": "opus", "hooks": {"PreToolUse": pre_tool_use}}),
    );

    let setup = sandbox.json(&sandbox.work, &["setup", "--no-plugin"]);
    assert_eq!(step(&setup, "hook")["status"], "installed");
    let settings = read_json(&settings_path(&sandbox));
    assert_eq!(keys(&settings), ["model", "hooks"]);
    assert_eq!(settings["model"], "opus");
    assert_eq!(keys(&settings["hooks"]), ["PreToolUse", "SessionStart"]);
    assert_eq!(settings["hooks"]["PreToolUse"], pre_tool_use);
    assert_eq!(skald_commands(&settings).len(), 1);
}

#[test]
fn stale_hook_path_is_repaired() {
    let sandbox = Sandbox::new();
    let other = json!({"hooks": [{"type": "command", "command": "echo hello"}]});
    let stale = json!({
        "hooks": [{"type": "command", "command": "/old/path/skald hook claude-code", "timeout": 10}]
    });
    write_json(
        &settings_path(&sandbox),
        &json!({"hooks": {"SessionStart": [other, stale]}}),
    );

    let setup = sandbox.json(&sandbox.work, &["setup", "--no-plugin"]);
    assert_eq!(step(&setup, "hook")["status"], "repaired");
    let settings = read_json(&settings_path(&sandbox));
    let commands = skald_commands(&settings);
    assert_eq!(commands.len(), 1);
    assert!(!commands[0].starts_with("/old/path/"), "{}", commands[0]);
    assert_eq!(commands[0], read_json(&marker_path(&sandbox))["command"]);
    let entries = settings["hooks"]["SessionStart"].as_array().unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0], other);
}

#[test]
fn hooks_sharing_an_entry_with_skald_survive_setup() {
    let sandbox = Sandbox::new();
    let load_env = json!({"type": "command", "command": "~/bin/load-env.sh"});
    let shared = json!({
        "hooks": [load_env, {"type": "command", "command": "/old/skald hook claude-code"}]
    });
    write_json(
        &settings_path(&sandbox),
        &json!({"hooks": {"SessionStart": [shared]}}),
    );

    let setup = sandbox.json(&sandbox.work, &["setup", "--no-plugin"]);
    assert_eq!(step(&setup, "hook")["status"], "repaired");
    let settings = read_json(&settings_path(&sandbox));
    assert_eq!(skald_commands(&settings).len(), 1);
    let entries = settings["hooks"]["SessionStart"].as_array().unwrap();
    assert!(
        entries
            .iter()
            .any(|entry| entry["hooks"] == json!([load_env])),
        "{settings:#}"
    );

    let setup = sandbox.json(&sandbox.work, &["setup", "--no-plugin"]);
    assert_eq!(step(&setup, "hook")["status"], "unchanged");
    assert_eq!(read_json(&settings_path(&sandbox)), settings);
}

#[test]
fn plugin_is_installed_when_absent() {
    let sandbox = Sandbox::new();
    fake_claude(&sandbox);

    let setup = sandbox.json(&sandbox.work, &["setup"]);
    assert_eq!(claude_calls(&sandbox), [MARKETPLACE_ADD, PLUGIN_INSTALL]);
    assert_eq!(step(&setup, "plugin")["status"], "installed");

    let setup = sandbox.json(&sandbox.work, &["setup"]);
    assert_eq!(step(&setup, "plugin")["status"], "unchanged");
    assert_eq!(claude_calls(&sandbox).len(), 2);

    let home = sandbox.json(&sandbox.work, &[]);
    assert_eq!(home["adapters"][0]["status"], "current");
    assert_eq!(home["plugin"]["id"], "code-skillset@skald");
    assert_eq!(home["plugin"]["status"], "current");
    assert!(!help(&home).contains(&"skald setup"), "home:\n{home:#}");
}

#[test]
fn plugin_is_left_alone_when_present() {
    let sandbox = Sandbox::new();
    install_plugin(
        &sandbox,
        &plugins_dir(&sandbox),
        "code-skillset@cc-loadout",
        Some(VERSION),
    );
    fake_claude(&sandbox);

    let setup = sandbox.json(&sandbox.work, &["setup"]);
    assert!(!claude_log(&sandbox).exists());
    assert_eq!(step(&setup, "plugin")["status"], "unchanged");
}

#[test]
fn missing_claude_executable_is_reported_not_fatal() {
    let sandbox = Sandbox::new();
    let empty = sandbox.root.join("no-claude");
    std::fs::create_dir(&empty).unwrap();
    let git = std::env::split_paths(&std::env::var_os("PATH").unwrap())
        .map(|dir| dir.join("git"))
        .find(|path| path.is_file())
        .expect("git on PATH");
    std::os::unix::fs::symlink(git, empty.join("git")).unwrap();

    let output = sandbox
        .cmd()
        .env("PATH", &empty)
        .args(["setup", "--json"])
        .output()
        .unwrap();
    let setup: Value = serde_json::from_slice(&output.stdout).unwrap();
    let plugin = step(&setup, "plugin");
    assert_eq!(plugin["status"], "skipped");
    let detail = plugin["detail"].as_str().unwrap();
    assert!(detail.contains("claude executable not found"), "{detail}");
    assert_eq!(step(&setup, "hook")["status"], "installed");
}

#[test]
fn failed_plugin_install_exits_1() {
    let sandbox = Sandbox::new();
    failing_claude(&sandbox);

    let (code, setup) = sandbox.json_with_code(&sandbox.work, &["setup"]);
    assert_eq!(code, 1, "stdout:\n{setup:#}");
    let plugin = step(&setup, "plugin");
    assert_eq!(plugin["status"], "failed");
    let detail = plugin["detail"].as_str().unwrap();
    assert!(detail.contains("boom"), "{detail}");
    assert_eq!(step(&setup, "hook")["status"], "installed");
    assert!(help(&setup).contains(&"skald setup"));
}

#[test]
fn outdated_adapter_is_reported() {
    let sandbox = Sandbox::new();
    write_json(
        &marker_path(&sandbox),
        &json!({
            "runtime": "claude-code",
            "version": "0.0.1",
            "command": "skald hook claude-code",
            "settings": settings_path(&sandbox),
            "installed_at": "2026-01-01T00:00:00Z"
        }),
    );

    let home = sandbox.json(&sandbox.work, &[]);
    assert_eq!(
        home["adapters"],
        json!([{"runtime": "claude-code", "version": "0.0.1", "status": "outdated"}])
    );
    assert!(help(&home).contains(&"skald setup"), "home:\n{home:#}");
}

#[test]
fn current_adapter_is_reported() {
    let sandbox = Sandbox::new();
    sandbox.json(&sandbox.work, &["setup", "--no-plugin"]);

    let home = sandbox.json(&sandbox.work, &[]);
    assert_eq!(
        home["adapters"],
        json!([{"runtime": "claude-code", "version": VERSION, "status": "current"}])
    );
}

#[test]
fn plugin_minimum_version_mismatch_is_reported() {
    let sandbox = Sandbox::new();
    install_plugin(
        &sandbox,
        &plugins_dir(&sandbox),
        "code-skillset@skald",
        Some("9.0.0"),
    );

    let home = sandbox.json(&sandbox.work, &[]);
    let plugin = &home["plugin"];
    assert_eq!(plugin["id"], "code-skillset@skald");
    assert_eq!(plugin["min_skald_version"], "9.0.0");
    assert_eq!(plugin["status"], "outdated");
    let detail = plugin["detail"].as_str().unwrap();
    assert!(
        detail.contains("9.0.0") && detail.contains("0.1.0"),
        "{detail}"
    );
    assert!(
        help(&home)
            .iter()
            .any(|line| line.starts_with("cargo install")),
        "home:\n{home:#}"
    );
}

#[test]
fn plugin_without_a_minimum_is_unknown() {
    let sandbox = Sandbox::new();
    install_plugin(
        &sandbox,
        &plugins_dir(&sandbox),
        "code-skillset@skald",
        None,
    );

    let home = sandbox.json(&sandbox.work, &[]);
    assert_eq!(home["plugin"]["status"], "unknown");
    assert_eq!(home["plugin"]["min_skald_version"], Value::Null);
}

#[test]
fn plugin_without_a_minimum_gets_an_update_command() {
    let sandbox = Sandbox::new();
    install_plugin(
        &sandbox,
        &plugins_dir(&sandbox),
        "code-skillset@cc-loadout",
        None,
    );

    let home = sandbox.json(&sandbox.work, &[]);
    assert_eq!(home["plugin"]["status"], "unknown");
    let update = "claude plugin marketplace update cc-loadout && claude plugin update code-skillset@cc-loadout";
    assert!(help(&home).contains(&update), "home:\n{home:#}");
}

#[test]
fn plugin_with_a_minimum_gets_no_update_command() {
    for min in [VERSION, "9.0.0"] {
        let sandbox = Sandbox::new();
        install_plugin(
            &sandbox,
            &plugins_dir(&sandbox),
            "code-skillset@cc-loadout",
            Some(min),
        );

        let home = sandbox.json(&sandbox.work, &[]);
        assert!(
            help(&home)
                .iter()
                .all(|line| !line.starts_with("claude plugin")),
            "min {min}, home:\n{home:#}"
        );
    }
}

#[test]
fn missing_plugin_is_reported() {
    let sandbox = Sandbox::new();

    let home = sandbox.json(&sandbox.work, &[]);
    assert_eq!(home["adapters"], json!([]));
    assert_eq!(home["plugin"]["status"], "missing");
    assert!(help(&home).contains(&"skald setup"), "home:\n{home:#}");
}

#[test]
fn plugin_cache_directory_can_be_overridden() {
    let sandbox = Sandbox::new();
    let cache = sandbox.root.join("plugin-root");
    install_plugin(&sandbox, &cache, "code-skillset@skald", Some(VERSION));

    let output = sandbox
        .cmd()
        .args(["--json"])
        .env("CLAUDE_CODE_PLUGIN_CACHE_DIR", &cache)
        .output()
        .unwrap();
    let home: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(home["plugin"]["status"], "current");
}

#[test]
fn hook_outside_a_repository_still_exits_0() {
    let sandbox = Sandbox::new();

    let assert = sandbox
        .cmd()
        .args(["hook", "claude-code"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("no git repository"), "stdout:\n{stdout}");
}

#[test]
fn hook_with_an_unreadable_store_prints_one_line() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    std::fs::create_dir_all(&sandbox.data).unwrap();
    Connection::open(sandbox.store_path())
        .unwrap()
        .pragma_update(None, "user_version", 999)
        .unwrap();

    let assert = sandbox
        .cmd_in(&repo)
        .args(["hook", "claude-code"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert_eq!(stdout.lines().count(), 1, "stdout:\n{stdout}");
    assert!(stdout.contains("upgrade skald"), "stdout:\n{stdout}");
}

#[test]
fn sandbox_path_keeps_outer_tools_and_stubs_claude() {
    let sandbox = Sandbox::new();
    let outer = sandbox.root.join("outer");
    std::fs::create_dir(&outer).unwrap();
    for name in ["claude", "marker"] {
        let path = outer.join(name);
        std::fs::write(&path, format!("#!/bin/sh\necho outer-{name}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let real = std::env::var_os("PATH").unwrap();
    let dirs = std::iter::once(outer.clone()).chain(std::env::split_paths(&real));
    let path = sandbox.path_over(&std::env::join_paths(dirs).unwrap());

    let marker = sandbox
        .command("sh", &sandbox.work)
        .env("PATH", &path)
        .args(["-c", "marker"])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&marker.stdout), "outer-marker\n");

    let claude = sandbox
        .command("sh", &sandbox.work)
        .env("PATH", &path)
        .args(["-c", "command -v claude"])
        .output()
        .unwrap();
    let found = String::from_utf8(claude.stdout).unwrap();
    assert_eq!(found.trim(), sandbox.bin.join("claude").to_str().unwrap());
    let run = sandbox
        .command(sandbox.bin.join("claude"), &sandbox.work)
        .output()
        .unwrap();
    assert!(!run.status.success());
}

#[test]
fn setup_respects_claude_config_dir() {
    let sandbox = Sandbox::new();
    let config = sandbox.root.join("cfg");

    sandbox
        .cmd()
        .args(["setup", "--no-plugin", "--json"])
        .env("CLAUDE_CONFIG_DIR", &config)
        .assert()
        .success();
    assert!(config.join("settings.json").exists());
    assert!(!sandbox.home.join(".claude").exists());
}

#[test]
fn hook_command_is_the_absolute_path_that_path_finds_for_this_skald() {
    let sandbox = Sandbox::new();
    let link = sandbox.bin.join("skald");
    std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_skald"), &link).unwrap();

    sandbox.json(&sandbox.work, &["setup", "--no-plugin"]);
    let settings = read_json(&settings_path(&sandbox));
    let expected = format!("{} hook claude-code", link.display());
    assert!(link.is_absolute());
    assert_eq!(skald_commands(&settings), [expected]);
}

#[test]
fn bare_name_hook_is_replaced_by_the_absolute_form() {
    let sandbox = Sandbox::new();
    let link = sandbox.bin.join("skald");
    std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_skald"), &link).unwrap();
    let bare = json!({"hooks": [{"type": "command", "command": "skald hook claude-code"}]});
    write_json(
        &settings_path(&sandbox),
        &json!({"hooks": {"SessionStart": [bare]}}),
    );

    let setup = sandbox.json(&sandbox.work, &["setup", "--no-plugin"]);
    assert_eq!(step(&setup, "hook")["status"], "repaired");
    let settings = read_json(&settings_path(&sandbox));
    assert_eq!(
        skald_commands(&settings),
        [format!("{} hook claude-code", link.display())]
    );
}

#[cfg(unix)]
#[test]
fn setup_creates_the_data_directory_private() {
    let sandbox = Sandbox::new();

    sandbox.json(&sandbox.work, &["setup", "--no-plugin"]);
    let mode = std::fs::metadata(&sandbox.data)
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o700, "mode is {:o}", mode & 0o777);
}
