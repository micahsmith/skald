use std::cmp::Ordering;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::SkaldError;

const PLUGIN_PREFIX: &str = "code-skillset@";

/// What `skald setup` records about an installed runtime adapter.
#[derive(Debug, Deserialize, Serialize)]
pub struct Marker {
    pub runtime: String,
    pub version: String,
    pub command: String,
    pub settings: String,
    pub installed_at: String,
}

pub fn marker_path(data_dir: &Path, runtime: &str) -> PathBuf {
    data_dir.join("adapters").join(format!("{runtime}.json"))
}

#[derive(Debug, Serialize)]
pub struct Adapter {
    runtime: String,
    version: Option<String>,
    pub status: AdapterStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AdapterStatus {
    Current,
    Outdated,
    Newer,
    Unknown,
}

#[derive(Debug, Serialize)]
pub struct Plugin {
    id: Option<String>,
    min_skald_version: Option<String>,
    pub status: PluginStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
    /// The manifest was read and has no `skaldMinVersion`, as in installs from before the key.
    #[serde(skip)]
    pub undeclared_minimum: bool,
}

impl Plugin {
    pub fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    fn unidentified(status: PluginStatus, detail: String) -> Self {
        Self {
            id: None,
            min_skald_version: None,
            status,
            detail: Some(detail),
            undeclared_minimum: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginStatus {
    Current,
    Outdated,
    Missing,
    Unknown,
}

/// The code-skillset plugin as Claude Code recorded its installation.
pub struct Installed {
    pub id: String,
    install_path: Option<PathBuf>,
}

/// The contents of `path`, or `None` when it does not exist.
pub fn read_if_present(path: &Path) -> Result<Option<String>, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
        Err(err) => Err(format!("cannot read {}: {err}", path.display())),
    }
}

fn program_version() -> Version {
    Version::parse(env!("CARGO_PKG_VERSION")).expect("Cargo.toml holds a semver version")
}

/// Every adapter marker in `data_dir`, in file name order.
pub fn adapters(data_dir: &Path) -> Result<Vec<Adapter>, SkaldError> {
    let dir = data_dir.join("adapters");
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => {
            return Err(SkaldError::new(
                "adapters-unavailable",
                format!("cannot read {}: {err}", dir.display()),
            ));
        }
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|entry| Some(entry.ok()?.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    paths.sort();
    let program = program_version();
    Ok(paths.iter().map(|path| adapter(path, &program)).collect())
}

fn adapter(path: &Path, program: &Version) -> Adapter {
    let marker = std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Marker>(&text).ok());
    match marker {
        Some(marker) => Adapter {
            status: adapter_status(&marker.version, program),
            runtime: marker.runtime,
            version: Some(marker.version),
        },
        None => Adapter {
            runtime: path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            version: None,
            status: AdapterStatus::Unknown,
        },
    }
}

fn adapter_status(version: &str, program: &Version) -> AdapterStatus {
    match Version::parse(version).map(|version| version.cmp(program)) {
        Ok(Ordering::Less) => AdapterStatus::Outdated,
        Ok(Ordering::Equal) => AdapterStatus::Current,
        Ok(Ordering::Greater) => AdapterStatus::Newer,
        Err(_) => AdapterStatus::Unknown,
    }
}

/// Finds the first `code-skillset@` plugin in `<plugins_dir>/installed_plugins.json`.
pub fn installed_plugin(plugins_dir: &Path) -> Result<Option<Installed>, String> {
    let path = plugins_dir.join("installed_plugins.json");
    let Some(text) = read_if_present(&path)? else {
        return Ok(None);
    };
    let record: Value = serde_json::from_str(&text)
        .map_err(|err| format!("cannot parse {}: {err}", path.display()))?;
    let found = record["plugins"]
        .as_object()
        .and_then(|plugins| plugins.iter().find(|(id, _)| id.starts_with(PLUGIN_PREFIX)));
    let Some((id, entry)) = found else {
        return Ok(None);
    };
    // Version 2 of the file lists every installation of a plugin; version 1 holds one.
    let installation = match entry {
        Value::Array(installations) => installations.first(),
        installation => Some(installation),
    };
    Ok(Some(Installed {
        id: id.clone(),
        install_path: installation
            .and_then(|installation| installation["installPath"].as_str())
            .map(PathBuf::from),
    }))
}

pub fn plugin(plugins_dir: &Path) -> Plugin {
    let installed = match installed_plugin(plugins_dir) {
        Ok(Some(installed)) => installed,
        Ok(None) => {
            return Plugin::unidentified(
                PluginStatus::Missing,
                "no code-skillset plugin is installed".to_owned(),
            );
        }
        Err(message) => return Plugin::unidentified(PluginStatus::Unknown, message),
    };
    let manifest = installed.install_path.as_deref().and_then(read_manifest);
    let min = manifest.as_ref().and_then(min_skald_version);
    let undeclared_minimum = manifest.is_some() && min.is_none();
    let (status, detail) = match &min {
        Some(min) => plugin_status(&installed.id, min, &program_version()),
        None => (
            PluginStatus::Unknown,
            Some(format!(
                "{} declares no minimum skald version",
                installed.id
            )),
        ),
    };
    Plugin {
        id: Some(installed.id),
        min_skald_version: min,
        status,
        detail,
        undeclared_minimum,
    }
}

fn read_manifest(install_path: &Path) -> Option<Value> {
    let text = std::fs::read_to_string(install_path.join(".claude-plugin/plugin.json")).ok()?;
    serde_json::from_str(&text).ok()
}

fn min_skald_version(manifest: &Value) -> Option<String> {
    manifest["metadata"]["skaldMinVersion"]
        .as_str()
        .map(String::from)
}

fn plugin_status(id: &str, min: &str, program: &Version) -> (PluginStatus, Option<String>) {
    match Version::parse(min) {
        Ok(min) if *program >= min => (PluginStatus::Current, None),
        Ok(min) => (
            PluginStatus::Outdated,
            Some(format!(
                "{id} needs skald {min} or newer, but this is skald {program}"
            )),
        ),
        Err(err) => (
            PluginStatus::Unknown,
            Some(format!(
                "{id} declares an invalid minimum skald version {min}: {err}"
            )),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(text: &str) -> Version {
        Version::parse(text).unwrap()
    }

    #[test]
    fn adapter_status_compares_the_marker_with_the_program() {
        let program = version("0.2.0");
        assert_eq!(adapter_status("0.1.9", &program), AdapterStatus::Outdated);
        assert_eq!(adapter_status("0.2.0", &program), AdapterStatus::Current);
        assert_eq!(adapter_status("0.10.0", &program), AdapterStatus::Newer);
        assert_eq!(adapter_status("two", &program), AdapterStatus::Unknown);
    }

    #[test]
    fn plugin_status_compares_the_minimum_with_the_program() {
        let program = version("0.2.0");
        let status = |min| plugin_status("code-skillset@skald", min, &program);
        assert_eq!(status("0.2.0"), (PluginStatus::Current, None));
        assert_eq!(status("0.1.0"), (PluginStatus::Current, None));
        assert_eq!(
            status("0.10.0"),
            (
                PluginStatus::Outdated,
                Some(
                    "code-skillset@skald needs skald 0.10.0 or newer, but this is skald 0.2.0"
                        .to_owned()
                )
            )
        );
        assert_eq!(status("latest").0, PluginStatus::Unknown);
    }
}
