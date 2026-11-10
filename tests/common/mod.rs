#![allow(dead_code)]

use std::cell::Cell;
use std::ffi::{OsStr, OsString};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use serde_json::Value;
use tempfile::TempDir;

// Inherited, these would redirect git when a test runs inside a git hook, and would point skald at
// the developer's Claude Code plugins.
const OUTER_VARS: [&str; 4] = [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "CLAUDE_CODE_PLUGIN_CACHE_DIR",
];

pub const ORIGIN: &str = "https://github.com/acme/orders.git";

/// Two entities, Order and Customer, joined by one relation.
pub const MODEL_TOON: &str = "\
entities[2]:
  - name: Order
    description: A customer order
    fields[2]{name,type,nullable}:
      id,uuid,false
      total,decimal,false
    relations[1]{target,cardinality,name}:
      Customer,one,customer
  - name: Customer
    fields[1]{name,type,nullable}:
      id,uuid,false
";

/// An interface record with two operations, the second without a description.
pub const INTERFACE_TOON: &str = "\
operations[2]:
  - name: createOrder
    description: Creates an order for a customer
    inputs[1]{name,type,nullable}:
      customer_id,uuid,false
    outputs[1]{name,type,nullable}:
      order,Order,false
    errors[1]{name,condition}:
      CustomerNotFound,no customer has customer_id
  - name: getOrder
    inputs[1]{name,type,nullable}:
      order_id,uuid,false
    outputs[1]{name,type,nullable}:
      order,Order,false
    errors[1]{name,condition}:
      OrderNotFound,no order has order_id
";

pub fn keys(value: &Value) -> Vec<&str> {
    value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect()
}

/// Submits `contents` as design `name` of `kind`, in JSON or TOON, and returns skald's output.
pub fn submit(sandbox: &Sandbox, dir: &Path, kind: &str, name: &str, contents: &str) -> Value {
    let file = sandbox.file(&format!("{name}.{kind}"), contents);
    let file = file.to_str().unwrap();
    sandbox.json(
        dir,
        &[
            "design", "submit", "--kind", kind, "--name", name, "--file", file,
        ],
    )
}

pub fn submit_id(sandbox: &Sandbox, dir: &Path, kind: &str, name: &str, contents: &str) -> String {
    let submitted = submit(sandbox, dir, kind, name, contents);
    submitted["design"]["id"].as_str().unwrap().to_owned()
}

/// A temporary directory that holds every location skald and git read, so that tests never touch
/// the developer's home directory, store, or git configuration.
pub struct Sandbox {
    _dir: TempDir,
    pub root: PathBuf,
    pub home: PathBuf,
    pub data: PathBuf,
    pub config: PathBuf,
    pub bin: PathBuf,
    /// An empty directory outside any repository, and the default working directory of `cmd`.
    pub work: PathBuf,
    repos: Cell<usize>,
}

impl Sandbox {
    pub fn new() -> Self {
        let dir = TempDir::new().unwrap();
        // git reports resolved paths, so the sandbox must hold resolved paths too.
        let root = dir.path().canonicalize().unwrap();
        let sandbox = Self {
            home: root.join("home"),
            data: root.join("data"),
            config: root.join("claude"),
            bin: root.join("bin"),
            work: root.join("work"),
            root,
            _dir: dir,
            repos: Cell::new(0),
        };
        for dir in [&sandbox.home, &sandbox.bin, &sandbox.work] {
            std::fs::create_dir_all(dir).unwrap();
        }
        // Filtering PATH directories to hide the real `claude` would also hide git and bash when
        // both live beside it, so a failing stub shadows it instead.
        let stub = sandbox.bin.join("claude");
        std::fs::write(
            &stub,
            "#!/bin/sh\necho 'the real claude is blocked in tests' >&2\nexit 1\n",
        )
        .unwrap();
        std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();
        sandbox
    }

    pub fn store_path(&self) -> PathBuf {
        self.data.join("skald.db")
    }

    /// `sandbox.bin` followed by every directory of `outer`, unchanged.
    pub fn path_over(&self, outer: &OsStr) -> OsString {
        let dirs = std::env::split_paths(outer);
        std::env::join_paths(std::iter::once(self.bin.clone()).chain(dirs)).unwrap()
    }

    fn env(&self) -> Vec<(&'static str, OsString)> {
        let path = self.path_over(&std::env::var_os("PATH").unwrap_or_default());
        vec![
            ("HOME", self.home.clone().into()),
            ("SKALD_HOME", self.data.clone().into()),
            ("CLAUDE_CONFIG_DIR", self.config.clone().into()),
            ("PATH", path),
            ("GIT_CONFIG_GLOBAL", "/dev/null".into()),
            ("GIT_CONFIG_SYSTEM", "/dev/null".into()),
            ("GIT_CEILING_DIRECTORIES", self.root.clone().into()),
            ("GIT_AUTHOR_NAME", "Sandbox".into()),
            ("GIT_AUTHOR_EMAIL", "sandbox@example.com".into()),
            ("GIT_COMMITTER_NAME", "Sandbox".into()),
            ("GIT_COMMITTER_EMAIL", "sandbox@example.com".into()),
        ]
    }

    /// `program` with the sandbox environment, run in `dir` with stdin closed. A test that needs
    /// stdin wraps the command in `assert_cmd::Command::from_std` and writes to it.
    pub fn command(&self, program: impl AsRef<OsStr>, dir: &Path) -> Command {
        let mut cmd = Command::new(program);
        cmd.current_dir(dir).envs(self.env()).stdin(Stdio::null());
        for var in OUTER_VARS {
            cmd.env_remove(var);
        }
        cmd
    }

    pub fn cmd(&self) -> Command {
        self.cmd_in(&self.work)
    }

    pub fn cmd_in(&self, dir: &Path) -> Command {
        self.command(env!("CARGO_BIN_EXE_skald"), dir)
    }

    /// Runs `skald <args> --json` in `dir`, asserts exit 0, and parses stdout.
    pub fn json(&self, dir: &Path, args: &[&str]) -> Value {
        let (code, value) = self.json_with_code(dir, args);
        assert_eq!(code, 0, "stdout:\n{value:#}");
        value
    }

    /// Runs `skald <args> --json` in `dir` and returns the exit code and the parsed stdout.
    pub fn json_with_code(&self, dir: &Path, args: &[&str]) -> (i32, Value) {
        let output = self.cmd_in(dir).args(args).arg("--json").output().unwrap();
        let stdout = String::from_utf8(output.stdout).unwrap();
        let value =
            serde_json::from_str(&stdout).unwrap_or_else(|err| panic!("{err} in:\n{stdout}"));
        (output.status.code().unwrap(), value)
    }

    /// Runs `script` with `bash -c` in `dir`.
    pub fn bash(&self, dir: &Path, script: &str) -> Output {
        self.command("bash", dir)
            .args(["-c", script])
            .output()
            .unwrap()
    }

    /// Writes `contents` to a file outside every repository and returns its path.
    pub fn file(&self, name: &str, contents: &str) -> PathBuf {
        let path = self.root.join("files").join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, contents).unwrap();
        path
    }

    pub fn git(&self, dir: &Path, args: &[&str]) -> String {
        let output = self.command("git", dir).args(args).output().unwrap();
        assert!(
            output.status.success(),
            "git {args:?} failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    /// Creates a repository on branch `main` with one commit and no remote.
    pub fn repo_without_origin(&self) -> PathBuf {
        let number = self.repos.get() + 1;
        self.repos.set(number);
        let repo = self.root.join(format!("repo-{number}"));
        std::fs::create_dir_all(&repo).unwrap();
        self.git(&repo, &["init", "--quiet", "--initial-branch=main"]);
        self.git(&repo, &["commit", "--quiet", "--allow-empty", "-m", "init"]);
        repo
    }

    pub fn repo_with_origin(&self, url: &str) -> PathBuf {
        let repo = self.repo_without_origin();
        self.git(&repo, &["remote", "add", "origin", url]);
        repo
    }

    /// Adds a worktree of `repo` on a new branch and returns its path.
    pub fn worktree(&self, repo: &Path, branch: &str) -> PathBuf {
        let path = self.root.join("worktrees").join(branch);
        self.git(
            repo,
            &[
                "worktree",
                "add",
                "--quiet",
                "-b",
                branch,
                path.to_str().unwrap(),
            ],
        );
        path
    }

    /// Creates branch `name` in `repo` and checks it out.
    pub fn checkout_branch(&self, repo: &Path, name: &str) {
        self.git(repo, &["checkout", "--quiet", "-b", name]);
    }
}
