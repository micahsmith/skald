mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use common::Sandbox;
use serde_json::Value;

const SCRIPT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/plugin/hooks/knowledge-base.sh"
);

struct Run {
    code: i32,
    stdout: String,
    stderr: String,
    /// The decoded `knowledge_base` object, or null when stdout is empty.
    output: Value,
}

/// The script under `/bin/bash` in `dir`, with the sandbox environment.
fn script(sandbox: &Sandbox, dir: &Path) -> Command {
    let mut cmd = sandbox.command("/bin/bash", dir);
    cmd.arg(SCRIPT);
    cmd
}

fn run(cmd: &mut Command) -> Run {
    let output = cmd.output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    let output_value = if stdout.is_empty() {
        Value::Null
    } else {
        assert!(stdout.starts_with("knowledge_base:\n"), "stdout:\n{stdout}");
        let decoded: Value = toon_format::decode_default(&stdout)
            .unwrap_or_else(|err| panic!("{err} in:\n{stdout}"));
        assert_eq!(
            common::keys(&decoded),
            ["knowledge_base"],
            "stdout:\n{stdout}"
        );
        decoded["knowledge_base"].clone()
    };
    Run {
        code: output.status.code().unwrap(),
        stdout,
        stderr,
        output: output_value,
    }
}

fn run_ok(cmd: &mut Command) -> Run {
    let run = run(cmd);
    assert_eq!(
        run.code, 0,
        "stdout:\n{}\nstderr:\n{}",
        run.stdout, run.stderr
    );
    run
}

/// Runs the default command in `dir`, asserts exit 0, and returns the run.
fn default_run(sandbox: &Sandbox, dir: &Path) -> Run {
    run_ok(&mut script(sandbox, dir))
}

fn knowledge_base(sandbox: &Sandbox, dir: &Path) -> Value {
    default_run(sandbox, dir).output
}

/// The keys of the `knowledge_base` object, in the order stdout lists them.
fn raw_keys(stdout: &str) -> Vec<&str> {
    stdout
        .lines()
        .skip(1)
        .map(|line| line.trim_start().split_once(':').unwrap().0)
        .collect()
}

fn skald_dir(base: &Path) -> String {
    format!("{}/.skald", base.display())
}

fn mkdir(path: &Path) {
    std::fs::create_dir_all(path).unwrap();
}

fn write(path: &Path, contents: &str) {
    mkdir(path.parent().unwrap());
    std::fs::write(path, contents).unwrap();
}

/// Every path under the sandbox root except `.git/` folders. A file maps to its bytes, a symbolic
/// link to its target, and a folder to `None`.
fn snapshot(sandbox: &Sandbox) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    let mut entries = BTreeMap::new();
    let mut pending = vec![sandbox.root.clone()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let kind = entry.file_type().unwrap();
            if kind.is_dir() {
                if entry.file_name() != ".git" {
                    pending.push(path.clone());
                    entries.insert(path, None);
                }
            } else if kind.is_symlink() {
                let target = std::fs::read_link(&path).unwrap();
                entries.insert(path, Some(target.into_os_string().into_encoded_bytes()));
            } else {
                let bytes = std::fs::read(&path).unwrap();
                entries.insert(path, Some(bytes));
            }
        }
    }
    entries
}

#[test]
fn reports_a_missing_folder_in_the_repository_root_without_creating_it() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_without_origin();
    let before = snapshot(&sandbox);

    let output = knowledge_base(&sandbox, &repo);

    assert_eq!(output["dir"], skald_dir(&repo));
    assert_eq!(output["exists"], false);
    assert_eq!(output["mode"], "markdown");
    assert_eq!(output["ignored"], false);
    assert_eq!(common::keys(&output), ["dir", "exists", "mode", "ignored"]);
    assert_eq!(snapshot(&sandbox), before);
}

#[test]
fn the_repository_root_folder_wins_over_a_listed_folder() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_without_origin();
    mkdir(&repo.join(".skald"));
    mkdir(&repo.join(".claude/.skald"));

    let output = knowledge_base(&sandbox, &repo);

    assert_eq!(output["dir"], skald_dir(&repo));
    assert_eq!(output["exists"], true);
}

#[test]
fn a_listed_folder_is_found_when_the_root_has_none() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_without_origin();
    mkdir(&repo.join("scratch/.skald"));

    let output = knowledge_base(&sandbox, &repo);

    assert_eq!(output["dir"], skald_dir(&repo.join("scratch")));
    assert_eq!(output["exists"], true);
}

#[test]
fn the_first_listed_folder_wins_when_several_exist() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_without_origin();
    mkdir(&repo.join("temp/.skald"));
    mkdir(&repo.join(".cursor/.skald"));

    let output = knowledge_base(&sandbox, &repo);

    assert_eq!(output["dir"], skald_dir(&repo.join(".cursor")));
}

#[test]
fn regular_file_is_no_match() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_without_origin();
    write(&repo.join(".skald"), "not a folder\n");

    let output = knowledge_base(&sandbox, &repo);

    assert_eq!(output["exists"], false);
    assert_eq!(output["dir"], skald_dir(&repo));
}

#[test]
fn a_subdirectory_finds_the_folder_in_the_repository_root() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_without_origin();
    mkdir(&repo.join(".skald"));
    let deep = repo.join("src/deep");
    mkdir(&deep);

    let output = knowledge_base(&sandbox, &deep);

    assert_eq!(output["dir"], skald_dir(&repo));
}

#[test]
fn outside_git_the_folder_is_looked_up_in_the_working_directory() {
    let sandbox = Sandbox::new();

    let output = knowledge_base(&sandbox, &sandbox.work);

    assert_eq!(output["dir"], skald_dir(&sandbox.work));
    assert_eq!(output["exists"], false);
    assert_eq!(output["ignored"], false);
}

#[test]
fn a_symbolic_link_to_the_repository_resolves_to_the_real_path() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_without_origin();
    mkdir(&repo.join(".skald"));
    let link = sandbox.root.join("link");
    std::os::unix::fs::symlink(&repo, &link).unwrap();

    let output = knowledge_base(&sandbox, &link);

    assert_eq!(output["dir"], skald_dir(&repo));
}

#[test]
fn a_gitignore_inside_the_folder_marks_it_ignored() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_without_origin();
    write(&repo.join(".skald/.gitignore"), "*\n");

    let output = knowledge_base(&sandbox, &repo);

    assert_eq!(output["ignored"], true);
}

#[test]
fn a_folder_without_a_gitignore_is_not_ignored() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_without_origin();
    mkdir(&repo.join(".skald"));

    let output = knowledge_base(&sandbox, &repo);

    assert_eq!(output["exists"], true);
    assert_eq!(output["ignored"], false);
}

#[test]
fn a_folder_ignored_by_the_repository_gitignore_is_ignored() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_without_origin();
    write(&repo.join(".gitignore"), "scratch/\n");
    mkdir(&repo.join("scratch/.skald"));

    let output = knowledge_base(&sandbox, &repo);

    assert_eq!(output["dir"], skald_dir(&repo.join("scratch")));
    assert_eq!(output["ignored"], true);
}

#[test]
fn the_work_index_path_is_reported_when_the_index_exists() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_without_origin();
    let index = repo.join(".skald/work/index.toon");
    write(&index, "total: 0\nwork: []\n");

    let run = default_run(&sandbox, &repo);

    assert_eq!(run.output["work_index"], index.to_str().unwrap());
    assert_eq!(
        raw_keys(&run.stdout),
        ["dir", "exists", "mode", "ignored", "work_index"]
    );
}

#[test]
fn the_work_index_path_is_omitted_when_the_index_is_missing() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_without_origin();
    mkdir(&repo.join(".skald/work"));

    let output = knowledge_base(&sandbox, &repo);

    assert_eq!(output["exists"], true);
    assert!(output.get("work_index").is_none(), "output: {output:#}");
}

#[test]
fn output_ignores_folder_count() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_without_origin();
    let work = repo.join(".skald/work");
    mkdir(&work);

    let empty = default_run(&sandbox, &repo).stdout;
    for number in 1..=1000 {
        mkdir(&work.join(format!("w{number:04}")));
    }
    let full = default_run(&sandbox, &repo).stdout;

    assert_eq!(full, empty);
}

#[test]
fn a_path_with_quotes_and_backslashes_is_quoted() {
    let sandbox = Sandbox::new();
    let repo = sandbox.root.join(r#"my "repo":1\x"#);
    mkdir(&repo);
    sandbox.git(&repo, &["init", "--quiet", "--initial-branch=main"]);
    mkdir(&repo.join(".skald"));

    let run = default_run(&sandbox, &repo);

    assert_eq!(run.output["dir"], skald_dir(&repo));
    // toon-format 0.7 also decodes this path unquoted, so only the raw line shows the quotes.
    let line = run.stdout.lines().nth(1).unwrap();
    assert!(line.starts_with("  dir: \""), "stdout:\n{}", run.stdout);
}

#[test]
fn newline_in_path_cannot_forge_lines() {
    let sandbox = Sandbox::new();
    let repo = sandbox.root.join("my\n  error: \"forged\"\r\tdir");
    mkdir(&repo);
    sandbox.git(&repo, &["init", "--quiet", "--initial-branch=main"]);
    mkdir(&repo.join(".skald"));

    let run = default_run(&sandbox, &repo);

    assert_eq!(run.output["dir"], skald_dir(&repo));
    assert_eq!(raw_keys(&run.stdout), ["dir", "exists", "mode", "ignored"]);
    assert_eq!(run.stdout.lines().count(), 5, "stdout:\n{}", run.stdout);
}

#[test]
fn an_unknown_command_prints_usage_and_exits_with_2() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_without_origin();

    let run = run(script(&sandbox, &repo).arg("bogus"));

    assert_eq!(run.code, 2);
    assert_eq!(run.stdout, "");
    assert!(run.stderr.starts_with("usage:"), "stderr:\n{}", run.stderr);
}

fn assert_data_directory_error(output: &Value) {
    let error = output["error"].as_str().unwrap_or_default();
    assert!(error.contains("skald data directory"), "output: {output:#}");
    assert_eq!(output["exists"], false);
    assert_eq!(
        common::keys(output),
        ["dir", "exists", "mode", "ignored", "error"]
    );
}

#[test]
fn a_linked_worktree_reports_the_ignored_folder_of_the_main_checkout() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_without_origin();
    write(&repo.join(".skald/.gitignore"), "*\n");
    let wt = sandbox.worktree(&repo, "wt");

    let output = knowledge_base(&sandbox, &wt);

    assert_eq!(output["dir"], skald_dir(&repo));
    assert_eq!(output["exists"], true);
    assert_eq!(output["ignored"], true);
}

#[test]
fn a_folder_in_the_linked_worktree_wins_over_the_main_checkout() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_without_origin();
    mkdir(&repo.join(".skald"));
    let wt = sandbox.worktree(&repo, "wt");
    mkdir(&wt.join(".skald"));

    let output = knowledge_base(&sandbox, &wt);

    assert_eq!(output["dir"], skald_dir(&wt));
}

#[test]
fn a_linked_worktree_without_any_folder_reports_the_main_checkout_path() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_without_origin();
    let wt = sandbox.worktree(&repo, "wt");

    let output = knowledge_base(&sandbox, &wt);

    assert_eq!(output["dir"], skald_dir(&repo));
    assert_eq!(output["exists"], false);
}

#[test]
fn a_worktree_of_a_bare_repository_reports_its_own_path_without_error() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_without_origin();
    let bare = sandbox.root.join("bare.git");
    let worktree = sandbox.root.join("bare-wt");
    sandbox.git(
        &sandbox.root,
        &[
            "clone",
            "--quiet",
            "--bare",
            repo.to_str().unwrap(),
            bare.to_str().unwrap(),
        ],
    );
    sandbox.git(
        &bare,
        &[
            "worktree",
            "add",
            "--quiet",
            worktree.to_str().unwrap(),
            "-b",
            "wt",
        ],
    );

    let output = knowledge_base(&sandbox, &worktree);

    assert_eq!(output["dir"], skald_dir(&worktree));
    assert_eq!(output["exists"], false);
    assert!(output.get("error").is_none(), "output: {output:#}");
}

#[test]
fn a_submodule_uses_its_own_checkout() {
    let sandbox = Sandbox::new();
    let library = sandbox.repo_without_origin();
    let repo = sandbox.repo_without_origin();
    sandbox.git(
        &repo,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "--quiet",
            "add",
            library.to_str().unwrap(),
            "sub",
        ],
    );
    let sub = repo.join("sub");

    let output = knowledge_base(&sandbox, &sub);

    assert_eq!(output["dir"], skald_dir(&sub));
}

#[test]
fn a_folder_that_is_the_skald_data_directory_is_an_error() {
    let sandbox = Sandbox::new();
    let data = sandbox.work.join(".skald");
    mkdir(&data);
    let before = snapshot(&sandbox);

    let run = run_ok(script(&sandbox, &sandbox.work).env("SKALD_HOME", &data));

    assert_data_directory_error(&run.output);
    assert_eq!(snapshot(&sandbox), before);
}

#[test]
fn a_link_to_the_skald_data_directory_is_an_error() {
    let sandbox = Sandbox::new();
    let data = sandbox.work.join(".skald");
    mkdir(&data);
    let link = sandbox.root.join("data-link");
    std::os::unix::fs::symlink(&data, &link).unwrap();
    let before = snapshot(&sandbox);

    let run = run_ok(script(&sandbox, &sandbox.work).env("SKALD_HOME", &link));

    assert_data_directory_error(&run.output);
    assert_eq!(snapshot(&sandbox), before);
}

#[test]
fn the_default_data_directory_is_an_error_when_it_is_missing() {
    let sandbox = Sandbox::new();
    let mut cmd = script(&sandbox, &sandbox.work);
    cmd.env_remove("SKALD_HOME").env("HOME", &sandbox.work);

    let run = run_ok(&mut cmd);

    assert_data_directory_error(&run.output);
}

#[test]
fn a_folder_is_valid_when_the_data_directory_is_elsewhere() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_without_origin();
    mkdir(&repo.join(".skald"));

    let output = knowledge_base(&sandbox, &repo);

    assert_eq!(output["dir"], skald_dir(&repo));
    assert!(output.get("error").is_none(), "output: {output:#}");
}

/// Runs the default command in a repository whose `.skald/config.toon` holds `config`.
fn with_config(config: &str) -> Run {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_without_origin();
    write(&repo.join(".skald/config.toon"), config);
    default_run(&sandbox, &repo)
}

fn assert_markdown(output: &Value) {
    assert_eq!(output["mode"], "markdown", "output: {output:#}");
    assert!(output.get("error").is_none(), "output: {output:#}");
}

fn assert_config_error(output: &Value, fragment: &str) {
    let error = output["error"].as_str().unwrap_or_default();
    assert!(error.contains(fragment), "output: {output:#}");
    assert!(output.get("mode").is_none(), "output: {output:#}");
}

#[test]
fn a_folder_without_a_config_uses_markdown_mode() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_without_origin();
    mkdir(&repo.join(".skald"));

    assert_markdown(&knowledge_base(&sandbox, &repo));
}

#[test]
fn a_blank_config_uses_markdown_mode() {
    assert_markdown(&with_config("\n\n   \n").output);
}

#[test]
fn a_quoted_mode_value_is_accepted() {
    assert_markdown(&with_config("mode: \"markdown\"\n").output);
}

#[test]
fn a_blank_line_before_the_key_is_accepted() {
    assert_markdown(&with_config("\nmode: markdown\n").output);
}

#[test]
fn skald_mode_is_reported_as_unavailable() {
    assert_config_error(
        &with_config("mode: skald\n").output,
        "skald mode is not available yet",
    );
}

#[test]
fn an_unknown_mode_is_an_error() {
    assert_config_error(
        &with_config("mode: files\n").output,
        "only valid value is markdown",
    );
}

#[test]
fn an_empty_mode_is_an_error() {
    assert_config_error(
        &with_config("mode:\n").output,
        "only valid value is markdown",
    );
}

#[test]
fn an_unknown_key_is_an_error_on_its_line() {
    assert_config_error(&with_config("colour: blue\n").output, "line 1");
}

#[test]
fn a_line_without_a_colon_is_an_error_on_its_line() {
    assert_config_error(&with_config("mode markdown\n").output, "line 1");
}

#[test]
fn line_number_counts_blanks() {
    assert_config_error(
        &with_config("\nmode: markdown\ncolour: blue\n").output,
        "line 3",
    );
}

#[test]
fn an_unreadable_config_is_an_error() {
    use std::os::unix::fs::PermissionsExt;

    let sandbox = Sandbox::new();
    let repo = sandbox.repo_without_origin();
    let config = repo.join(".skald/config.toon");
    write(&config, "mode: markdown\n");
    std::fs::set_permissions(&config, std::fs::Permissions::from_mode(0o000)).unwrap();
    // Root reads the file despite its mode, so the case cannot be set up.
    if std::fs::read(&config).is_ok() {
        return;
    }

    let output = knowledge_base(&sandbox, &repo);

    assert_config_error(&output, "cannot read");
}

#[test]
fn a_folder_that_cannot_be_entered_is_reported_as_an_error_and_blocks_init_and_index() {
    use std::os::unix::fs::PermissionsExt;

    let sandbox = Sandbox::new();
    let repo = sandbox.repo_without_origin();
    let folder = repo.join(".skald");
    mkdir(&folder);
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o000)).unwrap();
    // Root enters the folder despite its mode, so the case cannot be set up.
    if std::fs::read_dir(&folder).is_ok() {
        std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o755)).unwrap();
        return;
    }

    let default = default_run(&sandbox, &repo);
    let init = init(&sandbox, &repo);
    let index = index(&sandbox, &repo);
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o755)).unwrap();

    assert!(default.output["dir"].as_str().unwrap().ends_with("/.skald"));
    assert!(
        default.output["error"]
            .as_str()
            .unwrap_or_default()
            .contains("cannot open")
    );
    assert_failed(&init, "cannot open");
    assert_failed(&index, "cannot open");
}

#[test]
fn the_error_field_comes_last_in_the_output() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_without_origin();
    write(&repo.join(".skald/config.toon"), "mode: skald\n");
    write(&repo.join(".skald/.gitignore"), "*\n");

    let run = default_run(&sandbox, &repo);

    assert_eq!(run.output["ignored"], true);
    assert_eq!(raw_keys(&run.stdout), ["dir", "exists", "ignored", "error"]);
}

fn init(sandbox: &Sandbox, dir: &Path) -> Run {
    run(script(sandbox, dir).arg("init"))
}

fn init_ok(sandbox: &Sandbox, dir: &Path) -> Run {
    run_ok(script(sandbox, dir).arg("init"))
}

fn assert_failed(run: &Run, fragment: &str) {
    assert_eq!(
        run.code, 1,
        "stdout:\n{}\nstderr:\n{}",
        run.stdout, run.stderr
    );
    let error = run.output["error"].as_str().unwrap_or_default();
    assert!(error.contains(fragment), "stdout:\n{}", run.stdout);
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn init_creates_the_config_gitignore_and_work_folder() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_without_origin();

    let output = init_ok(&sandbox, &repo).output;

    let skald = repo.join(".skald");
    assert_eq!(read(&skald.join("config.toon")), "mode: markdown\n");
    assert_eq!(read(&skald.join(".gitignore")), "*\n");
    assert_eq!(std::fs::read_dir(skald.join("work")).unwrap().count(), 0);
    assert_eq!(output["dir"], skald_dir(&repo));
    assert_eq!(output["exists"], true);
    assert_eq!(output["mode"], "markdown");
    assert_eq!(output["ignored"], true);
    assert_eq!(common::keys(&output), ["dir", "exists", "mode", "ignored"]);
}

#[test]
fn second_init_changes_nothing() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_without_origin();
    init_ok(&sandbox, &repo);
    let before = snapshot(&sandbox);

    init_ok(&sandbox, &repo);

    assert_eq!(snapshot(&sandbox), before);
}

#[test]
fn repair_keeps_the_folder_committed() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_without_origin();
    let skald = repo.join(".skald");
    mkdir(&skald);

    let output = init_ok(&sandbox, &repo).output;

    assert!(skald.join("config.toon").is_file());
    assert!(skald.join("work").is_dir());
    assert!(!skald.join(".gitignore").exists());
    assert_eq!(output["ignored"], false);
}

#[test]
fn repair_in_a_listed_folder() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_without_origin();
    mkdir(&repo.join("scratch/.skald"));

    init_ok(&sandbox, &repo);

    assert!(repo.join("scratch/.skald/config.toon").is_file());
    assert!(!repo.join(".skald").exists());
}

#[test]
fn existing_config_is_kept() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_without_origin();
    let config = repo.join(".skald/config.toon");
    write(&config, "mode: \"markdown\"\n");

    init_ok(&sandbox, &repo);

    assert_eq!(read(&config), "mode: \"markdown\"\n");
    assert!(repo.join(".skald/work").is_dir());
}

#[test]
fn init_in_a_linked_worktree_creates_the_folder_in_the_main_checkout() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_without_origin();
    let wt = sandbox.worktree(&repo, "wt");

    let output = init_ok(&sandbox, &wt).output;

    assert!(repo.join(".skald").is_dir());
    assert!(!wt.join(".skald").exists());
    assert_eq!(output["dir"], skald_dir(&repo));
}

#[test]
fn init_outside_git_creates_the_folder_in_the_working_directory() {
    let sandbox = Sandbox::new();

    let output = init_ok(&sandbox, &sandbox.work).output;

    assert!(sandbox.work.join(".skald/config.toon").is_file());
    assert_eq!(output["ignored"], false);
}

#[test]
fn config_error_blocks_init() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_without_origin();
    write(&repo.join(".skald/config.toon"), "mode: skald\n");
    let before = snapshot(&sandbox);

    let run = init(&sandbox, &repo);

    assert_failed(&run, "skald mode is not available yet");
    assert_eq!(snapshot(&sandbox), before);
}

#[test]
fn data_directory_blocks_init() {
    let sandbox = Sandbox::new();
    let data = sandbox.work.join(".skald");
    mkdir(&data);
    let before = snapshot(&sandbox);

    let run = run(script(&sandbox, &sandbox.work)
        .arg("init")
        .env("SKALD_HOME", &data));

    assert_failed(&run, "skald data directory");
    assert_data_directory_error(&run.output);
    assert_eq!(snapshot(&sandbox), before);
}

#[test]
fn init_fails_when_the_folder_path_is_a_regular_file() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_without_origin();
    let file = repo.join(".skald");
    write(&file, "not a folder\n");

    let run = init(&sandbox, &repo);

    assert_failed(&run, "cannot create");
    assert_eq!(read(&file), "not a folder\n");
}

fn index(sandbox: &Sandbox, dir: &Path) -> Run {
    run(script(sandbox, dir).arg("index"))
}

fn index_ok(sandbox: &Sandbox, dir: &Path) -> Run {
    run_ok(script(sandbox, dir).arg("index"))
}

/// A repository after `init`, and its `.skald/work` folder.
fn initialized(sandbox: &Sandbox) -> (PathBuf, PathBuf) {
    let repo = sandbox.repo_without_origin();
    init_ok(sandbox, &repo);
    let work = repo.join(".skald/work");
    (repo, work)
}

/// The decoded `work/index.toon` of `repo`.
fn index_file(repo: &Path) -> Value {
    let text = read(&repo.join(".skald/work/index.toon"));
    toon_format::decode_default(&text).unwrap_or_else(|err| panic!("{err} in:\n{text}"))
}

/// The `name` of each row of the index, in order.
fn names(index: &Value) -> Vec<&str> {
    index["work"]
        .as_array()
        .unwrap_or_else(|| panic!("index: {index:#}"))
        .iter()
        .map(|row| row["name"].as_str().unwrap())
        .collect()
}

fn updated<'a>(index: &'a Value, name: &str) -> &'a str {
    let rows = index["work"].as_array().unwrap();
    let row = rows.iter().find(|row| row["name"] == name).unwrap();
    row["updated"].as_str().unwrap()
}

/// Sets the modification time of `path` with `touch -t`, in local time.
fn touch(sandbox: &Sandbox, path: &Path, stamp: &str) {
    let status = sandbox
        .command("touch", &sandbox.root)
        .arg("-t")
        .arg(stamp)
        .arg(path)
        .status()
        .unwrap();
    assert!(status.success(), "touch -t {stamp} {}", path.display());
}

/// Writes `path` and sets its time to `stamp`.
fn write_at(sandbox: &Sandbox, path: &Path, stamp: &str) {
    write(path, "notes\n");
    touch(sandbox, path, stamp);
}

#[test]
fn the_index_keeps_only_the_newest_40_folders() {
    let sandbox = Sandbox::new();
    let (repo, work) = initialized(&sandbox);
    for number in 1..=41 {
        let file = work.join(format!("w{number:02}/notes.md"));
        write_at(&sandbox, &file, &format!("2026010110{number:02}"));
    }

    let output = index_ok(&sandbox, &repo).output;

    let index = index_file(&repo);
    assert_eq!(index["total"], 41);
    let names = names(&index);
    assert_eq!(names.len(), 40);
    assert_eq!(names[0], "w41");
    assert_eq!(names[39], "w02");
    assert!(!names.contains(&"w01"), "index: {index:#}");
    let path = work.join("index.toon");
    assert_eq!(output["work_index"], path.to_str().unwrap());
}

#[test]
fn deep_edit_moves_a_folder_first() {
    let sandbox = Sandbox::new();
    let (repo, work) = initialized(&sandbox);
    let deep = work.join("a/x/y/file.md");
    write_at(&sandbox, &deep, "202601011200");
    write_at(&sandbox, &work.join("b/file.md"), "202602011200");
    index_ok(&sandbox, &repo);
    assert_eq!(names(&index_file(&repo)), ["b", "a"]);

    write(&deep, "edited\n");
    index_ok(&sandbox, &repo);

    let index = index_file(&repo);
    assert_eq!(names(&index)[0], "a");
    let today = String::from_utf8(sandbox.bash(&repo, "date +%F").stdout).unwrap();
    assert_eq!(updated(&index, "a"), today.trim());
}

#[test]
fn folders_with_equal_times_are_sorted_by_name() {
    let sandbox = Sandbox::new();
    let (repo, work) = initialized(&sandbox);
    write_at(&sandbox, &work.join("beta/notes.md"), "202603011200");
    write_at(&sandbox, &work.join("alpha/notes.md"), "202603011200");

    index_ok(&sandbox, &repo);

    assert_eq!(names(&index_file(&repo)), ["alpha", "beta"]);
}

#[test]
fn an_empty_folder_is_dated_by_its_own_time() {
    let sandbox = Sandbox::new();
    let (repo, work) = initialized(&sandbox);
    let empty = work.join("empty");
    mkdir(&empty);
    touch(&sandbox, &empty, "202604051200");

    index_ok(&sandbox, &repo);

    assert_eq!(updated(&index_file(&repo), "empty"), "2026-04-05");
}

#[test]
fn file_time_beats_folder_time() {
    let sandbox = Sandbox::new();
    let (repo, work) = initialized(&sandbox);
    write_at(&sandbox, &work.join("c/notes.md"), "202601101200");
    touch(&sandbox, &work.join("c"), "202606011200");

    index_ok(&sandbox, &repo);

    assert_eq!(updated(&index_file(&repo), "c"), "2026-01-10");
}

#[test]
fn folder_names_that_need_quotes_are_kept_as_strings() {
    let sandbox = Sandbox::new();
    let (repo, work) = initialized(&sandbox);
    for name in ["a,b", "2026", "two words"] {
        write(&work.join(name).join("notes.md"), "notes\n");
    }

    index_ok(&sandbox, &repo);

    let index = index_file(&repo);
    for row in index["work"].as_array().unwrap() {
        assert!(row["name"].is_string(), "index: {index:#}");
    }
    let names = names(&index);
    for name in ["a,b", "2026", "two words"] {
        assert!(names.contains(&name), "{name:?} missing from: {index:#}");
    }
}

#[test]
fn an_empty_work_folder_gives_an_empty_index() {
    let sandbox = Sandbox::new();
    let (repo, _) = initialized(&sandbox);

    index_ok(&sandbox, &repo);

    let index = index_file(&repo);
    assert_eq!(index["total"], 0);
    assert_eq!(index["work"], Value::Array(Vec::new()));
}

#[test]
fn index_creates_missing_work() {
    let sandbox = Sandbox::new();
    let (repo, work) = initialized(&sandbox);
    std::fs::remove_dir(&work).unwrap();

    index_ok(&sandbox, &repo);

    assert_eq!(index_file(&repo)["total"], 0);
}

#[test]
fn an_existing_index_file_is_not_counted_as_a_folder() {
    let sandbox = Sandbox::new();
    let (repo, work) = initialized(&sandbox);
    write_at(&sandbox, &work.join("only/notes.md"), "202605011200");
    write(&work.join("index.toon"), "total: 0\nwork: []\n");

    index_ok(&sandbox, &repo);
    let first = read(&work.join("index.toon"));
    assert_eq!(index_file(&repo)["total"], 1);
    index_ok(&sandbox, &repo);

    assert_eq!(index_file(&repo)["total"], 1);
    assert_eq!(read(&work.join("index.toon")), first);
}

#[test]
fn index_fails_when_the_knowledge_base_is_missing() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_without_origin();
    let before = snapshot(&sandbox);

    let run = index(&sandbox, &repo);

    assert_failed(&run, "run init first");
    assert_eq!(snapshot(&sandbox), before);
}

#[test]
fn config_error_blocks_index() {
    let sandbox = Sandbox::new();
    let (repo, work) = initialized(&sandbox);
    write(&repo.join(".skald/config.toon"), "mode: skald\n");
    write(&work.join("only/notes.md"), "notes\n");
    let before = snapshot(&sandbox);

    let run = index(&sandbox, &repo);

    assert_failed(&run, "skald mode is not available yet");
    assert_eq!(snapshot(&sandbox), before);
}
