use std::path::{Path, PathBuf};

use crate::error::SkaldError;

pub fn home_dir() -> Option<PathBuf> {
    env_dir("HOME")
}

fn env_dir(var: &str) -> Option<PathBuf> {
    std::env::var_os(var)
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
}

/// `$SKALD_HOME` when set, otherwise `~/.skald`.
pub fn data_dir() -> Result<PathBuf, SkaldError> {
    env_dir("SKALD_HOME")
        .or_else(|| home_dir().map(|home| home.join(".skald")))
        .ok_or_else(|| {
            SkaldError::new(
                "no-data-directory",
                "neither SKALD_HOME nor HOME is set, so skald has no data directory",
            )
            .with_help("SKALD_HOME=<path> skald")
        })
}

/// Creates `dir` and any missing parents. On Unix, each directory this call creates gets mode
/// 0700; a directory that already exists keeps its mode.
pub fn create_data_dir(dir: &Path) -> std::io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(dir)
}

/// `$CLAUDE_CONFIG_DIR` when set, otherwise `~/.claude`.
pub fn claude_config_dir() -> Result<PathBuf, SkaldError> {
    env_dir("CLAUDE_CONFIG_DIR")
        .or_else(|| home_dir().map(|home| home.join(".claude")))
        .ok_or_else(|| {
            SkaldError::new(
                "no-claude-config-directory",
                "neither CLAUDE_CONFIG_DIR nor HOME is set, so skald cannot find Claude Code",
            )
            .with_help("CLAUDE_CONFIG_DIR=<path> skald setup")
        })
}

/// `$CLAUDE_CODE_PLUGIN_CACHE_DIR` when set, otherwise `plugins` in the Claude Code config
/// directory.
pub fn claude_plugins_dir() -> Result<PathBuf, SkaldError> {
    match env_dir("CLAUDE_CODE_PLUGIN_CACHE_DIR") {
        Some(dir) => Ok(dir),
        None => Ok(claude_config_dir()?.join("plugins")),
    }
}

/// The first file named `name` on `PATH` that may be executed.
pub fn find_executable(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| is_executable(candidate))
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

pub fn collapse_home(path: &Path, home: Option<&Path>) -> String {
    match home.and_then(|home| path.strip_prefix(home).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".to_owned(),
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_prefix_collapses_to_tilde() {
        let home = Path::new("/home/ada");
        let collapse = |path: &str| collapse_home(Path::new(path), Some(home));
        assert_eq!(collapse("/home/ada/.cargo/bin/skald"), "~/.cargo/bin/skald");
        assert_eq!(collapse("/home/ada"), "~");
        assert_eq!(collapse("/home/adam/bin/skald"), "/home/adam/bin/skald");
        assert_eq!(collapse("/usr/bin/skald"), "/usr/bin/skald");
        assert_eq!(
            collapse_home(Path::new("/usr/bin/skald"), None),
            "/usr/bin/skald"
        );
    }
}
