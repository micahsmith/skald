mod common;

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use assert_cmd::assert::OutputAssertExt;
use common::{ORIGIN, Sandbox};
use rusqlite::Connection;
use serde_json::Value;

const BIN: &str = env!("CARGO_BIN_EXE_skald");
const DEADLINE: Duration = Duration::from_secs(5);

const MODEL_V1: &str = "\
entities[1]:
  - name: Order
    fields[1]{name,type,nullable}:
      id,uuid,false
";

const MODEL_V2: &str = "\
entities[1]:
  - name: Order
    fields[2]{name,type,nullable}:
      id,uuid,false
      total,decimal,false
";

/// Every command a user runs, labelled, with its documented exit code.
const EVERY_COMMAND: [(&str, i32); 10] = [
    ("skald", 0),
    ("setup --no-plugin", 0),
    ("hook claude-code", 0),
    ("design submit", 0),
    ("design submit version 2", 0),
    ("design list", 0),
    ("design get", 0),
    ("design diff", 0),
    ("design render", 0),
    ("design open --no-browser", 0),
];

// A new network namespace starts with loopback down, but an offline machine keeps loopback, which
// `design open` serves on. 0x8914 is SIOCSIFFLAGS and 1 is IFF_UP, from <linux/sockios.h> and
// <linux/if.h>.
const LOOPBACK_UP: &str = r#"ip link set lo up 2>/dev/null || python3 -c 'import fcntl, socket, struct; fcntl.ioctl(socket.socket(socket.AF_INET, socket.SOCK_DGRAM), 0x8914, struct.pack("16sH22x", b"lo", 1))' 2>/dev/null"#;

/// Runs `cmd` and returns its exit code and stdout. The test fails when `cmd` outlives
/// `DEADLINE`.
fn run_within_deadline(cmd: &mut Command, label: &str) -> (i32, String) {
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if start.elapsed() > DEADLINE {
            let _ = child.kill();
            let _ = child.wait();
            panic!("`{label}` ran longer than {DEADLINE:?}");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let mut stdout = String::new();
    let mut stderr = String::new();
    child.stdout.unwrap().read_to_string(&mut stdout).unwrap();
    child.stderr.unwrap().read_to_string(&mut stderr).unwrap();
    let code = status
        .code()
        .unwrap_or_else(|| panic!("`{label}` was killed by a signal; stderr:\n{stderr}"));
    (code, stdout)
}

fn submit_args(file: &Path) -> [&str; 8] {
    let file = file.to_str().unwrap();
    [
        "design",
        "submit",
        "--kind",
        "data-model",
        "--name",
        "orders",
        "--file",
        file,
    ]
}

/// Runs every command of `EVERY_COMMAND` with `--json` in a fresh repository, each started by
/// `launch`, and returns each label with its exit code.
fn run_every_command(launch: impl Fn(&Sandbox, &Path) -> Command) -> Vec<(&'static str, i32)> {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    let v1 = sandbox.file("model.toon", MODEL_V1);
    let v2 = sandbox.file("model-v2.toon", MODEL_V2);
    let page = sandbox.root.join("orders.html");
    let mut codes = Vec::new();
    let mut run = |label: &'static str, args: &[&str]| -> Value {
        let (code, stdout) =
            run_within_deadline(launch(&sandbox, &repo).args(args).arg("--json"), label);
        codes.push((label, code));
        serde_json::from_str(&stdout).unwrap_or(Value::Null)
    };
    run("skald", &[]);
    run("setup --no-plugin", &["setup", "--no-plugin"]);
    run("hook claude-code", &["hook", "claude-code"]);
    let submitted = run("design submit", &submit_args(&v1));
    let id = submitted["design"]["id"].as_str().unwrap_or("d-missing");
    run("design submit version 2", &submit_args(&v2));
    run("design list", &["design", "list"]);
    run("design get", &["design", "get", id]);
    run(
        "design diff",
        &["design", "diff", id, "--from", "1", "--to", "2"],
    );
    run(
        "design render",
        &["design", "render", id, "--out", page.to_str().unwrap()],
    );
    run(
        "design open --no-browser",
        &["design", "open", id, "--no-browser", "--idle-timeout", "1"],
    );
    codes
}

#[test]
fn binary_runs_alone() {
    let output = Command::new(BIN)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .arg("--version")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        concat!(env!("CARGO_PKG_VERSION"), "\n")
    );

    let (tool, args): (&str, &[&str]) = if cfg!(target_os = "macos") {
        ("otool", &["-L"])
    } else {
        ("ldd", &[])
    };
    let Ok(listing) = Command::new(tool).args(args).arg(BIN).output() else {
        println!("skipped linkage check: {tool} unavailable");
        return;
    };
    let listing = String::from_utf8_lossy(&listing.stdout);
    let foreign: Vec<&str> = linked_libraries(&listing)
        .into_iter()
        .filter(|library| !is_system_library(library))
        .collect();
    assert!(
        foreign.is_empty(),
        "skald links libraries outside the C runtime: {foreign:?}\n{listing}"
    );
}

/// The library paths or names in `ldd` or `otool -L` output.
fn linked_libraries(listing: &str) -> Vec<&str> {
    listing
        .lines()
        .filter(|line| line.starts_with(char::is_whitespace))
        .filter_map(|line| line.split_whitespace().next())
        .filter(|library| !["statically", "not"].contains(library))
        .collect()
}

fn is_system_library(library: &str) -> bool {
    const C_RUNTIME: [&str; 12] = [
        "linux-vdso.so",
        "linux-gate.so",
        "ld-linux",
        "ld-musl",
        "libc.so",
        "libc.musl",
        "libm.so",
        "libgcc_s.so",
        "libpthread.so",
        "libdl.so",
        "librt.so",
        "libutil.so",
    ];
    if cfg!(target_os = "macos") {
        return library.starts_with("/usr/lib/") || library.starts_with("/System/Library/");
    }
    let name = Path::new(library)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(library);
    C_RUNTIME.iter().any(|prefix| name.starts_with(prefix))
}

#[test]
fn home_view_with_1000_designs_is_fast() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    let seeded = sandbox.json(&repo, &["dev", "seed", "--designs", "1000"]);
    assert_eq!(seeded["seeded"]["designs"], 1000);

    let start = Instant::now();
    let assert = sandbox.cmd_in(&repo).arg("--json").assert().success();
    let elapsed = start.elapsed();
    println!("home view with 1000 designs: {elapsed:?}");
    assert!(
        elapsed < Duration::from_secs(1),
        "the home view took {elapsed:?}"
    );
    let home: Value = serde_json::from_slice(&assert.get_output().stdout).unwrap();
    assert_eq!(home["designs_count"], 1000);
    assert!(home["designs"].as_array().unwrap().len() <= 5, "{home:#}");
}

#[test]
fn every_command_works_with_stdin_closed() {
    assert_eq!(
        run_every_command(|sandbox, dir| sandbox.cmd_in(dir)),
        EVERY_COMMAND
    );
}

#[test]
fn every_record_table_carries_a_non_null_repository() {
    let sandbox = Sandbox::new();
    let repo = sandbox.repo_with_origin(ORIGIN);
    sandbox.json(&repo, &["dev", "seed", "--designs", "3"]);

    let conn = Connection::open(sandbox.store_path()).unwrap();
    let tables: Vec<String> = conn
        .prepare(
            "SELECT name FROM sqlite_master
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name != 'repository'
             ORDER BY name",
        )
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert!(!tables.is_empty());

    let count = |sql: String| -> i64 { conn.query_row(&sql, [], |row| row.get(0)).unwrap() };
    for table in &tables {
        let notnull: Option<bool> = conn
            .query_row(
                "SELECT \"notnull\" FROM pragma_table_info(?1) WHERE name = 'repository'",
                [table],
                |row| row.get(0),
            )
            .ok();
        assert_eq!(notnull, Some(true), "column {table}.repository");
        assert!(
            count(format!("SELECT count(*) FROM {table}")) > 0,
            "{table} holds no rows after seeding"
        );
        assert_eq!(
            count(format!(
                "SELECT count(*) FROM {table} WHERE repository IS NULL"
            )),
            0,
            "{table}"
        );
        assert_eq!(
            count(format!(
                "SELECT count(*) FROM {table} WHERE repository NOT IN (SELECT id FROM repository)"
            )),
            0,
            "{table} rows name an unknown repository"
        );
    }
}

/// Why `unshare -rn` cannot stand in for an offline machine here, if it cannot.
fn offline_unavailable() -> Option<String> {
    let probe = |script: &str| {
        Command::new("unshare")
            .args(["-rn", "sh", "-c", script])
            .stdin(Stdio::null())
            .output()
    };
    match probe("true") {
        Err(_) => return Some("unshare unavailable".to_owned()),
        Ok(output) if !output.status.success() => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Some(format!("unshare -rn is not permitted: {}", stderr.trim()));
        }
        Ok(_) => {}
    }
    match probe(LOOPBACK_UP) {
        Ok(output) if output.status.success() => None,
        _ => Some(
            "loopback cannot be brought up inside unshare -rn without ip or python3".to_owned(),
        ),
    }
}

#[test]
fn commands_succeed_without_network() {
    if let Some(reason) = offline_unavailable() {
        assert!(
            std::env::var("SKALD_REQUIRE_OFFLINE_TEST").as_deref() != Ok("1"),
            "SKALD_REQUIRE_OFFLINE_TEST=1 but the offline test cannot run: {reason}"
        );
        println!("skipped: {reason}");
        return;
    }
    let script = format!("{LOOPBACK_UP} && exec \"$@\"");
    let offline = run_every_command(|sandbox, dir| {
        let mut cmd = sandbox.command("unshare", dir);
        cmd.args(["-rn", "sh", "-c", &script, "sh", BIN]);
        cmd
    });
    let online = run_every_command(|sandbox, dir| sandbox.cmd_in(dir));
    assert_eq!(offline, online);
    assert_eq!(offline, EVERY_COMMAND);
}
