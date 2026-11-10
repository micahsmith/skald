use std::path::Path;

use serde_json::{Map, Value, json};

use crate::adapter::{self, AdapterStatus, PluginStatus};
use crate::design;
use crate::error::SkaldError;
use crate::git;
use crate::output::Output;
use crate::paths;
use crate::store::{self, Store};

const ABOUT: &str = "skald keeps design intent for a repository outside its working tree, keyed to \
                     its git history";

const NEWEST_DESIGNS: usize = 5;

pub fn view(cwd: &Path) -> Result<Output, SkaldError> {
    let (repo, designs, designs_count) = match git::resolve(cwd)? {
        Some(info) => {
            let mut store = Store::open(&paths::data_dir()?)?;
            let (tx, repo) = design::begin_read(&mut store, info)?;
            let (designs, total) = design::summaries(&tx, &repo.id, NEWEST_DESIGNS)?;
            tx.commit()?;
            (Some(repo), designs, total)
        }
        None => (None, Vec::new(), 0),
    };
    let adapters = adapter::adapters(&paths::data_dir()?)?;
    let plugin = adapter::plugin(&paths::claude_plugins_dir()?);

    let mut view = Map::new();
    view.insert("bin".to_owned(), Value::from(bin()));
    view.insert("about".to_owned(), Value::from(ABOUT));
    view.insert("version".to_owned(), Value::from(env!("CARGO_PKG_VERSION")));
    let mut help = Vec::new();
    match &repo {
        Some(repo) => {
            view.insert(
                "repository".to_owned(),
                json!({"identity": repo.identity, "branch": repo.branch}),
            );
            help.push(design::SUBMIT_HELP.to_owned());
        }
        None => {
            view.insert("repository".to_owned(), Value::Null);
            view.insert(
                "note".to_owned(),
                Value::from(format!(
                    "no git repository found at {}; designs are kept per repository",
                    cwd.display()
                )),
            );
            help.push("cd <repository> && skald".to_owned());
        }
    }
    view.insert(
        "designs".to_owned(),
        designs.iter().map(|design| design.item(&[])).collect(),
    );
    view.insert("designs_count".to_owned(), json!(designs_count));
    view.insert("adapters".to_owned(), json!(adapters));
    view.insert("plugin".to_owned(), json!(plugin));
    if !designs.is_empty() {
        help.push("skald design get <id>".to_owned());
    }
    if designs_count > designs.len() {
        help.push("skald design list".to_owned());
    }
    let adapters_current = !adapters.is_empty()
        && adapters
            .iter()
            .all(|adapter| adapter.status == AdapterStatus::Current);
    if !adapters_current || plugin.status == PluginStatus::Missing {
        help.push("skald setup".to_owned());
    }
    if plugin.status == PluginStatus::Outdated {
        help.push(store::UPGRADE.to_owned());
    }
    if let (true, Some(id)) = (plugin.undeclared_minimum, plugin.id()) {
        let marketplace = id
            .split_once('@')
            .map_or("", |(_, marketplace)| marketplace);
        help.push(format!(
            "claude plugin marketplace update {marketplace} && claude plugin update {id}"
        ));
    }
    Ok(Output::new(view).with_help(help))
}

/// What a session hook prints when the home view fails: one line, so that the session still
/// starts.
pub fn unavailable(err: SkaldError) -> Output {
    let mut view = Map::new();
    view.insert(
        "note".to_owned(),
        Value::from(format!("skald cannot show the home view: {}", err.message)),
    );
    Output::new(view)
}

fn bin() -> String {
    match std::env::current_exe() {
        Ok(exe) => paths::collapse_home(&exe, paths::home_dir().as_deref()),
        Err(_) => "skald".to_owned(),
    }
}
