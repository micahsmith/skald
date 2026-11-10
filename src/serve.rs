use std::io::{BufRead, BufReader, Write};
use std::net::IpAddr;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;
use tiny_http::{Header, Response, Server};

use crate::error::SkaldError;

const DEFAULT_IDLE_TIMEOUT: u64 = 300;

const LAUNCHER_WAIT: Duration = Duration::from_secs(2);

/// `skald serve` prints this before the URL, and `design open` reads it back.
const URL_PREFIX: &str = "url: ";

/// Serves `page` at `/` on 127.0.0.1 until no request has arrived for `idle_timeout` seconds.
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
    writeln!(stdout, "{URL_PREFIX}http://127.0.0.1:{port}/")
        .and_then(|()| stdout.flush())
        .map_err(|err| failed(format!("cannot write the URL to stdout: {err}")))?;

    let idle = Duration::from_secs(idle_timeout.unwrap_or(DEFAULT_IDLE_TIMEOUT));
    let html = Header::from_bytes("Content-Type", "text/html; charset=utf-8")
        .expect("the header is valid");
    // Omitting `allow-same-origin` gives the page an opaque origin, so its scripts cannot reach
    // cookies or storage of other servers on 127.0.0.1.
    let sandbox = Header::from_bytes(
        "Content-Security-Policy",
        "sandbox allow-scripts allow-forms allow-popups",
    )
    .expect("the header is valid");
    while let Some(request) = server
        .recv_timeout(idle)
        .map_err(|err| failed(format!("cannot accept a request: {err}")))?
    {
        let foreign_host = request
            .headers()
            .iter()
            .find(|header| header.field.equiv("Host"))
            .is_some_and(|header| !is_local_host(header.value.as_str()));
        let response = if foreign_host {
            Response::from_string("forbidden\n").with_status_code(403)
        } else if is_page(request.url()) {
            Response::from_data(page.as_bytes()).with_header(html.clone())
        } else {
            Response::from_string("not found\n").with_status_code(404)
        };
        let response = response.with_header(sandbox.clone());
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

fn is_page(url: &str) -> bool {
    url.split(['?', '#']).next() == Some("/")
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
    fn only_the_root_path_is_the_page() {
        assert!(is_page("/"));
        assert!(is_page("/?v=1"));
        assert!(!is_page("/nope"));
        assert!(!is_page("/favicon.ico"));
        assert!(!is_page(""));
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
            url_from_line("url: http://127.0.0.1:4242/\n"),
            Some("http://127.0.0.1:4242/")
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
