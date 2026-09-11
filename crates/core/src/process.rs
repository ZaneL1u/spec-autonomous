//! Bounded subprocess execution shared by runners and verification commands.
use crate::paths;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::Write,
    path::Path,
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Identity {
    pub pid: u32,
    pub identity: String,
}
#[derive(Debug)]
pub struct Output {
    pub code: i32,
    pub identity: Identity,
    pub timed_out: bool,
    pub cancelled: bool,
}
pub fn identity(pid: u32) -> String {
    #[cfg(unix)]
    {
        Command::new("ps")
            .args(["-p", &pid.to_string(), "-o", "lstart=", "-o", "stat="])
            .output()
            .ok()
            .filter(|x| x.status.success())
            .and_then(|x| {
                let s = String::from_utf8_lossy(&x.stdout).trim().to_string();
                let (start, status) = s.rsplit_once(' ')?;
                if status.starts_with('Z') {
                    None
                } else {
                    Some(start.trim().to_string())
                }
            })
            .unwrap_or_default()
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::{Foundation::*, System::Threading::*};
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                return if GetLastError() == ERROR_INVALID_PARAMETER {
                    String::new()
                } else {
                    "unknown".into()
                };
            }
            let mut exit = 0;
            let mut created: FILETIME = std::mem::zeroed();
            let mut ended: FILETIME = std::mem::zeroed();
            let mut kernel: FILETIME = std::mem::zeroed();
            let mut user: FILETIME = std::mem::zeroed();
            let valid = GetExitCodeProcess(handle, &mut exit) != 0
                && GetProcessTimes(handle, &mut created, &mut ended, &mut kernel, &mut user) != 0;
            CloseHandle(handle);
            if !valid {
                "unknown".into()
            } else if exit != 259 {
                String::new()
            } else {
                format!("{}:{}", created.dwHighDateTime, created.dwLowDateTime)
            }
        }
    }
}
#[cfg(unix)]
fn terminate(child: &mut Child) {
    // Child was created in its own process group; target the group, never the host.
    unsafe {
        libc::kill(-(child.id() as i32), libc::SIGTERM);
    }
    thread::sleep(Duration::from_millis(100));
    unsafe {
        libc::kill(-(child.id() as i32), libc::SIGKILL);
    }
    let _ = child.wait();
}
#[cfg(windows)]
fn terminate(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

pub struct Request<'a> {
    pub argv: &'a [String],
    pub cwd: &'a Path,
    pub env: &'a BTreeMap<String, String>,
    pub stdin: Option<&'a [u8]>,
    pub directory: &'a Path,
    pub timeout: Duration,
    pub max_log_bytes: u64,
    pub cancel: Arc<AtomicBool>,
}
pub fn execute(req: Request<'_>) -> Result<Output> {
    if req.argv.is_empty() {
        bail!("runner_unavailable: no executable configured");
    }
    fs::create_dir_all(req.directory)?;
    let stdout_path = req.directory.join("stdout.log");
    let stderr_path = req.directory.join("stderr.log");
    let mut cmd = Command::new(&req.argv[0]);
    cmd.args(&req.argv[1..])
        .current_dir(req.cwd)
        .envs(req.env)
        .stdin(if req.stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::from(File::create(&stdout_path)?))
        .stderr(Stdio::from(File::create(&stderr_path)?));
    // Node's test/IPC protocol belongs to the parent harness. Inheriting it can
    // make `node --test` exit zero without executing any assertions.
    for key in [
        "NODE_TEST_CONTEXT",
        "NODE_TEST_WORKER_ID",
        "NODE_CHANNEL_FD",
        "NODE_CHANNEL_SERIALIZATION_MODE",
        "NODE_UNIQUE_ID",
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_COMMON_DIR",
    ] {
        cmd.env_remove(key);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let mut child = cmd
        .spawn()
        .with_context(|| format!("runner_unavailable: cannot start {}", req.argv[0]))?;
    #[cfg(windows)]
    let _job = match job::attach(&child) {
        Ok(job) => job,
        Err(error) => {
            terminate(&mut child);
            return Err(error);
        }
    };
    let record = Identity {
        pid: child.id(),
        identity: identity(child.id()),
    };
    if let Err(error) = paths::atomic_write(
        &req.directory.join("process.json"),
        serde_json::to_vec(&record)?,
    ) {
        terminate(&mut child);
        return Err(error);
    }
    if let Some(input) = req.stdin {
        if let Some(mut pipe) = child.stdin.take() {
            let input = input.to_vec();
            thread::spawn(move || {
                let _ = pipe.write_all(&input);
            });
        }
    }
    let start = Instant::now();
    loop {
        let cancelled = req.cancel.load(Ordering::Relaxed);
        let timed_out = start.elapsed() >= req.timeout;
        let oversized = [&stdout_path, &stderr_path]
            .iter()
            .any(|p| fs::metadata(p).is_ok_and(|m| m.len() > req.max_log_bytes));
        if cancelled || timed_out || oversized {
            terminate(&mut child);
            if oversized {
                bail!("log_limit_exceeded: subprocess exceeded log quota");
            }
            return Ok(Output {
                code: if cancelled { 130 } else { 124 },
                identity: record,
                timed_out,
                cancelled,
            });
        }
        if let Some(status) = child.try_wait()? {
            // A finished parent must not leave descendants writing to the worktree.
            #[cfg(unix)]
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGKILL);
            }
            return Ok(Output {
                code: status.code().unwrap_or(1),
                identity: record,
                timed_out: false,
                cancelled: false,
            });
        }
        thread::sleep(Duration::from_millis(25));
    }
}
pub fn reconcile_process(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let record: Identity = serde_json::from_slice(&fs::read(path)?)?;
    let current = identity(record.pid);
    if current.is_empty() {
        #[cfg(unix)]
        {
            if unsafe { libc::kill(record.pid as i32, 0) } == 0
                || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
            {
                bail!("process_identity_unknown: process exists but identity inspection failed");
            }
        }
        return Ok(());
    }
    if record.identity.is_empty() || current == "unknown" || current != record.identity {
        bail!("process_identity_unknown: refuse to signal a reused or unknown PID");
    }
    #[cfg(unix)]
    {
        if record.pid <= 1
            || record.pid == std::process::id()
            || unsafe { libc::getpgid(record.pid as i32) } != record.pid as i32
        {
            bail!("process_identity_unknown: not an owned process group leader");
        }
        unsafe {
            libc::kill(-(record.pid as i32), libc::SIGTERM);
        }
        thread::sleep(Duration::from_millis(100));
        unsafe {
            libc::kill(-(record.pid as i32), libc::SIGKILL);
        }
        for _ in 0..100 {
            if !group_alive(record.pid)? {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(10));
        }
        bail!("process_still_running: old worker did not stop");
    }
    #[cfg(windows)]
    {
        bail!(
            "process_identity_unknown: restart recovery requires verifying the previous Windows worker is stopped"
        );
    }
}

#[cfg(unix)]
fn group_alive(group: u32) -> Result<bool> {
    let output = Command::new("ps")
        .args(["-axo", "pgid=,stat="])
        .output()
        .context("process_identity_unknown: cannot inspect process group")?;
    if !output.status.success() {
        bail!("process_identity_unknown: process group query failed");
    }
    Ok(String::from_utf8_lossy(&output.stdout).lines().any(|line| {
        let mut p = line.split_whitespace();
        p.next().and_then(|s| s.parse::<u32>().ok()) == Some(group)
            && p.next().is_some_and(|state| !state.starts_with('Z'))
    }))
}

#[cfg(windows)]
mod job {
    use anyhow::{Result, bail};
    use std::{os::windows::io::AsRawHandle, process::Child};
    use windows_sys::Win32::{Foundation::CloseHandle, System::JobObjects::*};
    pub struct Job(windows_sys::Win32::Foundation::HANDLE);
    impl Drop for Job {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
    pub fn attach(child: &Child) -> Result<Job> {
        unsafe {
            let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if handle.is_null() {
                bail!("job_unavailable: CreateJobObjectW failed");
            }
            let job = Job(handle);
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &info as *const _ as _,
                std::mem::size_of_val(&info) as u32,
            ) == 0
                || AssignProcessToJobObject(handle, child.as_raw_handle() as _) == 0
            {
                bail!("job_unavailable: cannot contain child process");
            }
            Ok(job)
        }
    }
}
