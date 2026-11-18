use rusqlite::{OptionalExtension, Transaction, params};

use crate::error::SkaldError;
use crate::git::RepoInfo;
use crate::id;
use crate::store;

pub struct RepoContext {
    pub id: String,
    pub identity: String,
    pub branch: String,
}

impl RepoContext {
    /// Returns `None` when the repository row is missing.
    pub fn find(tx: &Transaction<'_>, info: &RepoInfo) -> Result<Option<Self>, SkaldError> {
        let found = tx
            .query_row(
                "SELECT id FROM repository WHERE identity = ?1",
                [&info.identity],
                |row| row.get(0),
            )
            .optional()?;
        Ok(found.map(|id| Self::new(id, info)))
    }

    /// Inserts the row with `default_branch` when it is still missing. The caller resolves the
    /// branch beforehand so that git does not run while `tx` holds the write lock.
    pub fn find_or_insert(
        tx: &Transaction<'_>,
        info: RepoInfo,
        default_branch: String,
    ) -> Result<Self, SkaldError> {
        if let Some(repo) = Self::find(tx, &info)? {
            return Ok(repo);
        }
        let id = id::new('y');
        let created_at = store::now(tx)?;
        tx.execute(
            "INSERT INTO repository (id, identity, default_branch, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![id, info.identity, default_branch, created_at],
        )?;
        Ok(Self::new(id, &info))
    }

    fn new(id: String, info: &RepoInfo) -> Self {
        Self {
            id,
            identity: info.identity.clone(),
            branch: info.branch.clone(),
        }
    }
}
