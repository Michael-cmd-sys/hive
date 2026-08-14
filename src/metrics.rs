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
            let total: u64 = cols[1].parse()?;
            if cols.len() >= 7 {
                let avail: u64 = cols[6].parse()?;
                return Ok((total - avail, total));
            }
            let used: u64 = cols[2].parse()?;
            return Ok((used, total));
        }
    }
    anyhow::bail!("no Mem: line in free output")
}

pub fn parse_loadavg(s: &str) -> anyhow::Result<(f32, f32, f32)> {
    let cols: Vec<&str> = s.split_whitespace().collect();
    Ok((cols[0].parse()?, cols[1].parse()?, cols[2].parse()?))
}

pub fn parse_mpstat(s: &str) -> anyhow::Result<f32> {
    let line = s
        .lines()
        .filter(|l| {
            l.split_whitespace()
                .last()
                .map_or(false, |t| t.parse::<f32>().is_ok())
        })
        .last()
        .ok_or_else(|| anyhow::anyhow!("no mpstat data"))?;
    let cols: Vec<&str> = line.split_whitespace().collect();
    let idle: f32 = cols
        .last()
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
