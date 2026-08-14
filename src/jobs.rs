use crate::config::ClusterConfig;
use crate::error::{HiveError, Result};
use crate::ssh::Session;

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
    let remote = "/tmp/hive_hostfile";
    let put = format!("cat > {remote} <<'EOF'\n{hf}EOF");
    head.exec(&put).await?;
    let launcher = &cfg.mpi.launcher;
    let defaults = &cfg.mpi.default_args;
    let cmd = format!("{launcher} -hostfile {remote} {defaults} {args} {binary}");
    let o = head.exec(&cmd).await?;
    Ok(format!("exit={}\n{}\n{}", o.exit, o.stdout, o.stderr))
}
