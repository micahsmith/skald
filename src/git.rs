use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};

use crate::error::SkaldError;

pub struct RepoInfo {
    pub identity: String,
    pub branch: String,
    pub root: PathBuf,
}

/// Identifies the repository that holds `cwd`, or `None` outside a git worktree.
pub fn resolve(cwd: &Path) -> Result<Option<RepoInfo>, SkaldError> {
    let paths = spawn_git(cwd, &["rev-parse", "--show-toplevel", "--git-common-dir"]);
    let url = spawn_git(cwd, &["remote", "get-url", "origin"]);
    let head = spawn_git(cwd, &["symbolic-ref", "--quiet", "--short", "HEAD"]);
    // Collect all three before any early return, so no child is left unreaped.
    let (paths, url, head) = (wait_git(paths), finish_git(url), finish_git(head));
    let paths = paths?;
    if !paths.status.success() {
        let stderr = String::from_utf8_lossy(&paths.stderr).trim().to_owned();
        // `LC_ALL=C` in `spawn_git` keeps this message untranslated.
        if stderr.contains("not a git repository") {
            return Ok(None);
        }
        return Err(SkaldError::new("git-failed", stderr));
    }
    let paths = String::from_utf8_lossy(&paths.stdout).trim().to_owned();
    let mut lines = paths.lines();
    let (Some(root), Some(common_dir)) = (lines.next(), lines.next()) else {
        return Err(SkaldError::new(
            "git-failed",
            format!("git rev-parse printed unexpected output: {paths}"),
        ));
    };
    let identity = match url? {
        Some(url) => normalize_remote(&url),
        None => {
            // Git older than 2.31 lacks `--path-format=absolute` and may print this path relative to `cwd`.
            let common_dir = absolute_from(cwd, common_dir)?;
            // The common directory is shared by every worktree, so its parent is the main worktree.
            common_dir
                .parent()
                .unwrap_or(&common_dir)
                .display()
                .to_string()
        }
    };
    // `rev-parse --abbrev-ref HEAD` fails on a branch with no commits; `symbolic-ref` does not.
    let branch = head?.unwrap_or_else(|| "detached".to_owned());
    Ok(Some(RepoInfo {
        identity,
        branch,
        root: PathBuf::from(root),
    }))
}

/// Reduces a remote URL to `host/path`, so the identity holds no credentials and is the same
/// for the HTTPS and SSH forms of one repository. A non-default port stays in the host part.
/// A local path keeps its form.
fn normalize_remote(url: &str) -> String {
    let hostpath = if let Some((_, rest)) = url.split_once("://") {
        let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
        Some((authority, path))
    } else {
        // Git treats `host:path` as scp-style only when no `/` comes before the first `:`.
        url.split_once(':').filter(|(host, _)| !host.contains('/'))
    };
    let Some((authority, path)) = hostpath else {
        return trim_remote(url).to_owned();
    };
    let host = authority.rsplit('@').next().unwrap_or(authority);
    let path = trim_remote(path.trim_start_matches('/'));
    format!("{}/{path}", host.to_lowercase())
}

fn trim_remote(path: &str) -> &str {
    let trimmed = path.trim_end_matches('/');
    let trimmed = trimmed.strip_suffix(".git").unwrap_or(trimmed);
    if trimmed.is_empty() { path } else { trimmed }
}

fn absolute_from(cwd: &Path, path: &str) -> Result<PathBuf, SkaldError> {
    cwd.join(path).canonicalize().map_err(|err| {
        SkaldError::new(
            "git-failed",
            format!("cannot resolve git directory {path}: {err}"),
        )
    })
}

/// The target of `origin/HEAD`, otherwise `main` if that branch exists, otherwise `master`.
pub fn default_branch(root: &Path) -> Result<String, SkaldError> {
    let origin_head = git(
        root,
        &[
            "symbolic-ref",
            "--quiet",
            "--short",
            "refs/remotes/origin/HEAD",
        ],
    )?;
    if let Some(branch) = origin_head
        .as_deref()
        .and_then(|target| target.strip_prefix("origin/"))
    {
        return Ok(branch.to_owned());
    }
    let has_main = git(
        root,
        &["rev-parse", "--verify", "--quiet", "refs/heads/main"],
    )?;
    Ok(if has_main.is_some() { "main" } else { "master" }.to_owned())
}

fn git(dir: &Path, args: &[&str]) -> Result<Option<String>, SkaldError> {
    finish_git(spawn_git(dir, args))
}

// Piped stderr keeps git's messages out of skald's own stderr, which hooks forward.
fn spawn_git(dir: &Path, args: &[&str]) -> Result<Child, SkaldError> {
    Command::new("git")
        .current_dir(dir)
        .env("LC_ALL", "C")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| SkaldError::new("git-unavailable", format!("cannot run git: {err}")))
}

/// Returns `None` when git exits non-zero.
fn finish_git(child: Result<Child, SkaldError>) -> Result<Option<String>, SkaldError> {
    let output = wait_git(child)?;
    if !output.status.success() {
        return Ok(None);
    }
    Ok(Some(
        String::from_utf8_lossy(&output.stdout).trim().to_owned(),
    ))
}

fn wait_git(child: Result<Child, SkaldError>) -> Result<Output, SkaldError> {
    child?
        .wait_with_output()
        .map_err(|err| SkaldError::new("git-unavailable", format!("cannot run git: {err}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_remote_reduces_urls_to_host_and_path() {
        let cases = [
            (
                "https://github.com/acme/orders.git",
                "github.com/acme/orders",
            ),
            ("https://github.com/acme/orders", "github.com/acme/orders"),
            (
                "https://alice:ghp_SECRET@GitHub.com/acme/Orders.git",
                "github.com/acme/Orders",
            ),
            (
                "https://token@github.com/acme/orders/",
                "github.com/acme/orders",
            ),
            (
                "https://github.com/acme/orders.git/",
                "github.com/acme/orders",
            ),
            (
                "ssh://git@Example.com:2222/org/repo.git",
                "example.com:2222/org/repo",
            ),
            ("git@github.com:acme/orders.git", "github.com/acme/orders"),
            ("github.com:acme/orders", "github.com/acme/orders"),
            ("/srv/git/repo.git", "/srv/git/repo"),
            ("../repo/", "../repo"),
            ("./a:b/repo", "./a:b/repo"),
        ];
        for (url, identity) in cases {
            assert_eq!(normalize_remote(url), identity, "{url}");
        }
    }

    #[test]
    fn absolute_from_joins_relative_and_keeps_absolute() {
        let dir = std::env::temp_dir().join(format!("skald-abs-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("a/.git")).unwrap();
        let dir = dir.canonicalize().unwrap();
        assert_eq!(
            absolute_from(&dir.join("a"), ".git").unwrap(),
            dir.join("a/.git")
        );
        let abs = dir.join("a/.git");
        assert_eq!(absolute_from(&dir, abs.to_str().unwrap()).unwrap(), abs);
        assert!(absolute_from(&dir, "missing").is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
