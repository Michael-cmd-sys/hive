#[derive(Debug, Clone, Default, PartialEq)]
pub struct MachineStats {
    pub cores: u32,
    pub cpu_percent: f32,
    /// Per-core CPU busy % (0-100), one entry per logical CPU. This is the
    /// primary signal — a single aggregate hides which cores are saturated.
    pub cpu_per_core: Vec<f32>,
    pub mem_used_mib: u64,
    pub mem_total_mib: u64,
    pub load1: f32,
    pub load5: f32,
    pub load15: f32,
    pub uptime_secs: f64,
    /// Top processes by CPU on the node (collected via `ps`).
    pub top_procs: Vec<ProcInfo>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProcInfo {
    pub pid: u32,
    pub comm: String,
    pub cpu: f32,
    pub mem: f32,
}

pub fn parse_nproc(s: &str) -> anyhow::Result<u32> {
    Ok(s.trim().parse()?)
}

pub fn parse_free_m(s: &str) -> anyhow::Result<(u64, u64)> {
    for line in s.lines() {
        if line.starts_with("Mem:") {
            let cols: Vec<&str> = line.split_whitespace().collect();
            let total: u64 = cols
                .get(1)
                .ok_or_else(|| anyhow::anyhow!("bad free output"))?
                .parse()?;
            if cols.len() >= 7 {
                let avail: u64 = cols
                    .get(6)
                    .ok_or_else(|| anyhow::anyhow!("bad free output"))?
                    .parse()?;
                return Ok((total - avail, total));
            }
            if cols.len() < 3 {
                anyhow::bail!("bad free output");
            }
            let used: u64 = cols[2].parse()?;
            return Ok((used, total));
        }
    }
    anyhow::bail!("no Mem: line in free output")
}

pub fn parse_loadavg(s: &str) -> anyhow::Result<(f32, f32, f32)> {
    let cols: Vec<&str> = s.split_whitespace().collect();
    let load1: f32 = cols
        .first()
        .ok_or_else(|| anyhow::anyhow!("bad loadavg output"))?
        .parse()?;
    let load5: f32 = cols
        .get(1)
        .ok_or_else(|| anyhow::anyhow!("bad loadavg output"))?
        .parse()?;
    let load15: f32 = cols
        .get(2)
        .ok_or_else(|| anyhow::anyhow!("bad loadavg output"))?
        .parse()?;
    Ok((load1, load5, load15))
}

/// Parse `mpstat -P ALL` output into per-CPU busy % (one value per core).
/// The aggregate `all` row is skipped; the trailing `%idle` column of each
/// per-core row is converted to busy % (100 - idle). Returns an empty vec when
/// no per-core rows are present (e.g. the `top` fallback was used instead).
pub fn parse_mpstat_per_core(s: &str) -> Vec<f32> {
    let mut out = Vec::new();
    for line in s.lines() {
        let cols: Vec<&str> = line.split_whitespace().collect();
        for (i, c) in cols.iter().enumerate() {
            // The CPU column is the token "all" (aggregate) or a core index.
            // A real per-core row has a numeric metric immediately after it;
            // this guards against matching stray integers (e.g. "0 users" in
            // `top` output), which have a non-numeric next token.
            if *c == "all" {
                break;
            }
            if c.parse::<u32>().is_err() {
                continue;
            }
            let Some(next) = cols.get(i + 1) else { break };
            if next.parse::<f32>().is_err() {
                break;
            }
            if let Some(idle_str) = cols.last() {
                if let Ok(idle) = idle_str.trim_end_matches(',').parse::<f32>() {
                    out.push((100.0 - idle).clamp(0.0, 100.0));
                }
            }
            break;
        }
    }
    out
}

/// Returns idle % (not busy %); subtract from 100.0 to get CPU usage.
///
/// The header line contains `%idle`, so it has no trailing numeric column
/// and is skipped by the filter. The last numeric data line carries the
/// average idle % across the sampling window.
pub fn parse_mpstat(s: &str) -> anyhow::Result<f32> {
    let line = s
        .lines()
        .rev()
        .find(|l| {
            l.split_whitespace()
                .next_back()
                .is_some_and(|t| t.parse::<f32>().is_ok())
        })
        .ok_or_else(|| anyhow::anyhow!("no mpstat data"))?;
    let cols: Vec<&str> = line.split_whitespace().collect();
    let idle: f32 = cols
        .iter()
        .next_back()
        .ok_or_else(|| anyhow::anyhow!("empty"))?
        .parse()?;
    Ok(idle)
}

pub fn parse_uptime(s: &str) -> anyhow::Result<f64> {
    let first = s
        .split_whitespace()
        .next()
        .ok_or_else(|| anyhow::anyhow!("empty"))?;
    Ok(first.parse()?)
}

/// Parse `ps -eo pid,comm,%cpu,%mem` output into process records.
pub fn parse_ps(s: &str) -> Vec<ProcInfo> {
    let mut out = Vec::new();
    for line in s.lines().skip(1) {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 4 {
            continue;
        }
        let pid = match cols[0].parse::<u32>() {
            Ok(p) => p,
            Err(_) => continue,
        };
        let comm = cols[1].to_string();
        let cpu = cols[2].parse::<f32>().unwrap_or(0.0);
        let mem = cols[3].parse::<f32>().unwrap_or(0.0);
        out.push(ProcInfo {
            pid,
            comm,
            cpu,
            mem,
        });
    }
    out
}

/// A parsed snapshot of `/proc/stat`: the aggregate `cpu` line (if present)
/// and one entry per logical core as `(core_id, total_jiffies, idle_jiffies)`.
/// `idle_jiffies` includes iowait, matching how `mpstat` reports idle.
pub type CpuSnapshot = (Option<(u64, u64)>, Vec<(u32, u64, u64)>);

/// Parse `/proc/stat` into a [`CpuSnapshot`]. Works on any Linux host with no
/// external tools, unlike `mpstat` (sysstat) which is often not installed.
pub fn parse_proc_stat(s: &str) -> CpuSnapshot {
    let mut agg = None;
    let mut per = Vec::new();
    for line in s.lines() {
        let mut it = line.split_whitespace();
        let Some(label) = it.next() else { continue };
        if !label.starts_with("cpu") {
            continue;
        }
        let rest = &label[3..];
        let cols: Vec<u64> = it.filter_map(|c| c.parse().ok()).collect();
        if cols.len() < 4 {
            continue;
        }
        let idle = cols[3];
        let iowait = cols.get(4).copied().unwrap_or(0);
        let total = cols.iter().sum::<u64>();
        let idle_total = idle + iowait;
        if rest.is_empty() {
            agg = Some((total, idle_total));
        } else if let Ok(core) = rest.parse::<u32>() {
            per.push((core, total, idle_total));
        }
    }
    (agg, per)
}

/// Split raw `cat /proc/stat` output that was captured twice with a
/// `---HIVE-SPLIT---` marker between samples.
pub fn split_proc_stat(raw: &str) -> (CpuSnapshot, CpuSnapshot) {
    let mut parts = raw.splitn(2, "---HIVE-SPLIT---");
    let prev = parts.next().map(parse_proc_stat).unwrap_or_default();
    let cur = parts.next().map(parse_proc_stat).unwrap_or_default();
    (prev, cur)
}

/// Compute per-core (and aggregate) busy % between two `/proc/stat` snapshots.
/// Busy is derived from jiffy deltas, so no wall-clock measurement is needed.
pub fn cpu_busy_between(prev: &CpuSnapshot, cur: &CpuSnapshot) -> (f32, Vec<f32>) {
    let agg = match (prev.0, cur.0) {
        (Some((pt, pi)), Some((ct, ci))) => {
            let dt = ct.saturating_sub(pt);
            let di = ci.saturating_sub(pi);
            if dt > 0 {
                (dt - di.min(dt)) as f32 / dt as f32 * 100.0
            } else {
                0.0
            }
        }
        _ => 0.0,
    };
    let per: Vec<f32> = cur
        .1
        .iter()
        .filter_map(|(id, ct, ci)| {
            prev.1
                .iter()
                .find(|(pid, _, _)| pid == id)
                .map(|(_, pt, pi)| {
                    let dt = ct.saturating_sub(*pt);
                    let di = ci.saturating_sub(*pi);
                    if dt > 0 {
                        ((dt - di.min(dt)) as f32 / dt as f32 * 100.0).clamp(0.0, 100.0)
                    } else {
                        0.0
                    }
                })
        })
        .collect();
    (agg, per)
}
