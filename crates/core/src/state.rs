use crate::{git::Repository, model::Run, paths};
use anyhow::{Context, Result, bail};
use fs2::FileExt;
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use std::{
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
};

pub struct Lease {
    file: File,
}
impl Lease {
    pub fn acquire(repo: &Repository) -> Result<Self> {
        repo.require_local_runtime()?;
        let runtime = repo.runtime()?;
        fs::create_dir_all(&runtime)?;
        let path = paths::inside(&repo.common, "spec-autonomous/coordinator.lock")?;
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path)?;
        file.try_lock_exclusive()
            .context("run_already_active: another coordinator owns this Git repository")?;
        Ok(Self { file })
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}
pub struct Store {
    db: Connection,
    pub root: PathBuf,
    writable: bool,
}
impl Store {
    pub fn for_control(repo: &Repository) -> Result<Self> {
        let root = repo.runtime()?;
        let path = paths::inside(&repo.common, "spec-autonomous/state.db")?;
        if !path.is_file() {
            bail!("run_not_found: no runtime state");
        }
        let db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
        db.busy_timeout(std::time::Duration::from_millis(250))?;
        let version: u32 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version != 1 {
            bail!("schema_unsupported: controls require current runtime schema");
        }
        Ok(Self {
            db,
            root,
            writable: true,
        })
    }
    pub fn open(repo: &Repository, writable: bool) -> Result<Option<Self>> {
        let root = repo.runtime()?;
        let file = paths::inside(&repo.common, "spec-autonomous/state.db")?;
        if !writable && !file.exists() {
            return Ok(None);
        }
        if writable {
            fs::create_dir_all(&root)?;
        }
        let flags = if writable {
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE
        } else {
            OpenFlags::SQLITE_OPEN_READ_ONLY
        };
        let db = Connection::open_with_flags(&file, flags)?;
        db.busy_timeout(std::time::Duration::from_millis(250))?;
        let version: u32 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version > 1 {
            bail!("schema_unsupported: runtime database is newer than this CLI");
        }
        if writable {
            if version == 0 {
                let tables: u64 = db.query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'", [], |r| r.get(0))?;
                if tables > 0 {
                    let backup = root.join(format!("state.v0.{}.backup.db", uuid::Uuid::new_v4()));
                    // SQLite creates a consistent standalone backup including WAL
                    // data before any schema or journal changes are attempted.
                    db.execute("VACUUM INTO ?1", [backup.to_string_lossy().as_ref()])?;
                    File::open(&backup)?.sync_all()?;
                }
            }
            db.pragma_update(None, "journal_mode", "WAL")?;
            db.pragma_update(None, "synchronous", "FULL")?;
            db.execute_batch("BEGIN IMMEDIATE;
                CREATE TABLE IF NOT EXISTS runs(id TEXT PRIMARY KEY,payload TEXT NOT NULL,updated_at TEXT NOT NULL);
                CREATE TABLE IF NOT EXISTS events(seq INTEGER PRIMARY KEY AUTOINCREMENT,run_id TEXT NOT NULL,kind TEXT NOT NULL,payload TEXT NOT NULL,created_at TEXT NOT NULL);
                CREATE TABLE IF NOT EXISTS controls(run_id TEXT PRIMARY KEY,action TEXT NOT NULL);
                PRAGMA user_version=1; COMMIT;")?;
            let registry = toml::to_string(
                &serde_json::json!({"schema_version":1,"repository_id":paths::hash(repo.common.to_string_lossy().as_bytes()),"ledger":"state.db"}),
            )?;
            paths::atomic_write(&root.join("registry.toml"), registry)?;
        } else if version == 0 {
            bail!("state_unavailable: database is not initialized");
        }
        Ok(Some(Self { db, root, writable }))
    }
    pub fn save(&mut self, run: &Run, kind: &str) -> Result<()> {
        let tx = self.db.transaction()?;
        let body = serde_json::to_string(run)?;
        tx.execute("INSERT INTO runs(id,payload,updated_at) VALUES(?1,?2,?3) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload,updated_at=excluded.updated_at",params![run.id,body,run.updated_at])?;
        tx.execute("INSERT INTO events(run_id,kind,payload,created_at) VALUES(?1,?2,?3,?4)",params![run.id,kind,serde_json::to_string(&serde_json::json!({"stage":run.stage,"status":run.status,"accepted_head":run.accepted_head}))?,paths::now()])?;
        tx.commit()?;
        Ok(())
    }
    pub fn get(&self, id: &str) -> Result<Run> {
        paths::valid_id(id)?;
        let body: Option<String> = self
            .db
            .query_row("SELECT payload FROM runs WHERE id=?1", [id], |r| r.get(0))
            .optional()?;
        serde_json::from_str(&body.context("run_not_found: no such run")?)
            .context("state_corrupt: run payload")
    }
    pub fn list(&self) -> Result<Vec<Run>> {
        let mut q = self
            .db
            .prepare("SELECT payload FROM runs ORDER BY updated_at DESC")?;
        let rows = q.query_map([], |r| r.get::<_, String>(0))?;
        rows.map(|s| Ok(serde_json::from_str(&s?)?)).collect()
    }
    pub fn control(&self, id: &str, action: &str) -> Result<()> {
        self.get(id)?;
        if !["pause", "cancel", ""].contains(&action) {
            bail!("invalid_control: unknown action");
        }
        self.db.execute("INSERT INTO controls(run_id,action) VALUES(?1,?2) ON CONFLICT(run_id) DO UPDATE SET action=excluded.action",params![id,action])?;
        Ok(())
    }
    pub fn requested(&self, id: &str) -> Result<String> {
        Ok(self
            .db
            .query_row("SELECT action FROM controls WHERE run_id=?1", [id], |r| {
                r.get(0)
            })
            .optional()?
            .unwrap_or_default())
    }
    pub fn events(&self, id: &str) -> Result<Vec<serde_json::Value>> {
        let mut q = self.db.prepare(
            "SELECT seq,kind,payload,created_at FROM events WHERE run_id=?1 ORDER BY seq",
        )?;
        let rows = q.query_map([id], |r| {
            Ok((
                r.get::<_, u64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })?;
        rows.map(|row|{let(seq,kind,payload,time)=row?;Ok(serde_json::json!({"seq":seq,"kind":kind,"data":serde_json::from_str::<serde_json::Value>(&payload)?,"time":time}))}).collect()
    }
    pub fn attempt_dir(&self, run: &str, attempt: &str) -> Result<PathBuf> {
        if !self.writable {
            bail!("state_read_only: cannot create attempt directories from a read-only store");
        }
        paths::valid_id(run)?;
        paths::valid_id(attempt)?;
        let path = paths::inside(&self.root, &format!("runs/{run}/attempts/{attempt}"))?;
        fs::create_dir_all(&path)?;
        Ok(path)
    }
}
pub fn report_path(root: &Path, id: &str) -> PathBuf {
    root.join("runs").join(id).join("report.md")
}
