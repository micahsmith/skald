use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

use serde::Serialize;
use serde_json::{Map, Value, json};

use crate::adapter::{self, Marker};
use crate::error::SkaldError;
use crate::output::Output;
use crate::paths;
use crate::store;

const RUNTIME: &str = "claude-code";
const HOOK_SUFFIX: &str = " hook claude-code";
const PLUGIN_ID: &str = "code-skillset@skald";
const MARKETPLACE_ADD: [&str; 6] = [
    "plugin",
    "marketplace",
    "add",
    "https://github.com/micahsmith/skald.git",
    "--scope",
    "user",
];
const PLUGIN_INSTALL: [&str; 5] = ["plugin", "install", PLUGIN_ID, "--scope", "user"];

#[derive(Debug, Serialize)]
struct Step {
    step: &'static str,
    status: Status,
    detail: String,
}

impl Step {
    fn new(step: &'static str, status: Status, detail: impl Into<String>) -> Self {
        Self {
            step,
            status,
            detail: detail.into(),
        }
    }

    /// A successful step reports `path`; a failed one reports why it failed.
    fn at(step: &'static str, path: &Path, result: Result<Status, String>) -> Self {
        match result {
            Ok(status) => Self::new(step, status, path.display().to_string()),
            Err(message) => Self::new(step, Status::Failed, message),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum Status {
    Installed,
    Repaired,
    Unchanged,
    Skipped,
    Failed,
}

pub fn run(no_plugin: bool) -> Result<Output, SkaldError> {
    let command = hook_command()?;
    let settings = paths::claude_config_dir()?.join("settings.json");
    let data_dir = paths::data_dir()?;
    let marker = adapter::marker_path(&data_dir, RUNTIME);

    let hook_step = Step::at("hook", &settings, install_hook(&settings, &command));
    let marker_step = if hook_step.status == Status::Failed {
        Step::new("marker", Status::Skipped, "the hook is not installed")
    } else {
        Step::at(
            "marker",
            &marker,
            paths::create_data_dir(&data_dir)
                .map_err(|err| format!("cannot create {}: {err}", data_dir.display()))
                .and_then(|()| write_marker(&marker, &command, &settings)),
        )
    };
    let plugin_step = if no_plugin {
        Step::new("plugin", Status::Skipped, "the --no-plugin flag was given")
    } else {
        install_plugin(&paths::claude_plugins_dir()?)
    };

    let steps = [hook_step, marker_step, plugin_step];
    let failed = steps.iter().any(|step| step.status == Status::Failed);
    let mut value = Map::new();
    value.insert("setup".to_owned(), json!({"runtime": RUNTIME}));
    value.insert("steps".to_owned(), json!(steps));
    let mut help = vec!["skald".to_owned()];
    if failed {
        help.push("skald setup".to_owned());
    }
    Ok(Output::new(value)
        .with_help(help)
        .with_exit_code(u8::from(failed)))
}

/// The path of `skald` on `PATH` when it is this executable, otherwise this executable's path.
/// Symlinks in the `PATH` match stay unresolved.
fn hook_command() -> Result<String, SkaldError> {
    let exe = std::env::current_exe().map_err(|err| {
        SkaldError::new(
            "no-executable-path",
            format!("cannot find the skald executable: {err}"),
        )
    })?;
    let program = match paths::find_executable("skald") {
        Some(found) if same_file(&found, &exe) => std::path::absolute(found).unwrap_or(exe),
        _ => exe,
    };
    let program = program.to_str().ok_or_else(|| {
        SkaldError::new(
            "no-executable-path",
            format!(
                "the skald executable path {} is not UTF-8",
                program.display()
            ),
        )
    })?;
    Ok(format!("{}{HOOK_SUFFIX}", shell_word(program)))
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// `word` as one shell word, quoted only when the shell would otherwise split or expand it.
fn shell_word(word: &str) -> String {
    let plain = !word.is_empty()
        && word
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"/._-+@%:,".contains(&byte));
    if plain {
        word.to_owned()
    } else {
        format!("'{}'", word.replace('\'', r"'\''"))
    }
}

fn install_hook(settings: &Path, command: &str) -> Result<Status, String> {
    let mut value = match adapter::read_if_present(settings)? {
        Some(text) if !text.trim().is_empty() => serde_json::from_str(&text)
            .map_err(|err| format!("cannot parse {}: {err}", settings.display()))?,
        _ => json!({}),
    };
    let status = merge_hook(&mut value, command)
        .map_err(|problem| format!("cannot edit {}: {problem}", settings.display()))?;
    if status != Status::Unchanged {
        write_atomic(settings, &pretty(&value))
            .map_err(|err| format!("cannot write {}: {err}", settings.display()))?;
    }
    Ok(status)
}

/// Makes `settings` hold exactly one skald SessionStart entry, running `command`.
fn merge_hook(settings: &mut Value, command: &str) -> Result<Status, &'static str> {
    let entry = json!({"hooks": [{"type": "command", "command": command, "timeout": 10}]});
    let entries = settings
        .as_object_mut()
        .ok_or("the settings are not a JSON object")?
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or("`hooks` is not a JSON object")?
        .entry("SessionStart")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .ok_or("`hooks.SessionStart` is not a JSON array")?;
    let before = entries.clone();
    let mut kept: Vec<Value> = Vec::with_capacity(before.len() + 1);
    let mut position = None;
    for mut existing in before.iter().cloned() {
        let Some(hooks) = existing["hooks"].as_array_mut() else {
            kept.push(existing);
            continue;
        };
        let count = hooks.len();
        hooks.retain(|hook| !is_skald_hook(hook["command"].as_str().unwrap_or(""), command));
        if hooks.len() < count {
            position.get_or_insert(kept.len());
        }
        if !hooks.is_empty() {
            kept.push(existing);
        }
    }
    let found = position.is_some();
    kept.insert(position.unwrap_or(kept.len()), entry);
    if kept == before {
        return Ok(Status::Unchanged);
    }
    *entries = kept;
    Ok(if found {
        Status::Repaired
    } else {
        Status::Installed
    })
}

/// A hook is skald's when it is the command being installed, or when it runs a file named `skald`
/// as `<program> hook claude-code`. An older install may have recorded a different path.
fn is_skald_hook(hook_command: &str, command: &str) -> bool {
    if hook_command == command {
        return true;
    }
    let Some(program) = hook_command.strip_suffix(HOOK_SUFFIX) else {
        return false;
    };
    let word = match program.strip_prefix('\'') {
        Some(quoted) => quoted
            .strip_suffix('\'')
            .map(|inner| inner.replace(r"'\''", "'")),
        None => program.split_whitespace().next().map(str::to_owned),
    };
    word.is_some_and(|word| {
        Path::new(&word)
            .file_name()
            .is_some_and(|name| name == "skald")
    })
}

fn write_marker(path: &Path, command: &str, settings: &Path) -> Result<Status, String> {
    let version = env!("CARGO_PKG_VERSION");
    let settings = settings.display().to_string();
    let existing = adapter::read_if_present(path)?;
    let unchanged = existing
        .as_deref()
        .and_then(|text| serde_json::from_str::<Marker>(text).ok())
        .is_some_and(|marker| {
            marker.version == version && marker.command == command && marker.settings == settings
        });
    if unchanged {
        return Ok(Status::Unchanged);
    }
    let marker = Marker {
        runtime: RUNTIME.to_owned(),
        version: version.to_owned(),
        command: command.to_owned(),
        settings,
        installed_at: now().map_err(|err| format!("cannot read the clock: {err}"))?,
    };
    write_atomic(path, &pretty(&marker))
        .map_err(|err| format!("cannot write {}: {err}", path.display()))?;
    Ok(if existing.is_some() {
        Status::Repaired
    } else {
        Status::Installed
    })
}

fn now() -> rusqlite::Result<String> {
    // The marker's timestamps must use the store's format, which only `store::now` spells.
    store::now(&rusqlite::Connection::open_in_memory()?)
}

fn install_plugin(plugins_dir: &Path) -> Step {
    match adapter::installed_plugin(plugins_dir) {
        Ok(Some(installed)) => return Step::new("plugin", Status::Unchanged, installed.id),
        Ok(None) => {}
        Err(message) => return Step::new("plugin", Status::Failed, message),
    }
    let Some(claude) = paths::find_executable("claude") else {
        return Step::new("plugin", Status::Skipped, "claude executable not found");
    };
    for args in [&MARKETPLACE_ADD[..], &PLUGIN_INSTALL[..]] {
        if let Err(message) = run_claude(&claude, args) {
            return Step::new("plugin", Status::Failed, message);
        }
    }
    Step::new("plugin", Status::Installed, PLUGIN_ID)
}

/// Runs `claude` with its output captured, because skald's stdout carries only skald's output.
fn run_claude(claude: &Path, args: &[&str]) -> Result<(), String> {
    let output = Command::new(claude)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|err| format!("cannot run {}: {err}", claude.display()))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let first_line = stderr
        .lines()
        .chain(stdout.lines())
        .map(str::trim)
        .find(|line| !line.is_empty());
    Err(match first_line {
        Some(line) => line.to_owned(),
        None => format!("claude {} exited with {}", args.join(" "), output.status),
    })
}

fn pretty(value: &impl Serialize) -> String {
    let mut text = serde_json::to_string_pretty(value).expect("settings and markers are JSON");
    text.push('\n');
    text
}

/// Replaces `path` through a rename, so that Claude Code never reads a partial file. A symlink at
/// `path` stays in place and its target is replaced.
fn write_atomic(path: &Path, contents: &str) -> std::io::Result<()> {
    let target = fs::canonicalize(path).unwrap_or_else(|_| path.to_owned());
    let dir = target.parent().unwrap_or(Path::new(""));
    fs::create_dir_all(dir)?;
    let name = target.file_name().unwrap_or_default().to_string_lossy();
    let temp = dir.join(format!(".{name}.{}.tmp", std::process::id()));
    let result = replace(&temp, &target, contents);
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

fn replace(temp: &Path, target: &Path, contents: &str) -> std::io::Result<()> {
    create_private(temp, contents)?;
    // Settings may hold secrets, so a file the user restricted stays restricted.
    if let Ok(existing) = fs::metadata(target) {
        fs::set_permissions(temp, existing.permissions())?;
    }
    fs::rename(temp, target)
}

#[cfg(unix)]
fn create_private(path: &Path, contents: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(contents.as_bytes())
}

#[cfg(not(unix))]
fn create_private(path: &Path, contents: &str) -> std::io::Result<()> {
    fs::write(path, contents)
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMMAND: &str = "/usr/local/bin/skald hook claude-code";

    fn skald_entry(command: &str) -> Value {
        json!({"hooks": [{"type": "command", "command": command, "timeout": 10}]})
    }

    #[test]
    fn hook_is_installed_into_empty_settings() {
        let mut settings = json!({});
        assert_eq!(merge_hook(&mut settings, COMMAND), Ok(Status::Installed));
        assert_eq!(
            settings,
            json!({"hooks": {"SessionStart": [skald_entry(COMMAND)]}})
        );
    }

    #[test]
    fn identical_hook_is_unchanged() {
        let mut settings = json!({"hooks": {"SessionStart": [skald_entry(COMMAND)]}});
        let before = settings.clone();
        assert_eq!(merge_hook(&mut settings, COMMAND), Ok(Status::Unchanged));
        assert_eq!(settings, before);
    }

    #[test]
    fn stale_and_duplicate_hooks_are_repaired_in_place() {
        let other =
            json!({"matcher": "startup", "hooks": [{"type": "command", "command": "date"}]});
        let mut settings = json!({
            "model": "opus",
            "hooks": {
                "SessionStart": [
                    other,
                    skald_entry("/old/skald hook claude-code"),
                    other,
                    skald_entry(COMMAND),
                ]
            }
        });
        assert_eq!(merge_hook(&mut settings, COMMAND), Ok(Status::Repaired));
        assert_eq!(
            settings,
            json!({
                "model": "opus",
                "hooks": {"SessionStart": [other, skald_entry(COMMAND), other]}
            })
        );
    }

    #[test]
    fn a_skald_entry_with_a_matcher_is_repaired() {
        let mut entry = skald_entry(COMMAND);
        entry["matcher"] = json!("startup");
        let mut settings = json!({"hooks": {"SessionStart": [entry]}});
        assert_eq!(merge_hook(&mut settings, COMMAND), Ok(Status::Repaired));
        assert_eq!(
            settings["hooks"]["SessionStart"],
            json!([skald_entry(COMMAND)])
        );
    }

    #[test]
    fn hooks_sharing_an_entry_with_skald_survive() {
        let load_env = json!({"type": "command", "command": "~/bin/load-env.sh"});
        let shared = json!({
            "hooks": [load_env, {"type": "command", "command": "/old/skald hook claude-code"}]
        });
        let mut settings = json!({"hooks": {"SessionStart": [shared]}});
        assert_eq!(merge_hook(&mut settings, COMMAND), Ok(Status::Repaired));
        assert_eq!(
            settings["hooks"]["SessionStart"],
            json!([skald_entry(COMMAND), {"hooks": [load_env]}])
        );
        assert_eq!(merge_hook(&mut settings, COMMAND), Ok(Status::Unchanged));
    }

    #[test]
    fn only_hooks_that_run_a_file_named_skald_are_skald_hooks() {
        assert!(is_skald_hook("skald hook claude-code", COMMAND));
        assert!(is_skald_hook("/old/skald hook claude-code", COMMAND));
        assert!(is_skald_hook(
            "'/Users/ada/App Support/skald' hook claude-code",
            COMMAND
        ));
        assert!(is_skald_hook(COMMAND, COMMAND));
        assert!(!is_skald_hook("other-tool hook claude-code", COMMAND));
        assert!(!is_skald_hook("skald hook other", COMMAND));
    }

    #[test]
    fn settings_of_the_wrong_shape_are_refused() {
        for mut settings in [
            json!([]),
            json!({"hooks": []}),
            json!({"hooks": {"SessionStart": {}}}),
        ] {
            let before = settings.clone();
            assert!(merge_hook(&mut settings, COMMAND).is_err(), "{before}");
            assert_eq!(settings, before);
        }
    }

    #[test]
    fn shell_word_quotes_only_when_needed() {
        assert_eq!(
            shell_word("/home/ada/.cargo/bin/skald"),
            "/home/ada/.cargo/bin/skald"
        );
        assert_eq!(
            shell_word("/Users/ada/Application Support/skald"),
            "'/Users/ada/Application Support/skald'"
        );
        assert_eq!(shell_word("/tmp/ada's/skald"), r"'/tmp/ada'\''s/skald'");
        assert_eq!(shell_word("/tmp/$HOME/skald"), "'/tmp/$HOME/skald'");
    }
}
