use crate::config::ClusterConfig;
use crate::error::{HiveError, Result};
use crate::ssh::Session;
use std::sync::atomic::{AtomicU64, Ordering};

static HF_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone)]
pub struct Worker {
    pub host: String,
    pub slots: u32,
}

/// One `host:slots` line per worker, as used by mpirun -hostfile.
pub fn build_hostfile(workers: &[Worker]) -> String {
    let mut s = String::new();
    for w in workers {
        s.push_str(&format!("{}:{}\n", w.host, w.slots));
    }
    s
}

/// Run a command on an already-open session, return full output.
pub async fn run_on(session: &mut Session, cmd: &str) -> Result<String> {
    let o = session.exec(cmd).await?;
    if o.exit != 0 {
        return Err(HiveError::Command { host: session.name.clone(), code: o.exit as i32, stderr: o.stderr });
    }
    Ok(o.stdout)
}

/// Launch an MPI job from `head` across `workers`.
/// Assumes mpirun/mpiexec + the target binary already exist on the nodes.
pub async fn dispatch_mpi(
    head: &mut Session,
    cfg: &ClusterConfig,
    workers: &[Worker],
    binary: &str,
    args: &str,
) -> Result<String> {
    let hf = build_hostfile(workers);
    let remote = format!(
        "/tmp/hive_hostfile_{}_{}",
        std::process::id(),
        HF_COUNTER.fetch_add(1, Ordering::SeqCst)
    );
    let put = format!("cat > {remote} <<'EOF'\n{hf}EOF");
    let o = head.exec(&put).await?;
    if o.exit != 0 {
        return Err(HiveError::Command {
            host: head.name.clone(),
            code: o.exit as i32,
            stderr: o.stderr,
        });
    }
    let launcher = &cfg.mpi.launcher;
    let defaults = &cfg.mpi.default_args;
    // `args` is passed through unquoted by design so callers can supply raw
    // mpirun flags; only `binary` is quoted to avoid word-splitting on spaces.
    let cmd = format!("{launcher} -hostfile {remote} {defaults} {args} \"{binary}\"");
    let o = head.exec(&cmd).await?;
    let _ = head.exec(&format!("rm -f {remote}")).await;
    if o.exit != 0 {
        return Err(HiveError::Command {
            host: head.name.clone(),
            code: o.exit as i32,
            stderr: format!("{}\n{}", o.stdout, o.stderr),
        });
    }
    Ok(format!("exit={}\n{}\n{}", o.exit, o.stdout, o.stderr))
}
