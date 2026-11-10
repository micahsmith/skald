use std::path::PathBuf;

use rusqlite::{OptionalExtension, Transaction, params};

use crate::error::SkaldError;
use crate::git::{self, RepoInfo};
use crate::id;
use crate::store;

#[expect(dead_code, reason = "nothing reads default_branch or root yet")]
pub struct RepoContext {
    pub id: String,
    pub identity: String,
    pub default_branch: String,
    pub branch: String,
    pub root: PathBuf,
}

impl RepoContext {
    /// Returns `info` unchanged when the repository row is missing.
    pub fn find(
        tx: &Transaction<'_>,
        info: RepoInfo,
    ) -> Result<Result<Self, RepoInfo>, SkaldError> {
        let found = tx
            .query_row(
                "SELECT id, default_branch FROM repository WHERE identity = ?1",
                [&info.identity],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        Ok(match found {
            Some((id, default_branch)) => Ok(Self::new(id, default_branch, info)),
            None => Err(info),
        })
    }

    pub fn find_or_insert(tx: &Transaction<'_>, info: RepoInfo) -> Result<Self, SkaldError> {
        let info = match Self::find(tx, info)? {
            Ok(repo) => return Ok(repo),
            Err(info) => info,
        };
        let id = id::new('y');
        let default_branch = git::default_branch(&info.root)?;
        let created_at = store::now(tx)?;
        tx.execute(
            "INSERT INTO repository (id, identity, default_branch, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![id, info.identity, default_branch, created_at],
        )?;
        Ok(Self::new(id, default_branch, info))
    }

    fn new(id: String, default_branch: String, info: RepoInfo) -> Self {
        Self {
            id,
            identity: info.identity,
            default_branch,
            branch: info.branch,
            root: info.root,
        }
    }
}
