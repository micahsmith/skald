use std::collections::{HashMap, HashSet};

use serde_json::{Map, Value};

use super::Problem;
use super::diff::{self, Diffable};
use crate::id;

/// Every kind of element that a typed record holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ElementKind {
    Entity,
    Field,
    Relation,
    Operation,
    Input,
    Output,
    Error,
}

impl ElementKind {
    pub fn noun(self) -> &'static str {
        match self {
            Self::Entity => "entity",
            Self::Field => "field",
            Self::Relation => "relation",
            Self::Operation => "operation",
            Self::Input => "input",
            Self::Output => "output",
            Self::Error => "error",
        }
    }

    pub fn assign(self, supplied: Option<&str>) -> String {
        let prefix = match self {
            Self::Entity => 'e',
            Self::Field => 'f',
            Self::Relation => 'r',
            Self::Operation => 'o',
            Self::Input | Self::Output => 'p',
            Self::Error => 'x',
        };
        supplied.map_or_else(|| id::new(prefix), str::to_owned)
    }

    pub fn for_diff<const N: usize>(
        self,
        id: &str,
        attributes: [(&'static str, Value); N],
    ) -> diff::Element {
        diff::Element {
            kind: self.noun(),
            id: id.to_owned(),
            attributes: attributes.into(),
        }
    }
}

/// The element under validation: its kind, its path, and the id the submission supplied.
pub struct Site<'a, 'p> {
    pub element: ElementKind,
    pub path: &'p str,
    pub id: Option<&'a str>,
}

fn pointer_token(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

/// Collects problems while tracking which ids the submission may keep and which it has used.
pub struct Check<'a> {
    /// The element kind of every id in the previous version.
    known: HashMap<String, &'static str>,
    used: HashSet<&'a str>,
    problems: Vec<Problem>,
}

impl<'a> Check<'a> {
    /// `previous` is the newest stored record of the design, whose ids a submission may keep.
    pub fn new(previous: Option<&impl Diffable>) -> Self {
        let known = previous
            .map(|record| record.ids())
            .unwrap_or_default()
            .into_iter()
            .collect();
        Self {
            known,
            used: HashSet::new(),
            problems: Vec::new(),
        }
    }

    /// `site.id` is attached to the problem. `path` is the problem's own path, which differs from
    /// `site.path` for a child path.
    pub fn report(&mut self, site: &Site<'a, '_>, path: &str, code: &'static str, message: String) {
        self.problems.push(Problem {
            path: path.to_owned(),
            id: site.id.map(str::to_owned),
            code,
            message,
        });
    }

    pub fn id(&mut self, site: &Site<'a, '_>) {
        let Some(id) = site.id else { return };
        let noun = site.element.noun();
        if !self.used.insert(id) {
            self.report(
                site,
                site.path,
                "duplicate-id",
                format!("id `{id}` is already used by an earlier element"),
            );
        } else if self.known.get(id) != Some(&noun) {
            self.report(
                site,
                site.path,
                "unknown-id",
                format!(
                    "the newest version of this design has no {noun} with id `{id}`; omit `id` \
                     to create a new {noun}"
                ),
            );
        }
    }

    pub fn name(
        &mut self,
        site: &Site<'a, '_>,
        siblings: &mut HashSet<&'a str>,
        name: Option<&'a str>,
    ) -> String {
        match name {
            Some(name) if !name.trim().is_empty() => {
                self.unique(site, siblings, name);
            }
            _ => self.report(
                site,
                site.path,
                "missing-name",
                format!("{} has no name", site.element.noun()),
            ),
        }
        name.unwrap_or_default().to_owned()
    }

    pub fn unique(&mut self, site: &Site<'a, '_>, siblings: &mut HashSet<&'a str>, name: &'a str) {
        if !siblings.insert(name) {
            self.report(
                site,
                site.path,
                "duplicate-name",
                format!(
                    "an earlier {} is already named `{name}`",
                    site.element.noun()
                ),
            );
        }
    }

    /// Returns `value`, and reports `code` when it is missing or blank.
    pub fn required(
        &mut self,
        site: &Site<'a, '_>,
        attribute: &str,
        code: &'static str,
        value: Option<&str>,
    ) -> String {
        let value = value.unwrap_or_default();
        if value.trim().is_empty() {
            let message = format!("{} has no {attribute}", site.element.noun());
            self.report(site, site.path, code, message);
        }
        value.to_owned()
    }

    pub fn unknown_keys(&mut self, site: &Site<'a, '_>, unknown: &Map<String, Value>) {
        let noun = site.element.noun();
        let article = if noun.starts_with(['a', 'e', 'i', 'o', 'u']) {
            "an"
        } else {
            "a"
        };
        for key in unknown.keys() {
            self.report(
                site,
                &format!("{}/{}", site.path, pointer_token(key)),
                "unknown-key",
                format!("`{key}` is not a key of {article} {noun}"),
            );
        }
    }

    /// Reports the unknown keys of a whole record, which has no element. `noun` includes its
    /// article.
    pub fn unknown_record_keys(&mut self, unknown: &Map<String, Value>, noun: &str) {
        for key in unknown.keys() {
            self.problems.push(Problem {
                path: format!("/{}", pointer_token(key)),
                id: None,
                code: "unknown-key",
                message: format!("`{key}` is not a key of {noun}"),
            });
        }
    }

    pub fn finish<T>(self, record: T) -> Result<T, Vec<Problem>> {
        if self.problems.is_empty() {
            Ok(record)
        } else {
            Err(self.problems)
        }
    }
}
