mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use common::{Sandbox, read, repo_path};
use serde_json::Value;

const KNOWLEDGE_BASE_COMMAND: &str = r#"bash "${CLAUDE_PLUGIN_ROOT}/hooks/knowledge-base.sh""#;

const KNOWLEDGE_BASE_SKILLS: [&str; 6] = [
    "write-spec",
    "write-plan",
    "handoff",
    "deep-review",
    "merge-review",
    "resolve-review",
];

const KNOWLEDGE_BASE_BLOCK: &str = "\
## Knowledge Base

Read `knowledge_base` from the session context. If it is missing, or its `dir` is outside the
repository you are working in, run `bash scripts/knowledge-base.sh` to print it. If it reports an
`error`, show the error to the user and stop. Before the first save, run
`bash scripts/knowledge-base.sh init` if `exists` is false. After `init` in a git repository, if
`ignored` is false, git tracks the folder, so tell the user before you save there. After you create
or edit any file under `work/`, run `bash scripts/knowledge-base.sh index`.";

const END_MARKER: &str = "<!-- end shared -->";

/// The command of every hook that `plugin/hooks/hooks.json` registers for `event`.
fn hook_commands(event: &str) -> Vec<String> {
    let config: Value = serde_json::from_str(&read("plugin/hooks/hooks.json")).unwrap();
    config["hooks"][event]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|entry| entry["hooks"].as_array().into_iter().flatten())
        .map(|hook| hook["command"].as_str().unwrap().to_owned())
        .collect()
}

fn runs_knowledge_base(command: &str) -> bool {
    command.contains("knowledge-base.sh")
}

#[test]
fn session_start_runs_guides_and_the_knowledge_base_once() {
    let commands = hook_commands("SessionStart");
    let knowledge_base: Vec<_> = commands
        .iter()
        .filter(|command| runs_knowledge_base(command))
        .collect();
    assert_eq!(knowledge_base, [KNOWLEDGE_BASE_COMMAND], "{commands:?}");
    let guides = r#"bash "${CLAUDE_PLUGIN_ROOT}/hooks/emit-guides.sh" session"#;
    assert!(
        commands.iter().any(|command| command == guides),
        "{commands:?}"
    );
}

#[test]
fn subagent_start_runs_guides_but_not_the_knowledge_base() {
    let commands = hook_commands("SubagentStart");
    let guides = r#"bash "${CLAUDE_PLUGIN_ROOT}/hooks/emit-guides.sh" subagent"#;
    assert!(
        commands.iter().any(|command| command == guides),
        "{commands:?}"
    );
    assert!(
        !commands.iter().any(|command| runs_knowledge_base(command)),
        "{commands:?}"
    );
}

#[test]
fn the_session_start_knowledge_base_command_runs_and_prints_the_folder_report() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_without_origin();
    let command = hook_commands("SessionStart")
        .into_iter()
        .find(|command| runs_knowledge_base(command))
        .expect("no SessionStart command runs knowledge-base.sh");

    let output = sandbox
        .command("/bin/bash", &repo)
        .args(["-c", &command])
        .env("CLAUDE_PLUGIN_ROOT", repo_path("plugin"))
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_eq!(
        stdout.lines().next(),
        Some("knowledge_base:"),
        "stdout:\n{stdout}"
    );
}

#[test]
fn readme_names_the_hook() {
    let readme = read("README.md");
    let (_, after) = readme
        .split_once("\n## Hooks\n")
        .expect("README.md has no Hooks section");
    let section = after.split("\n## ").next().unwrap();
    assert!(section.contains("knowledge-base"), "{section}");
    assert!(section.contains("session start only"), "{section}");
}

struct Block {
    name: String,
    /// The 1-based line number of the opening marker.
    line: usize,
    /// `None` when no closing marker follows the opening marker.
    content: Option<String>,
}

fn opening_marker(line: &str) -> Option<&str> {
    line.strip_prefix("<!-- shared: ")?
        .strip_suffix(" -->")
        .filter(|name| !name.is_empty())
}

fn trim_blank_lines(lines: &[&str]) -> String {
    let filled = |line: &&str| !line.trim().is_empty();
    let start = lines.iter().position(filled).unwrap_or(lines.len());
    let end = lines
        .iter()
        .rposition(filled)
        .map_or(start, |last| last + 1);
    lines[start..end].join("\n")
}

fn shared_blocks(text: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut open: Option<(Block, Vec<&str>)> = None;
    for (index, line) in text.lines().enumerate() {
        if let Some(name) = opening_marker(line) {
            blocks.extend(open.take().map(|(block, _)| block));
            let block = Block {
                name: name.to_owned(),
                line: index + 1,
                content: None,
            };
            open = Some((block, Vec::new()));
        } else if line == END_MARKER {
            if let Some((mut block, lines)) = open.take() {
                block.content = Some(trim_blank_lines(&lines));
                blocks.push(block);
            }
        } else if let Some((_, lines)) = &mut open {
            lines.push(line);
        }
    }
    blocks.extend(open.map(|(block, _)| block));
    blocks
}

fn block_content(text: &str, name: &str) -> Option<String> {
    shared_blocks(text)
        .into_iter()
        .find(|block| block.name == name)?
        .content
}

fn markdown_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(markdown_files(&path));
        } else if path.extension().is_some_and(|extension| extension == "md") {
            files.push(path);
        }
    }
    files.sort();
    files
}

/// Every way in which the shared blocks or the script copies under `plugin` are out of sync. Each
/// problem names the files, paths, or block involved.
fn sync_problems(plugin: &Path) -> Vec<String> {
    let mut problems = Vec::new();
    let mut found: BTreeMap<String, Vec<(String, Option<String>)>> = BTreeMap::new();
    for path in markdown_files(plugin) {
        let file = path.strip_prefix(plugin).unwrap().display().to_string();
        let text = std::fs::read_to_string(&path).unwrap();
        for block in shared_blocks(&text) {
            if block.content.is_none() {
                problems.push(format!(
                    "{file}:{}: shared block `{}` has no closing marker",
                    block.line, block.name
                ));
            }
            found
                .entry(block.name)
                .or_default()
                .push((file.clone(), block.content));
        }
    }

    for (name, copies) in &found {
        let files: BTreeSet<&str> = copies.iter().map(|(file, _)| file.as_str()).collect();
        if files.len() == 1 {
            problems.push(format!(
                "shared block `{name}` appears only in {}",
                copies[0].0
            ));
        }
        let mut versions: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for (file, content) in copies {
            if let Some(content) = content {
                versions.entry(content).or_default().push(file);
            }
        }
        if versions.len() > 1 {
            let groups: Vec<String> = versions.values().map(|files| files.join(", ")).collect();
            problems.push(format!(
                "copies of shared block `{name}` differ, grouped by text: {}",
                groups.join(" | ")
            ));
        }
    }

    let script = std::fs::read(plugin.join("hooks/knowledge-base.sh")).unwrap();
    for skill in KNOWLEDGE_BASE_SKILLS {
        let skill_file = format!("skills/{skill}/SKILL.md");
        let has_block = found
            .get("knowledge-base")
            .is_some_and(|copies| copies.iter().any(|(file, _)| *file == skill_file));
        if !has_block {
            problems.push(format!(
                "{skill_file} lacks the shared block `knowledge-base`"
            ));
        }
        let copy = format!("skills/{skill}/scripts/knowledge-base.sh");
        match std::fs::read(plugin.join(&copy)) {
            Ok(bytes) if bytes == script => {}
            Ok(_) => problems.push(format!("{copy} differs from hooks/knowledge-base.sh")),
            Err(err) => problems.push(format!("cannot read {copy}: {err}")),
        }
    }
    problems
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).unwrap();
        }
    }
}

/// The problems in a copy of `plugin/` after `change` edits the copy.
fn problems_after(change: impl FnOnce(&Path)) -> Vec<String> {
    let sandbox = Sandbox::new();
    let plugin = sandbox.root.join("plugin");
    copy_dir(&repo_path("plugin"), &plugin);
    change(&plugin);
    sync_problems(&plugin)
}

fn assert_one_problem(problems: &[String], fragment: &str) {
    assert!(
        problems.len() == 1 && problems[0].contains(fragment),
        "expected one problem naming {fragment:?}, got {problems:#?}"
    );
}

fn replace_once(path: &Path, from: &str, to: &str) {
    let text = std::fs::read_to_string(path).unwrap();
    assert_eq!(
        text.matches(from).count(),
        1,
        "{from:?} in {}",
        path.display()
    );
    std::fs::write(path, text.replacen(from, to, 1)).unwrap();
}

#[test]
fn committed_plugin_passes() {
    let problems = sync_problems(&repo_path("plugin"));
    assert!(problems.is_empty(), "{problems:#?}");
}

#[test]
fn block_text_matches_the_spec() {
    assert!(KNOWLEDGE_BASE_BLOCK.contains("bash scripts/knowledge-base.sh index"));
    for skill in KNOWLEDGE_BASE_SKILLS {
        let content = block_content(
            &read(&format!("plugin/skills/{skill}/SKILL.md")),
            "knowledge-base",
        );
        assert_eq!(content.as_deref(), Some(KNOWLEDGE_BASE_BLOCK), "{skill}");
    }
}

#[test]
fn changed_block_fails() {
    let problems = problems_after(|plugin| {
        replace_once(
            &plugin.join("skills/handoff/SKILL.md"),
            "to the user and stop.",
            "to the user and stop!",
        );
    });
    assert_one_problem(&problems, "knowledge-base");
}

#[test]
fn changed_script_copy_fails() {
    let copy = "skills/write-plan/scripts/knowledge-base.sh";
    let problems = problems_after(|plugin| {
        let path = plugin.join(copy);
        let mut bytes = std::fs::read(&path).unwrap();
        bytes.push(b'\n');
        std::fs::write(&path, bytes).unwrap();
    });
    assert_one_problem(&problems, copy);
}

#[test]
fn changed_issue_template_fails() {
    let problems = problems_after(|plugin| {
        replace_once(
            &plugin.join("skills/merge-review/CONVERTER.md"),
            "### [SEC-1]",
            "### [SEC-2]",
        );
    });
    assert_one_problem(&problems, "review-issue-format");
}

#[test]
fn lone_block_name_fails() {
    let problems = problems_after(|plugin| {
        let path = plugin.join("skills/brainstorm/SKILL.md");
        let text = std::fs::read_to_string(&path).unwrap();
        let block = format!("<!-- shared: solo -->\n\nOnly here.\n\n{END_MARKER}\n");
        std::fs::write(&path, format!("{text}\n{block}")).unwrap();
    });
    assert_one_problem(&problems, "solo");
}

#[test]
fn missing_script_copy_fails() {
    let copy = "skills/deep-review/scripts/knowledge-base.sh";
    let problems = problems_after(|plugin| std::fs::remove_file(plugin.join(copy)).unwrap());
    assert_one_problem(&problems, copy);
}

#[test]
fn missing_block_fails() {
    let problems = problems_after(|plugin| {
        let path = plugin.join("skills/resolve-review/SKILL.md");
        let text = std::fs::read_to_string(&path).unwrap();
        let start = text.find("<!-- shared: knowledge-base -->").unwrap();
        let end = start + text[start..].find(END_MARKER).unwrap() + END_MARKER.len();
        std::fs::write(&path, format!("{}{}", &text[..start], &text[end..])).unwrap();
    });
    assert_one_problem(&problems, "resolve-review");
}

#[test]
fn unclosed_marker_fails() {
    let problems = problems_after(|plugin| {
        replace_once(
            &plugin.join("skills/write-spec/SKILL.md"),
            &format!("{END_MARKER}\n"),
            "",
        );
    });
    assert_one_problem(&problems, "skills/write-spec/SKILL.md");
}

fn skill(name: &str) -> String {
    read(&format!("plugin/skills/{name}/SKILL.md"))
}

const SCRATCH_FOLDERS: [&str; 3] = ["tmp/", "temp/", "scratch/"];

#[test]
fn write_spec_saves_to_the_work_folder() {
    assert!(skill("write-spec").contains("`work/<name>/spec-<date>.md`"));
}

#[test]
fn write_plan_saves_to_the_work_folder() {
    assert!(skill("write-plan").contains("`work/<name>/plan-<date>.md`"));
}

#[test]
fn deep_review_saves_to_the_work_folder() {
    assert!(skill("deep-review").contains("`work/<name>/review-<target>-<date>.md`"));
}

#[test]
fn handoff_saves_to_the_handoffs_folder() {
    let text = skill("handoff");
    assert!(text.contains("`handoffs/handoff-<summary>-<date>.md`"));
    assert!(!text.contains("<short-summary>"));
}

const NEWEST_REVIEW_COMMAND: &str = r#"ls -t "<dir>"/work/*/review-*.md | head -1"#;

#[test]
fn merge_review_finds_the_newest_review_with_the_shared_command() {
    assert!(skill("merge-review").contains(NEWEST_REVIEW_COMMAND));
}

#[test]
fn resolve_review_finds_the_newest_review_with_the_shared_command() {
    assert!(skill("resolve-review").contains(NEWEST_REVIEW_COMMAND));
}

#[test]
fn no_folder_search_in_any_skill() {
    for name in KNOWLEDGE_BASE_SKILLS {
        let text = skill(name);
        for word in ["mktemp"].iter().chain(&SCRATCH_FOLDERS) {
            assert!(!text.contains(word), "{name} contains {word:?}");
        }
    }
}

#[test]
fn project_instructions_name_mktemp_and_the_knowledge_base_but_no_scratch_folder() {
    let text = read("CLAUDE.md");
    assert!(text.contains("mktemp"));
    assert!(text.contains("knowledge-base"));
    for folder in SCRATCH_FOLDERS {
        assert!(!text.contains(folder), "CLAUDE.md contains {folder:?}");
    }
}

#[test]
fn the_review_issue_format_block_is_one_fenced_markdown_block() {
    for file in [
        "plugin/skills/deep-review/SKILL.md",
        "plugin/skills/merge-review/CONVERTER.md",
    ] {
        let content = block_content(&read(file), "review-issue-format")
            .unwrap_or_else(|| panic!("{file} has no closed `review-issue-format` block"));
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.first(), Some(&"```markdown"), "{file}");
        assert_eq!(lines.last(), Some(&"```"), "{file}");
    }
}
