use std::io::{BufRead, BufReader, Write};
use std::net::IpAddr;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;
use tiny_http::{Header, Response, Server};

use crate::design;
use crate::error::SkaldError;
use crate::id;
use crate::output::Output;
use crate::paths;
use crate::store::Store;

const DEFAULT_IDLE_TIMEOUT: u64 = 300;

const LAUNCHER_WAIT: Duration = Duration::from_secs(2);

/// `skald serve` prints this before the URL, and `design open` reads it back.
const URL_PREFIX: &str = "url: ";

/// Serves a stored design version, or the page on stdin when `page_from_stdin` is set.
pub fn run(
    design: Option<String>,
    version: Option<u32>,
    page_from_stdin: bool,
    port: u16,
    idle_timeout: Option<u64>,
    stdout: &mut impl Write,
) -> Result<Output, SkaldError> {
    let page = if page_from_stdin {
        std::io::read_to_string(std::io::stdin())
            .map_err(|err| failed(format!("cannot read the page: {err}")))?
    } else {
        let (design, version) = design
            .zip(version)
            .expect("clap requires --design and --version without --page-from-stdin");
        let cwd = std::env::current_dir().map_err(|err| {
            SkaldError::new(
                "no-working-directory",
                format!("cannot read the working directory: {err}"),
            )
        })?;
        let mut store = Store::open(&paths::data_dir()?)?;
        design::served(&mut store, &cwd, &design, Some(version))?.1
    };
    serve(page, port, idle_timeout, stdout)?;
    Ok(Output::Raw(String::new()))
}

/// Serves `page` at a secret path on 127.0.0.1 until no request for that path has arrived for
/// `idle_timeout` seconds. Every other path answers 404 and does not extend the server's life.
/// The URL is written to `stdout` as one line before the first request is taken.
pub fn serve(
    page: String,
    port: u16,
    idle_timeout: Option<u64>,
    stdout: &mut impl Write,
) -> Result<(), SkaldError> {
    let server = Server::http(("127.0.0.1", port)).map_err(|err| {
        failed(format!("cannot listen on 127.0.0.1:{port}: {err}"))
            .with_help("skald serve --design <id> --version <n> --port 0")
    })?;
    let port = server
        .server_addr()
        .to_ip()
        .expect("a TCP server has an IP address")
        .port();
    let page_path = format!("/{}/", id::random(32));
    writeln!(stdout, "{URL_PREFIX}http://127.0.0.1:{port}{page_path}")
        .and_then(|()| stdout.flush())
        .map_err(|err| failed(format!("cannot write the URL to stdout: {err}")))?;

    let idle = Duration::from_secs(idle_timeout.unwrap_or(DEFAULT_IDLE_TIMEOUT));
    let html = Header::from_bytes("Content-Type", "text/html; charset=utf-8")
        .expect("the header is valid");
    let text = Header::from_bytes("Content-Type", "text/plain; charset=UTF-8")
        .expect("the header is valid");
    // Omitting `allow-same-origin` gives the page an opaque origin, so its scripts cannot reach
    // cookies or storage of other servers on 127.0.0.1. The remaining directives block every
    // remote load and form post, so a page cannot send data off the machine.
    let sandbox = Header::from_bytes(
        "Content-Security-Policy",
        "sandbox allow-scripts allow-popups; default-src 'none'; script-src 'unsafe-inline'; \
         style-src 'unsafe-inline'; img-src data:; form-action 'none'",
    )
    .expect("the header is valid");
    let mut deadline = Instant::now() + idle;
    while let Some(request) = server
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .map_err(|err| failed(format!("cannot accept a request: {err}")))?
    {
        let foreign_host = request
            .headers()
            .iter()
            .find(|header| header.field.equiv("Host"))
            .is_some_and(|header| !is_local_host(header.value.as_str()));
        let (status, content_type, body) = if foreign_host {
            (403, &text, &b"forbidden\n"[..])
        } else if is_page(request.url(), &page_path) {
            deadline = Instant::now() + idle;
            (200, &html, page.as_bytes())
        } else {
            (404, &text, &b"not found\n"[..])
        };
        let response = Response::new(
            status.into(),
            vec![content_type.clone(), sandbox.clone()],
            body,
            Some(body.len()),
            None,
        );
        // A client that hangs up early must not stop the server.
        let _ = request.respond(response);
    }
    Ok(())
}

/// A browser sends the attacker's own host name in a DNS rebinding request, so only IP literals and
/// `localhost` (with or without a trailing dot) are local.
fn is_local_host(authority: &str) -> bool {
    let host = match authority.strip_prefix('[') {
        Some(rest) => rest.split_once(']').map_or(rest, |(host, _)| host),
        None => authority
            .rsplit_once(':')
            .map_or(authority, |(host, _)| host),
    };
    host.parse::<IpAddr>().is_ok() || host.trim_end_matches('.').eq_ignore_ascii_case("localhost")
}

fn is_page(url: &str, page_path: &str) -> bool {
    url.split(['?', '#']).next() == Some(page_path)
}

/// Starts `skald serve` for `page` in the background and returns its URL.
pub fn spawn(page: &str, idle_timeout: Option<u64>) -> Result<String, SkaldError> {
    let exe = std::env::current_exe()
        .map_err(|err| failed(format!("cannot find the skald executable: {err}")))?;
    let mut cmd = Command::new(exe);
    cmd.args(["serve", "--page-from-stdin"]);
    if let Some(seconds) = idle_timeout {
        cmd.args(["--idle-timeout", &seconds.to_string()]);
    }
    // In JSON, an error the server prints before its URL fits on the one line read below.
    cmd.arg("--json")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    detach(&mut cmd);
    let mut child = cmd
        .spawn()
        .map_err(|err| failed(format!("cannot start the local server: {err}")))?;
    // A child that exits early closes the pipe. The write error is ignored so that the child's
    // own error line is still read below.
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(page.as_bytes());
    }

    let mut line = String::new();
    BufReader::new(child.stdout.take().expect("stdout is piped"))
        .read_line(&mut line)
        .map_err(|err| failed(format!("cannot read from the local server: {err}")))?;
    if let Some(url) = url_from_line(&line) {
        return Ok(url.to_owned());
    }
    let _ = child.kill();
    let _ = child.wait();
    let reason = serde_json::from_str::<Value>(&line)
        .ok()
        .and_then(|value| value["error"]["message"].as_str().map(str::to_owned))
        .unwrap_or_else(|| "it exited without printing a URL".to_owned());
    Err(failed(format!("the local server did not start: {reason}")))
}

fn url_from_line(line: &str) -> Option<&str> {
    line.strip_prefix(URL_PREFIX)
        .map(str::trim_end)
        .filter(|url| url.starts_with("http://"))
}

/// Opens `url` in the default browser, and reports `launched` or `unavailable`.
pub fn launch_browser(url: &str) -> &'static str {
    let (program, args) = launcher(url);
    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    detach(&mut cmd);
    let Ok(mut child) = cmd.spawn() else {
        return "unavailable";
    };
    // A launcher still running at the deadline is left alone: `xdg-open` can stay in the
    // foreground for as long as the browser it started.
    let deadline = Instant::now() + LAUNCHER_WAIT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return if status.success() {
                    "launched"
                } else {
                    "unavailable"
                };
            }
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            _ => return "launched",
        }
    }
}

fn launcher(url: &str) -> (&'static str, Vec<&str>) {
    if cfg!(target_os = "macos") {
        ("open", vec![url])
    } else if cfg!(windows) {
        // `start` takes its first quoted argument as a window title, so the title comes first.
        ("cmd", vec!["/c", "start", "", url])
    } else {
        ("xdg-open", vec![url])
    }
}

/// Moves a child out of the caller's process group, so that it outlives a terminal or an agent
/// shell that signals the whole group once `design open` returns.
fn detach(cmd: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
}

fn failed(message: String) -> SkaldError {
    SkaldError::new("serve-failed", message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_secret_path_is_the_page() {
        assert!(is_page("/abc/", "/abc/"));
        assert!(is_page("/abc/?v=1", "/abc/"));
        assert!(!is_page("/", "/abc/"));
        assert!(!is_page("/abc", "/abc/"));
        assert!(!is_page("/nope", "/abc/"));
        assert!(!is_page("", "/abc/"));
    }

    #[test]
    fn only_ip_literals_and_localhost_are_local_hosts() {
        for host in [
            "127.0.0.1",
            "127.0.0.1:80",
            "[::1]:8080",
            "[::1]",
            "localhost",
            "LocalHost:1",
            "localhost.:9",
        ] {
            assert!(is_local_host(host), "{host}");
        }
        for host in [
            "attacker.example",
            "attacker.example:80",
            "127.0.0.1.evil.example",
            "",
            "localhost.evil.example",
        ] {
            assert!(!is_local_host(host), "{host}");
        }
    }

    #[test]
    fn url_is_read_from_the_first_line() {
        assert_eq!(
            url_from_line("url: http://127.0.0.1:4242/abc/\n"),
            Some("http://127.0.0.1:4242/abc/")
        );
        assert_eq!(url_from_line("{\"error\":{}}\n"), None);
        assert_eq!(url_from_line("url: \n"), None);
        assert_eq!(url_from_line(""), None);
    }

    #[test]
    fn launcher_receives_the_url_last() {
        let url = "http://127.0.0.1:4242/";
        let (program, args) = launcher(url);
        assert!(["open", "cmd", "xdg-open"].contains(&program));
        assert_eq!(args.last(), Some(&url));
    }
}
