use std::path::{Path, PathBuf};

use rusqlite::{Connection, Transaction, TransactionBehavior};

use crate::error::SkaldError;

/// The one place that spells the timestamp format stored in the database.
pub fn now(conn: &Connection) -> rusqlite::Result<String> {
    conn.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [], |row| {
        row.get(0)
    })
}

/// SQL per schema version. Index 0 is version 1.
const MIGRATIONS: &[&str] = &["
    CREATE TABLE repository (
        id TEXT PRIMARY KEY,
        identity TEXT UNIQUE NOT NULL,
        default_branch TEXT NOT NULL,
        created_at TEXT NOT NULL
    );
    CREATE TABLE design (
        id TEXT PRIMARY KEY,
        repository TEXT NOT NULL,
        kind TEXT NOT NULL,
        name TEXT NOT NULL,
        branch TEXT NOT NULL,
        created_at TEXT NOT NULL,
        UNIQUE (repository, name)
    );
    CREATE INDEX design_created ON design (repository, created_at);
    CREATE TABLE design_version (
        design TEXT NOT NULL,
        number INTEGER NOT NULL,
        repository TEXT NOT NULL,
        submitted_at TEXT NOT NULL,
        record TEXT,
        html TEXT,
        PRIMARY KEY (design, number)
    );
"];

pub const UPGRADE: &str = "cargo install --locked --git https://github.com/micahsmith/skald skald";

pub struct Store {
    conn: Connection,
    path: PathBuf,
}

impl From<rusqlite::Error> for SkaldError {
    fn from(err: rusqlite::Error) -> Self {
        Self::new("store-error", format!("store error: {err}"))
    }
}

impl Store {
    /// Opens `<data_dir>/skald.db`, creating the directory and the file as needed, and migrates
    /// it to the newest schema.
    pub fn open(data_dir: &Path) -> Result<Self, SkaldError> {
        crate::paths::create_data_dir(data_dir).map_err(|err| {
            SkaldError::new(
                "store-unavailable",
                format!("cannot create {}: {err}", data_dir.display()),
            )
        })?;
        let path = data_dir.join("skald.db");
        let conn = Connection::open(&path).map_err(|err| {
            SkaldError::new(
                "store-unavailable",
                format!("cannot open {}: {err}", path.display()),
            )
        })?;
        let mut store = Self { conn, path };
        store.migrate()?;
        Ok(store)
    }

    pub fn migrate(&mut self) -> Result<(), SkaldError> {
        self.migrate_with(MIGRATIONS)
    }

    fn migrate_with(&mut self, migrations: &[&str]) -> Result<(), SkaldError> {
        let newest = migrations.len();
        if schema_version(&self.conn)? == newest {
            return Ok(());
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        // Read again under the write lock: another process may have migrated in the meantime.
        let current = schema_version(&tx)?;
        if current > newest {
            return Err(SkaldError::new(
                "store-newer-than-program",
                format!(
                    "the store at {} has schema version {current}, but this skald knows versions \
                     up to {newest}; upgrade skald",
                    self.path.display()
                ),
            )
            .with_help(UPGRADE));
        }
        for sql in &migrations[current..] {
            tx.execute_batch(sql)?;
        }
        tx.pragma_update(None, "user_version", newest as i64)?;
        tx.commit()?;
        Ok(())
    }

    /// Takes the write lock at once, so that a later write in the transaction never fails on a
    /// lock held by another process.
    pub fn transaction(&mut self) -> Result<Transaction<'_>, SkaldError> {
        Ok(self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?)
    }

    /// Takes no lock until the first read, so that processes that only read never wait on each
    /// other. A write inside it fails at once with SQLITE_BUSY when another process holds the
    /// write lock, so it MUST NOT write.
    pub fn read_transaction(&mut self) -> Result<Transaction<'_>, SkaldError> {
        Ok(self
            .conn
            .transaction_with_behavior(TransactionBehavior::Deferred)?)
    }
}

fn schema_version(conn: &Connection) -> Result<usize, SkaldError> {
    let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    usize::try_from(version).map_err(|_| {
        SkaldError::new(
            "store-error",
            format!("the store has an invalid schema version {version}"),
        )
    })
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn tables(store: &Store) -> Vec<String> {
        let mut statement = store
            .conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
            .unwrap();
        statement
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    }

    #[test]
    fn fresh_store_migrates_to_the_newest_version() {
        let dir = TempDir::new().unwrap();
        let data_dir = dir.path().join("data");

        let store = Store::open(&data_dir).unwrap();
        assert!(data_dir.join("skald.db").exists());
        assert_eq!(schema_version(&store.conn).unwrap(), MIGRATIONS.len());
        assert_eq!(tables(&store), ["design", "design_version", "repository"]);
        drop(store);

        let store = Store::open(&data_dir).unwrap();
        assert_eq!(schema_version(&store.conn).unwrap(), MIGRATIONS.len());
    }

    #[test]
    fn appended_migration_runs_alone() {
        let dir = TempDir::new().unwrap();
        let mut store = Store::open(dir.path()).unwrap();
        let mut migrations = MIGRATIONS.to_vec();
        migrations.push("CREATE TABLE appended (id TEXT);");

        store.migrate_with(&migrations).unwrap();
        assert_eq!(schema_version(&store.conn).unwrap(), MIGRATIONS.len() + 1);
        assert!(tables(&store).contains(&"appended".to_owned()));

        store.migrate_with(&migrations).unwrap();
        assert_eq!(schema_version(&store.conn).unwrap(), MIGRATIONS.len() + 1);
    }

    #[test]
    fn store_newer_than_the_program_is_refused() {
        let dir = TempDir::new().unwrap();
        let store = Store::open(dir.path()).unwrap();
        store
            .conn
            .pragma_update(None, "user_version", MIGRATIONS.len() as i64 + 1)
            .unwrap();
        drop(store);

        let err = Store::open(dir.path()).err().unwrap();
        assert_eq!(err.code, "store-newer-than-program");
        assert_eq!(err.exit_code, 1);
        assert_eq!(err.help, [UPGRADE]);
    }
}
