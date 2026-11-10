mod check;
mod data_model;
mod diff;
mod interface;
mod render;

use std::path::Path;

use clap::ValueEnum;
use clap::builder::PossibleValue;
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};

use crate::cli::DesignCommand;
use crate::error::SkaldError;
use crate::git::{self, RepoInfo};
use crate::id;
use crate::output::{self, Output};
use crate::repository::RepoContext;
use crate::serve;
use crate::store::{self, Store};
use data_model::DataModelRecord;
use diff::{Diff, Diffable};
use interface::InterfaceRecord;
use render::DesignHeader;

pub const SUBMIT_HELP: &str = "skald design submit --kind data-model --name <name> --file <path>";

/// Fields a list item may carry. The first four are shown by default.
pub(crate) const FIELDS: [&str; 7] = [
    "id",
    "name",
    "kind",
    "version",
    "branch",
    "created_at",
    "submitted_at",
];

const DEFAULT_LIMIT: usize = 50;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    DataModel,
    Interface,
    Freeform,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DataModel => "data-model",
            Self::Interface => "interface",
            Self::Freeform => "freeform",
        }
    }
}

impl ValueEnum for Kind {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::DataModel, Self::Interface, Self::Freeform]
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        let help = match self {
            Self::DataModel => "Entities, attributes, and relations",
            Self::Interface => "Operations, inputs, and outputs",
            Self::Freeform => "An HTML page with no fixed structure",
        };
        Some(PossibleValue::new(self.as_str()).help(help))
    }
}

impl FromSql for Kind {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let text = value.as_str()?;
        Self::from_str(text, false)
            .map_err(|_| FromSqlError::Other(format!("unknown design kind `{text}`").into()))
    }
}

impl ToSql for Kind {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        self.as_str().to_sql()
    }
}

/// A record kind whose submissions are typed, validated, and stored as canonical JSON.
trait Typed: Diffable + Serialize + DeserializeOwned {
    type Submission: DeserializeOwned;

    /// Validates `submission` and assigns ids to new elements, keeping the ids of `previous`, the
    /// newest stored record of the same design. Problems are listed in document order, each
    /// element's own before its children's.
    fn validate(
        submission: &Self::Submission,
        previous: Option<&Self>,
    ) -> Result<Self, Vec<Problem>>;
}

/// One reason a submission was rejected. `path` is a JSON pointer into the submission.
#[derive(Debug, PartialEq, Serialize)]
pub struct Problem {
    pub path: String,
    pub id: Option<String>,
    pub code: &'static str,
    pub message: String,
}

pub fn run(command: DesignCommand, store: &mut Store, cwd: &Path) -> Result<Output, SkaldError> {
    match command {
        DesignCommand::Submit { kind, name, file } => submit(store, cwd, kind, &name, &file),
        DesignCommand::List { limit, fields } => {
            list(store, cwd, limit.unwrap_or(DEFAULT_LIMIT), &fields)
        }
        DesignCommand::Get { id, version, full } => get(store, cwd, &id, version, full),
        DesignCommand::Diff { id, from, to } => diff(store, cwd, &id, from, to),
        DesignCommand::Render { id, version, out } => render(store, cwd, &id, version, &out),
        DesignCommand::Open {
            id,
            version,
            no_browser,
            idle_timeout,
        } => open(store, cwd, &id, version, no_browser, idle_timeout),
    }
}

/// The newest version of a design, as `submit` needs it.
struct Newest {
    id: String,
    kind: Kind,
    branch: String,
    number: u32,
    record: Option<String>,
    html: Option<String>,
}

impl Newest {
    fn find(
        tx: &Transaction<'_>,
        repository: &str,
        name: &str,
    ) -> Result<Option<Self>, SkaldError> {
        Ok(tx
            .query_row(
                "SELECT d.id, d.kind, d.branch, v.number, v.record, v.html
                 FROM design d JOIN design_version v ON v.design = d.id
                 WHERE d.repository = ?1 AND d.name = ?2
                 ORDER BY v.number DESC LIMIT 1",
                params![repository, name],
                |row| {
                    Ok(Self {
                        id: row.get(0)?,
                        kind: row.get(1)?,
                        branch: row.get(2)?,
                        number: row.get(3)?,
                        record: row.get(4)?,
                        html: row.get(5)?,
                    })
                },
            )
            .optional()?)
    }
}

fn submit(
    store: &mut Store,
    cwd: &Path,
    kind: Kind,
    name: &str,
    file: &str,
) -> Result<Output, SkaldError> {
    let info = repository(cwd)?;
    let text = read(file)?;
    let (tx, repo) = begin(store, info)?;
    let newest = Newest::find(&tx, &repo.id, name)?;
    if let Some(newest) = newest.as_ref().filter(|newest| newest.kind != kind) {
        return Err(kind_mismatch(name, newest.kind, kind));
    }
    let (record, html) = match kind {
        Kind::DataModel => (
            Some(canonical::<DataModelRecord>(&text, newest.as_ref())?),
            None,
        ),
        Kind::Interface => (
            Some(canonical::<InterfaceRecord>(&text, newest.as_ref())?),
            None,
        ),
        Kind::Freeform => (None, Some(freeform(text)?)),
    };
    let (record, html) = (record.as_deref(), html.as_deref());

    let now = store::now(&tx)?;
    let (id, branch, version, created) = match newest {
        Some(newest) if newest.record.as_deref() == record && newest.html.as_deref() == html => {
            (newest.id, newest.branch, newest.number, false)
        }
        Some(newest) => {
            let version = newest.number + 1;
            insert_version(&tx, &newest.id, version, &repo.id, &now, record, html)?;
            (newest.id, newest.branch, version, true)
        }
        None => {
            let id = insert_design(&tx, &repo, kind, name, &now, record, html)?;
            (id, repo.branch, 1, true)
        }
    };
    tx.commit()?;

    let mut view = Map::new();
    view.insert(
        "design".to_owned(),
        json!({
            "id": id,
            "name": name,
            "kind": kind.as_str(),
            "version": version,
            "branch": branch,
            "status": if created { "created" } else { "unchanged" },
        }),
    );
    let mut help = vec![
        format!("skald design open {id}"),
        format!("skald design get {id}"),
    ];
    if created && version > 1 && kind != Kind::Freeform {
        help.push(format!(
            "skald design diff {id} --from {} --to {version}",
            version - 1
        ));
    }
    Ok(Output::new(view).with_help(help))
}

/// Decodes and validates `text` against the newest stored record, and returns the canonical
/// record.
fn canonical<T: Typed>(text: &str, newest: Option<&Newest>) -> Result<String, SkaldError> {
    let submission: T::Submission =
        decode(text, newest.map(|newest| newest.id.as_str())).map_err(rejected)?;
    let previous: Option<T> = newest
        .map(|newest| stored(&newest.id, newest.record.as_deref()))
        .transpose()?;
    let record = T::validate(&submission, previous.as_ref()).map_err(rejected)?;
    Ok(serde_json::to_string(&record).expect("a record always serializes"))
}

fn freeform(html: String) -> Result<String, SkaldError> {
    if html.trim().is_empty() {
        return Err(
            SkaldError::new("empty-submission", "the submitted file holds no HTML")
                .with_help("skald design submit --kind freeform --name <name> --file <path>"),
        );
    }
    Ok(html)
}

fn kind_mismatch(name: &str, stored: Kind, submitted: Kind) -> SkaldError {
    let (stored, submitted) = (stored.as_str(), submitted.as_str());
    SkaldError::new(
        "kind-mismatch",
        format!(
            "design `{name}` has kind {stored}, so it cannot take a submission of kind {submitted}"
        ),
    )
    .with_help(format!(
        "skald design submit --kind {stored} --name {name} --file <path>"
    ))
    .with_help(format!(
        "skald design submit --kind {submitted} --name <name> --file <path>"
    ))
}

/// The record each design made by `skald dev seed` holds.
const SEED_RECORD: &str = "\
entities[1]:
  - name: Seed
    fields[1]{name,type,nullable}:
      id,uuid,false
";

/// Adds `count` data-model designs to the repository at `cwd` in one transaction. Only the
/// performance and schema tests use it, through the hidden `skald dev seed` command.
pub fn seed(store: &mut Store, cwd: &Path, count: usize) -> Result<Output, SkaldError> {
    let (tx, repo) = begin(store, repository(cwd)?)?;
    let now = store::now(&tx)?;
    let (_, existing) = summaries(&tx, &repo.id, 0)?;
    for number in existing + 1..=existing + count {
        let record = canonical::<DataModelRecord>(SEED_RECORD, None)?;
        let name = format!("seed-{number}");
        insert_design(
            &tx,
            &repo,
            Kind::DataModel,
            &name,
            &now,
            Some(&record),
            None,
        )?;
    }
    tx.commit()?;
    let mut view = Map::new();
    view.insert(
        "seeded".to_owned(),
        json!({"repository": repo.identity, "designs": count}),
    );
    Ok(Output::new(view))
}

/// Inserts a new design with `record` or `html` as its version 1, and returns its id.
fn insert_design(
    tx: &Transaction<'_>,
    repo: &RepoContext,
    kind: Kind,
    name: &str,
    created_at: &str,
    record: Option<&str>,
    html: Option<&str>,
) -> Result<String, SkaldError> {
    let id = id::new('d');
    tx.execute(
        "INSERT INTO design (id, repository, kind, name, branch, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, repo.id, kind.as_str(), name, repo.branch, created_at],
    )?;
    insert_version(tx, &id, 1, &repo.id, created_at, record, html)?;
    Ok(id)
}

fn insert_version(
    tx: &Transaction<'_>,
    design: &str,
    number: u32,
    repository: &str,
    submitted_at: &str,
    record: Option<&str>,
    html: Option<&str>,
) -> Result<(), SkaldError> {
    tx.execute(
        "INSERT INTO design_version (design, number, repository, submitted_at, record, html)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![design, number, repository, submitted_at, record, html],
    )?;
    Ok(())
}

fn list(
    store: &mut Store,
    cwd: &Path,
    limit: usize,
    fields: &[String],
) -> Result<Output, SkaldError> {
    if let Some(unknown) = fields
        .iter()
        .find(|field| !FIELDS.contains(&field.as_str()))
    {
        return Err(SkaldError::new(
            "unknown-field",
            format!("unknown field `{unknown}` for `skald design list`"),
        )
        .with_help(format!("Valid fields: {}", FIELDS.join(", "))));
    }
    let (tx, repo) = begin_read(store, repository(cwd)?)?;
    let (designs, total) = summaries(&tx, &repo.id, limit)?;
    tx.commit()?;

    let shown = designs.len();
    let mut view = Map::new();
    view.insert(
        "designs".to_owned(),
        designs.iter().map(|design| design.item(fields)).collect(),
    );
    view.insert(
        "count".to_owned(),
        Value::from(format!("{shown} of {total} total")),
    );
    let mut help = Vec::new();
    if total == 0 {
        view.insert(
            "note".to_owned(),
            Value::from(format!("0 designs recorded for {}", repo.identity)),
        );
        help.push(SUBMIT_HELP);
    } else {
        help.push("skald design get <id>");
        if shown < total {
            help.push("skald design list --limit <n>");
        }
    }
    Ok(Output::new(view).with_help(help.into_iter().map(String::from)))
}

/// One stored version of a design.
struct Version {
    name: String,
    kind: Kind,
    branch: String,
    number: u32,
    submitted_at: String,
    record: Option<String>,
    html: Option<String>,
}

impl Version {
    /// Finds version `number` of design `id`, or its newest version when `number` is `None`.
    fn find(
        store: &mut Store,
        cwd: &Path,
        id: &str,
        number: Option<u32>,
    ) -> Result<Self, SkaldError> {
        let (tx, repo) = begin_read(store, repository(cwd)?)?;
        let found = tx
            .query_row(
                "SELECT d.name, d.kind, d.branch, v.number, v.submitted_at, v.record, v.html
                 FROM design d JOIN design_version v ON v.design = d.id
                 WHERE d.id = ?1 AND d.repository = ?2 AND v.number = coalesce(
                     ?3, (SELECT max(number) FROM design_version WHERE design = d.id))",
                params![id, repo.id, number],
                |row| {
                    Ok(Self {
                        name: row.get(0)?,
                        kind: row.get(1)?,
                        branch: row.get(2)?,
                        number: row.get(3)?,
                        submitted_at: row.get(4)?,
                        record: row.get(5)?,
                        html: row.get(6)?,
                    })
                },
            )
            .optional()?;
        tx.commit()?;
        found.ok_or_else(|| {
            let what = number
                .map(|n| format!(" and version {n}"))
                .unwrap_or_default();
            SkaldError::new(
                "not-found",
                format!("no design with id `{id}`{what} in {}", repo.identity),
            )
            .with_help("skald design list")
        })
    }
}

fn get(
    store: &mut Store,
    cwd: &Path,
    id: &str,
    version: Option<u32>,
    full: bool,
) -> Result<Output, SkaldError> {
    let mut found = Version::find(store, cwd, id, version)?;
    let version_flag = version
        .map(|n| format!(" --version {n}"))
        .unwrap_or_default();
    let (key, mut content) = match found.html.take() {
        Some(html) => ("html", Value::from(html)),
        None => ("record", stored(id, found.record.as_deref())?),
    };
    let mut help = Vec::new();
    if !full && output::truncate_text(&mut content) {
        help.push(format!("skald design get {id}{version_flag} --full"));
    }
    help.push(format!("skald design open {id}{version_flag}"));

    let mut view = Map::new();
    view.insert(
        "design".to_owned(),
        json!({
            "id": id,
            "name": found.name,
            "kind": found.kind.as_str(),
            "branch": found.branch,
            "version": found.number,
            "submitted_at": found.submitted_at,
        }),
    );
    view.insert(key.to_owned(), content);
    Ok(Output::new(view).with_help(help))
}

/// Writes the rendered page to `out`, or to stdout when `out` is `-`.
fn render(
    store: &mut Store,
    cwd: &Path,
    id: &str,
    version: Option<u32>,
    out: &Path,
) -> Result<Output, SkaldError> {
    let found = Version::find(store, cwd, id, version)?;
    let page = page(id, &found)?;
    if out == Path::new("-") {
        return Ok(Output::Raw(page));
    }

    let path = cwd.join(out);
    std::fs::write(&path, &page).map_err(|err| {
        SkaldError::new(
            "unwritable-file",
            format!("cannot write {}: {err}", path.display()),
        )
        .with_help(format!("skald design render {id} --out <path>"))
    })?;
    let mut view = Map::new();
    view.insert(
        "rendered".to_owned(),
        json!({
            "design": id,
            "version": found.number,
            "path": path.display().to_string(),
            "bytes": page.len(),
        }),
    );
    Ok(Output::new(view))
}

/// The rendered page of `version` of design `id`.
fn page(id: &str, version: &Version) -> Result<String, SkaldError> {
    let record = match version.kind {
        Kind::DataModel => render::Record::DataModel(stored(id, version.record.as_deref())?),
        Kind::Interface => render::Record::Interface(stored(id, version.record.as_deref())?),
        Kind::Freeform => {
            return Err(SkaldError::new(
                "not-renderable",
                format!(
                    "design `{id}` has kind {}; only data-model and interface designs can be rendered",
                    version.kind.as_str()
                ),
            )
            .with_help(format!("skald design open {id}")));
        }
    };
    let header = DesignHeader {
        name: &version.name,
        kind: version.kind.as_str(),
    };
    Ok(render::render(&header, version.number, &record))
}

/// The page `skald serve` answers with, and its version number: the stored HTML of a freeform
/// design, or the page `design render` writes for a typed one.
pub fn served(
    store: &mut Store,
    cwd: &Path,
    id: &str,
    version: Option<u32>,
) -> Result<(u32, String), SkaldError> {
    let mut found = Version::find(store, cwd, id, version)?;
    let page = match found.html.take() {
        Some(html) => html,
        None => page(id, &found)?,
    };
    Ok((found.number, page))
}

fn open(
    store: &mut Store,
    cwd: &Path,
    id: &str,
    version: Option<u32>,
    no_browser: bool,
    idle_timeout: Option<u64>,
) -> Result<Output, SkaldError> {
    // Building the page here reports a missing or unservable design under its own error code,
    // rather than as a server that failed to start.
    let (number, page) = served(store, cwd, id, version)?;
    let url = serve::spawn(&page, idle_timeout)?;
    let browser = if no_browser {
        "skipped"
    } else {
        serve::launch_browser(&url)
    };
    let mut view = Map::new();
    view.insert(
        "opened".to_owned(),
        json!({"design": id, "version": number, "url": url, "browser": browser}),
    );
    Ok(Output::new(view).with_help([format!("skald design get {id} --version {number}")]))
}

fn diff(store: &mut Store, cwd: &Path, id: &str, from: u32, to: u32) -> Result<Output, SkaldError> {
    let (tx, repo) = begin_read(store, repository(cwd)?)?;
    let found: Option<(Kind, u32)> = tx
        .query_row(
            "SELECT d.kind, max(v.number)
             FROM design d JOIN design_version v ON v.design = d.id
             WHERE d.id = ?1 AND d.repository = ?2
             GROUP BY d.id",
            params![id, repo.id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let Some((kind, newest)) = found else {
        return Err(SkaldError::new(
            "not-found",
            format!("no design with id `{id}` in {}", repo.identity),
        )
        .with_help("skald design list"));
    };
    let diff_of = match kind {
        Kind::DataModel => changes::<DataModelRecord>,
        Kind::Interface => changes::<InterfaceRecord>,
        Kind::Freeform => {
            return Err(SkaldError::new(
                "not-comparable",
                format!(
                    "design `{id}` has kind {}, whose versions cannot be compared",
                    kind.as_str()
                ),
            )
            .with_help(format!("skald design get {id} --version <n>"))
            .with_help(format!("skald design open {id} --version <n>")));
        }
    };
    // Versions are numbered from 1 without gaps, because no version is ever deleted.
    if let Some(missing) = [from, to]
        .into_iter()
        .find(|number| !(1..=newest).contains(number))
    {
        return Err(SkaldError::new(
            "not-found",
            format!("design `{id}` has no version {missing}; its newest version is {newest}"),
        )
        .with_help(format!("skald design diff {id} --from <n> --to <n>")));
    }
    let record = |number: u32| -> Result<Option<String>, SkaldError> {
        Ok(tx.query_row(
            "SELECT record FROM design_version WHERE design = ?1 AND number = ?2",
            params![id, number],
            |row| row.get(0),
        )?)
    };
    let (older, newer) = (record(from)?, record(to)?);
    tx.commit()?;
    let changes = diff_of(id, older.as_deref(), newer.as_deref())?;

    let mut view = Map::new();
    view.insert(
        "diff".to_owned(),
        json!({
            "design": id,
            "from": from,
            "to": to,
            "added": changes.added,
            "removed": changes.removed,
            "changed": changes.changed,
        }),
    );
    Ok(Output::new(view).with_help([format!("skald design get {id} --version {to}")]))
}

fn changes<T: Typed>(id: &str, from: Option<&str>, to: Option<&str>) -> Result<Diff, SkaldError> {
    Ok(diff::diff(&stored::<T>(id, from)?, &stored::<T>(id, to)?))
}

/// Parses the stored record of design `id`. A missing record, as a freeform design has, reads
/// as JSON `null`.
fn stored<T: DeserializeOwned>(id: &str, record: Option<&str>) -> Result<T, SkaldError> {
    serde_json::from_str(record.unwrap_or("null")).map_err(|err| {
        SkaldError::new(
            "store-error",
            format!("the stored record of design `{id}` cannot be read: {err}"),
        )
    })
}

/// A design as a list item: its newest version and when it was submitted.
pub struct Summary {
    id: String,
    name: String,
    kind: Kind,
    version: i64,
    branch: String,
    created_at: String,
    submitted_at: String,
}

impl Summary {
    /// The default fields followed by `extra` in the order given. Unknown names are skipped.
    pub fn item(&self, extra: &[String]) -> Value {
        let names = FIELDS[..4].iter().copied();
        let names = names.chain(extra.iter().map(String::as_str));
        let item: Map<String, Value> = names
            .filter_map(|name| Some((name.to_owned(), self.field(name)?)))
            .collect();
        Value::Object(item)
    }

    fn field(&self, name: &str) -> Option<Value> {
        Some(match name {
            "id" => json!(self.id),
            "name" => json!(self.name),
            "kind" => json!(self.kind.as_str()),
            "version" => json!(self.version),
            "branch" => json!(self.branch),
            "created_at" => json!(self.created_at),
            "submitted_at" => json!(self.submitted_at),
            _ => return None,
        })
    }
}

const SUMMARIES_SQL: &str =
    "SELECT d.id, d.name, d.kind, v.number, d.branch, d.created_at, v.submitted_at
     FROM design d JOIN design_version v ON v.design = d.id
     WHERE d.repository = ?1
       AND v.number = (SELECT max(number) FROM design_version WHERE design = d.id)
     ORDER BY d.created_at DESC, d.rowid DESC
     LIMIT ?2";

/// The newest `limit` designs of `repository`, newest first, and the number of designs in total.
pub fn summaries(
    conn: &Connection,
    repository: &str,
    limit: usize,
) -> Result<(Vec<Summary>, usize), SkaldError> {
    let total: i64 = conn.query_row(
        "SELECT count(*) FROM design WHERE repository = ?1",
        [repository],
        |row| row.get(0),
    )?;
    // rowid breaks ties between designs created within the same second.
    let mut statement = conn.prepare(SUMMARIES_SQL)?;
    let limit = i64::try_from(limit).unwrap_or(i64::MAX);
    let summaries = statement
        .query_map(params![repository, limit], |row| {
            Ok(Summary {
                id: row.get(0)?,
                name: row.get(1)?,
                kind: row.get(2)?,
                version: row.get(3)?,
                branch: row.get(4)?,
                created_at: row.get(5)?,
                submitted_at: row.get(6)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok((summaries, usize::try_from(total).unwrap_or_default()))
}

fn repository(cwd: &Path) -> Result<RepoInfo, SkaldError> {
    git::resolve(cwd)?.ok_or_else(|| {
        SkaldError::new(
            "not-a-repository",
            format!(
                "no git repository found at {}; designs are kept per repository",
                cwd.display()
            ),
        )
        .with_help("cd <repository> && skald")
    })
}

fn begin(store: &mut Store, info: RepoInfo) -> Result<(Transaction<'_>, RepoContext), SkaldError> {
    let tx = store.transaction()?;
    let repo = RepoContext::find_or_insert(&tx, info)?;
    Ok((tx, repo))
}

/// Opens a deferred transaction for a command that only reads. The repository row is inserted on
/// first sight in a short `IMMEDIATE` transaction of its own, because a deferred transaction that
/// upgrades to a write fails with SQLITE_BUSY without waiting when another process holds the lock.
pub fn begin_read(
    store: &mut Store,
    info: RepoInfo,
) -> Result<(Transaction<'_>, RepoContext), SkaldError> {
    let tx = store.read_transaction()?;
    let found = RepoContext::find(&tx, info)?;
    drop(tx);
    let repo = match found {
        Ok(repo) => repo,
        Err(info) => {
            let tx = store.transaction()?;
            let repo = RepoContext::find_or_insert(&tx, info)?;
            tx.commit()?;
            repo
        }
    };
    Ok((store.read_transaction()?, repo))
}

fn read(file: &str) -> Result<String, SkaldError> {
    let (text, source) = if file == "-" {
        (std::io::read_to_string(std::io::stdin()), "standard input")
    } else {
        (std::fs::read_to_string(file), file)
    };
    text.map_err(|err| {
        SkaldError::new("unreadable-file", format!("cannot read {source}: {err}"))
            .with_help(SUBMIT_HELP)
    })
}

/// Decodes JSON when the text starts with `{` or `[`, and TOON otherwise. `id` is the design
/// that the text is submitted to, when it exists.
fn decode<T: DeserializeOwned>(text: &str, id: Option<&str>) -> Result<T, Vec<Problem>> {
    let value: Value = if text.trim_start().starts_with(['{', '[']) {
        serde_json::from_str(text).map_err(|err| format!("invalid JSON: {err}"))
    } else {
        toon_format::decode_default(text).map_err(|err| format!("invalid TOON: {err}"))
    }
    .map_err(|message| vec![invalid_format(message)])?;
    let mut problems = Vec::new();
    truncated_text(&value, &mut String::new(), id, &mut problems);
    if !problems.is_empty() {
        return Err(problems);
    }
    serde_json::from_value(value).map_err(|err| vec![invalid_format(err.to_string())])
}

/// Adds a problem for each string in `value` that `get` cut short, because storing it would lose
/// the rest of the text.
fn truncated_text(value: &Value, path: &mut String, id: Option<&str>, problems: &mut Vec<Problem>) {
    let length = path.len();
    match value {
        Value::String(text) if output::is_truncated(text) => {
            let command = id.map_or_else(
                || "skald design get <id> --full".to_owned(),
                |id| format!("skald design get {id} --full"),
            );
            problems.push(Problem {
                path: path.clone(),
                id: None,
                code: "truncated-text",
                message: format!(
                    "text ends with the marker that `get` adds to long text; read the full text with `{command}`"
                ),
            });
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                path.push_str(&format!("/{index}"));
                truncated_text(item, path, id, problems);
                path.truncate(length);
            }
        }
        Value::Object(map) => {
            for (key, item) in map {
                path.push('/');
                path.push_str(&key.replace('~', "~0").replace('/', "~1"));
                truncated_text(item, path, id, problems);
                path.truncate(length);
            }
        }
        _ => {}
    }
}

fn invalid_format(message: String) -> Problem {
    Problem {
        path: String::new(),
        id: None,
        code: "invalid-format",
        message,
    }
}

fn rejected(problems: Vec<Problem>) -> SkaldError {
    let count = problems.len();
    let noun = if count == 1 { "problem" } else { "problems" };
    let mut error = SkaldError::new("invalid-submission", format!("{count} {noun} found"))
        .with_help("Fix the listed problems and run `skald design submit ...` again");
    error.problems = problems
        .iter()
        .map(|problem| serde_json::to_value(problem).expect("a problem always serializes"))
        .collect();
    error
}

#[cfg(test)]
mod tests {
    use super::data_model::DataModelSubmission;
    use super::*;

    #[test]
    fn summaries_plan_needs_no_sort() {
        let dir = tempfile::TempDir::new().unwrap();
        let mut store = crate::store::Store::open(dir.path()).unwrap();
        let tx = store.transaction().unwrap();
        let mut statement = tx
            .prepare(&format!("EXPLAIN QUERY PLAN {SUMMARIES_SQL}"))
            .unwrap();
        let plan: Vec<String> = statement
            .query_map(params!["repository", 5], |row| row.get(3))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(
            plan.iter().all(|step| !step.contains("TEMP B-TREE")),
            "the summaries query sorts in a temporary tree: {plan:?}"
        );
    }

    fn decoded(text: &str) -> Result<Value, Problem> {
        decode(text, None).map_err(|mut problems| problems.remove(0))
    }

    #[test]
    fn format_is_detected_from_the_first_non_whitespace_byte() {
        assert_eq!(decoded(" \n{\"a\": 1}").unwrap(), json!({"a": 1}));
        assert_eq!(decoded("[1, 2]").unwrap(), json!([1, 2]));
        assert_eq!(decoded("a: 1\n").unwrap(), json!({"a": 1}));
    }

    #[test]
    fn undecodable_text_is_an_invalid_format_problem() {
        for text in ["{\"a\": ", "a: [1\n  b"] {
            let problem = decoded(text).unwrap_err();
            assert_eq!(problem.code, "invalid-format");
            assert_eq!(problem.path, "");
        }
    }

    #[test]
    fn a_record_of_the_wrong_shape_is_an_invalid_format_problem() {
        let problem = decode::<DataModelSubmission>("entities: 3\n", None)
            .unwrap_err()
            .remove(0);
        assert_eq!(problem.code, "invalid-format");
        let problem = decode::<DataModelSubmission>("", None)
            .unwrap_err()
            .remove(0);
        assert!(problem.message.contains("entities"), "{}", problem.message);
    }

    #[test]
    fn kind_names_round_trip() {
        for kind in Kind::value_variants() {
            assert_eq!(Kind::from_str(kind.as_str(), false), Ok(*kind));
        }
    }

    #[test]
    fn freeform_design_is_not_renderable() {
        let version = Version {
            name: "sketch".to_owned(),
            kind: Kind::Freeform,
            branch: "main".to_owned(),
            number: 1,
            submitted_at: "2026-10-26T16:12:37Z".to_owned(),
            record: None,
            html: None,
        };
        let error = page("dsketch00000", &version).unwrap_err();
        assert_eq!(error.code, "not-renderable");
        assert_eq!(error.exit_code, 1);
        assert_eq!(error.help, ["skald design open dsketch00000"]);
    }

    #[test]
    fn rejection_lists_every_problem() {
        let problem = |path: &str| Problem {
            path: path.to_owned(),
            id: None,
            code: "missing-name",
            message: "entity has no name".to_owned(),
        };
        let one = rejected(vec![problem("/entities/0")]);
        assert_eq!(one.message, "1 problem found");
        let two = rejected(vec![problem("/entities/0"), problem("/entities/1")]);
        assert_eq!(two.code, "invalid-submission");
        assert_eq!(two.exit_code, 1);
        assert_eq!(two.message, "2 problems found");
        assert_eq!(
            two.problems[1],
            json!({"path": "/entities/1", "id": null, "code": "missing-name", "message": "entity has no name"})
        );
    }
}
