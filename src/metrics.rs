#[derive(Debug, Clone, Default, PartialEq)]
pub struct MachineStats {
    pub cores: u32,
    pub cpu_percent: f32,
    pub mem_used_mib: u64,
    pub mem_total_mib: u64,
    pub load1: f32,
    pub load5: f32,
    pub load15: f32,
    pub uptime_secs: f64,
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

/// Returns idle % (not busy %); subtract from 100.0 to get CPU usage.
pub fn parse_mpstat(s: &str) -> anyhow::Result<f32> {
    // The header line contains `%idle`, so it has no trailing numeric column
    // and is skipped by the filter. The last numeric data line carries the
    // average idle % across the sampling window.
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
