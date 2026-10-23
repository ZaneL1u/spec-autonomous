use crate::{git::Repository, model::Run, paths};
use anyhow::{Context, Result, bail};
use fs2::FileExt;
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use std::{
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
};

/// Bounded wait for the coordinator lock before reporting another active coordinator.
pub const LOCK_WAIT: std::time::Duration = std::time::Duration::from_secs(10);
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
        // Concurrent host calls (a heartbeat racing a receipt, two claims) are normal.
        // Wait briefly for the lock instead of failing them straight away.
        let deadline = std::time::Instant::now() + LOCK_WAIT;
        loop {
            match file.try_lock_exclusive() {
                Ok(()) => return Ok(Self { file }),
                Err(error) if std::time::Instant::now() < deadline => {
                    let _ = error;
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                Err(error) => {
                    return Err(error).context(
                        "run_already_active: another coordinator owns this Git repository",
                    );
                }
            }
        }
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}
/// Schema 4 keeps the run core in `runs` and stores attempts and evidence as rows, so a
/// heartbeat or a single check writes one row instead of the whole multi-megabyte run.
pub const SCHEMA_VERSION: u32 = 4;
pub struct Store {
    db: Connection,
    pub root: PathBuf,
    writable: bool,
    split: bool,
    /// Per-run hashes of attempt rows written by this store, to skip unchanged rows.
    saved: std::collections::BTreeMap<String, Vec<String>>,
}
/// Write the run core and only the attempt/evidence rows that changed.
fn write_split(
    tx: &rusqlite::Transaction<'_>,
    run: &Run,
    updated_at: &str,
    known_hashes: Option<&[String]>,
) -> Result<Vec<String>> {
    let mut core = run.clone();
    core.attempts = vec![];
    core.evidence = vec![];
    let body = serde_json::to_string(&core)?;
    tx.execute("INSERT INTO runs(id,payload,updated_at) VALUES(?1,?2,?3) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload,updated_at=excluded.updated_at",params![run.id,body,updated_at])?;
    let known: Vec<String> = match known_hashes {
        Some(hashes) => hashes.to_vec(),
        None => {
            let mut q = tx.prepare("SELECT hash FROM run_attempts WHERE run_id=?1 ORDER BY idx")?;
            let rows = q.query_map([&run.id], |r| r.get::<_, String>(0))?;
            rows.collect::<std::result::Result<_, _>>()?
        }
    };
    let mut hashes = Vec::with_capacity(run.attempts.len());
    for (idx, attempt) in run.attempts.iter().enumerate() {
        let payload = serde_json::to_string(attempt)?;
        let hash = paths::hash(&payload);
        if known.get(idx) != Some(&hash) {
            tx.execute("INSERT INTO run_attempts(run_id,idx,hash,payload) VALUES(?1,?2,?3,?4) ON CONFLICT(run_id,idx) DO UPDATE SET hash=excluded.hash,payload=excluded.payload",params![run.id,idx as i64,hash,payload])?;
        }
        hashes.push(hash);
    }
    if known.len() > run.attempts.len() {
        tx.execute(
            "DELETE FROM run_attempts WHERE run_id=?1 AND idx>=?2",
            params![run.id, run.attempts.len() as i64],
        )?;
    }
    let stored: i64 = tx.query_row(
        "SELECT count(*) FROM run_evidence WHERE run_id=?1",
        [&run.id],
        |r| r.get(0),
    )?;
    let stored = stored.max(0) as usize;
    if stored > run.evidence.len() {
        tx.execute("DELETE FROM run_evidence WHERE run_id=?1", [&run.id])?;
    }
    let start = if stored > run.evidence.len() {
        0
    } else {
        stored
    };
    for (idx, evidence) in run.evidence.iter().enumerate().skip(start) {
        tx.execute("INSERT INTO run_evidence(run_id,idx,payload) VALUES(?1,?2,?3) ON CONFLICT(run_id,idx) DO UPDATE SET payload=excluded.payload",params![run.id,idx as i64,serde_json::to_string(evidence)?])?;
    }
    Ok(hashes)
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
        if !(2..=SCHEMA_VERSION).contains(&version) {
            bail!("schema_unsupported: controls require current runtime schema");
        }
        Ok(Self {
            db,
            root,
            writable: true,
            split: version >= SCHEMA_VERSION,
            saved: Default::default(),
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
        let mut db = Connection::open_with_flags(&file, flags)?;
        db.busy_timeout(std::time::Duration::from_millis(250))?;
        let version: u32 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version > SCHEMA_VERSION {
            bail!("schema_unsupported: runtime database is newer than this CLI");
        }
        if writable {
            if version < SCHEMA_VERSION {
                let tables: u64 = db.query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'", [], |r| r.get(0))?;
                if tables > 0 {
                    let backup = root.join(format!(
                        "state.v{version}.{}.backup.db",
                        uuid::Uuid::new_v4()
                    ));
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
                CREATE TABLE IF NOT EXISTS run_attempts(run_id TEXT NOT NULL,idx INTEGER NOT NULL,hash TEXT NOT NULL,payload TEXT NOT NULL,PRIMARY KEY(run_id,idx));
                CREATE TABLE IF NOT EXISTS run_evidence(run_id TEXT NOT NULL,idx INTEGER NOT NULL,payload TEXT NOT NULL,PRIMARY KEY(run_id,idx));
                COMMIT;")?;
            if version < SCHEMA_VERSION {
                // Explode the monolithic payloads of schema 2/3 rows into the split tables so
                // every later save writes only the rows that changed.
                let tx = db.transaction()?;
                let rows: Vec<(String, String, String)> = {
                    let mut q =
                        tx.prepare("SELECT id, payload, updated_at FROM runs ORDER BY updated_at")?;
                    let rows = q.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
                    rows.collect::<std::result::Result<_, _>>()?
                };
                for (id, payload, updated_at) in rows {
                    let run: Run = serde_json::from_str(&payload)
                        .with_context(|| format!("state_corrupt: run payload {id}"))?;
                    write_split(&tx, &run, &updated_at, None)?;
                }
                tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
                tx.commit()?;
            }
            let registry = toml::to_string(
                &serde_json::json!({"schema_version":1,"repository_id":paths::hash(repo.common.to_string_lossy().as_bytes()),"ledger":"state.db"}),
            )?;
            paths::atomic_write(&root.join("registry.toml"), registry)?;
        } else if version == 0 {
            bail!("state_unavailable: database is not initialized");
        }
        // A read-only opener of an older ledger still reads its monolithic payloads.
        let split = writable || version >= SCHEMA_VERSION;
        Ok(Some(Self {
            db,
            root,
            writable,
            split,
            saved: Default::default(),
        }))
    }
    pub fn save(&mut self, run: &Run, kind: &str) -> Result<()> {
        let tx = self.db.transaction()?;
        let known = self.saved.get(&run.id).cloned();
        let hashes = write_split(&tx, run, &run.updated_at, known.as_deref())?;
        tx.execute("INSERT INTO events(run_id,kind,payload,created_at) VALUES(?1,?2,?3,?4)",params![run.id,kind,serde_json::to_string(&serde_json::json!({"stage":run.stage,"status":run.status,"accepted_head":run.accepted_head}))?,paths::now()])?;
        tx.commit()?;
        // Only rows that actually committed may be skipped next time.
        self.saved.insert(run.id.clone(), hashes);
        Ok(())
    }
    pub fn get(&self, id: &str) -> Result<Run> {
        paths::valid_id(id)?;
        let body: Option<String> = self
            .db
            .query_row("SELECT payload FROM runs WHERE id=?1", [id], |r| r.get(0))
            .optional()?;
        let mut run: Run = serde_json::from_str(&body.context("run_not_found: no such run")?)
            .context("state_corrupt: run payload")?;
        if self.split {
            self.load_rows(&mut run)?;
        }
        Ok(run)
    }
    pub fn list(&self) -> Result<Vec<Run>> {
        let mut q = self
            .db
            .prepare("SELECT payload FROM runs ORDER BY updated_at DESC")?;
        let rows = q.query_map([], |r| r.get::<_, String>(0))?;
        let mut runs: Vec<Run> = rows
            .map(|s| Ok(serde_json::from_str(&s?)?))
            .collect::<Result<_>>()?;
        if self.split {
            for run in &mut runs {
                self.load_rows(run)?;
            }
        }
        Ok(runs)
    }
    fn load_rows(&self, run: &mut Run) -> Result<()> {
        let mut q = self
            .db
            .prepare("SELECT payload FROM run_attempts WHERE run_id=?1 ORDER BY idx")?;
        let attempts = q.query_map([&run.id], |r| r.get::<_, String>(0))?;
        run.attempts = attempts
            .map(|s| serde_json::from_str(&s?).context("state_corrupt: attempt row"))
            .collect::<Result<_>>()?;
        let mut q = self
            .db
            .prepare("SELECT payload FROM run_evidence WHERE run_id=?1 ORDER BY idx")?;
        let evidence = q.query_map([&run.id], |r| r.get::<_, String>(0))?;
        run.evidence = evidence
            .map(|s| serde_json::from_str(&s?).context("state_corrupt: evidence row"))
            .collect::<Result<_>>()?;
        Ok(())
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
    pub fn user_version(&self) -> Result<u32> {
        Ok(self
            .db
            .pragma_query_value(None, "user_version", |row| row.get(0))?)
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
