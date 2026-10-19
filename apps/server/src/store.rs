use crate::models::{Artifact, Job};
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::{Path, PathBuf};

pub struct Store {
    connection: Connection,
    pub root: PathBuf,
}

impl Store {
    pub fn open(root: &Path) -> Result<Self> {
        std::fs::create_dir_all(root.join("artifacts"))?;
        std::fs::create_dir_all(root.join("temporary"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o700))?;
        }
        let connection = Connection::open(root.join("most.sqlite3"))?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        connection.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
          CREATE TABLE IF NOT EXISTS artifacts (id TEXT PRIMARY KEY, sha256 TEXT NOT NULL UNIQUE, data TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS jobs (id TEXT PRIMARY KEY, request_key TEXT NOT NULL UNIQUE, request_data TEXT NOT NULL, data TEXT NOT NULL);")?;
        let store = Self {
            connection,
            root: root.to_owned(),
        };
        for mut job in store.jobs()? {
            if !["succeeded", "failed", "cancelled", "needs_reconciliation"]
                .contains(&job.state.as_str())
            {
                job.state = "needs_reconciliation".into();
                job.message = "Program został przerwany. Sprawdź wynik na urządzeniu przed ponowną instalacją.".into();
                job.updated_at = crate::models::now();
                store.update_job(&job)?;
            }
        }
        // Incomplete uploads have no database row and are never treated as valid artifacts.
        for entry in std::fs::read_dir(root.join("temporary"))? {
            let entry = entry?;
            if entry.path().is_file() && entry.file_name().to_string_lossy().ends_with(".upload") {
                std::fs::remove_file(entry.path())?;
            }
        }
        Ok(store)
    }

    pub fn artifacts(&self) -> Result<Vec<Artifact>> {
        self.query_json("SELECT data FROM artifacts ORDER BY rowid DESC")
    }
    pub fn jobs(&self) -> Result<Vec<Job>> {
        self.query_json("SELECT data FROM jobs ORDER BY rowid DESC LIMIT 100")
    }
    fn query_json<T: serde::de::DeserializeOwned>(&self, sql: &str) -> Result<Vec<T>> {
        let mut statement = self.connection.prepare(sql)?;
        let records = statement.query_map([], |row| row.get::<_, String>(0))?;
        records
            .map(|row| Ok(serde_json::from_str(&row?)?))
            .collect()
    }
    pub fn artifact(&self, id: &str) -> Result<Artifact> {
        let data: String = self
            .connection
            .query_row("SELECT data FROM artifacts WHERE id=?1", [id], |row| {
                row.get(0)
            })
            .optional()?
            .context("Nie znaleziono IPA.")?;
        Ok(serde_json::from_str(&data)?)
    }
    pub fn by_hash(&self, hash: &str) -> Result<Option<Artifact>> {
        let data: Option<String> = self
            .connection
            .query_row(
                "SELECT data FROM artifacts WHERE sha256=?1",
                [hash],
                |row| row.get(0),
            )
            .optional()?;
        data.map(|v| serde_json::from_str(&v).map_err(Into::into))
            .transpose()
    }
    pub fn artifact_path(&self, artifact: &Artifact) -> PathBuf {
        self.root
            .join("artifacts")
            .join(format!("{}.ipa", artifact.sha256))
    }
    pub fn add_artifact(&self, artifact: &Artifact) -> Result<()> {
        self.connection.execute(
            "INSERT INTO artifacts (id,sha256,data) VALUES (?1,?2,?3)",
            params![
                artifact.id,
                artifact.sha256,
                serde_json::to_string(artifact)?
            ],
        )?;
        Ok(())
    }
    pub fn job(&self, id: &str) -> Result<Job> {
        let data: String = self
            .connection
            .query_row("SELECT data FROM jobs WHERE id=?1", [id], |row| row.get(0))
            .optional()?
            .context("Nie znaleziono zadania.")?;
        Ok(serde_json::from_str(&data)?)
    }
    pub fn request(&self, key: &str, body: &str) -> Result<Option<Job>> {
        let row: Option<(String, String)> = self
            .connection
            .query_row(
                "SELECT request_data,data FROM jobs WHERE request_key=?1",
                [key],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        if let Some((stored, data)) = row {
            ensure!(
                stored == body,
                "Ten klucz żądania został już użyty z innymi danymi."
            );
            Ok(Some(serde_json::from_str(&data)?))
        } else {
            Ok(None)
        }
    }
    pub fn add_job(&self, job: &Job, key: &str, request: &str) -> Result<()> {
        self.connection.execute(
            "INSERT INTO jobs (id,request_key,request_data,data) VALUES (?1,?2,?3,?4)",
            params![job.id, key, request, serde_json::to_string(job)?],
        )?;
        Ok(())
    }
    pub fn update_job(&self, job: &Job) -> Result<()> {
        ensure!(
            self.connection.execute(
                "UPDATE jobs SET data=?1 WHERE id=?2",
                params![serde_json::to_string(job)?, job.id]
            )? == 1,
            "Zadanie nie istnieje."
        );
        Ok(())
    }
}
