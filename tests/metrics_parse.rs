use hive::metrics::{
    cpu_busy_between, parse_free_m, parse_loadavg, parse_mpstat, parse_mpstat_per_core,
    parse_nproc, parse_proc_stat, parse_ps, parse_uptime, split_proc_stat, MachineStats,
};

#[test]
fn parses_nproc() {
    assert_eq!(parse_nproc("8\n").unwrap(), 8);
}

#[test]
fn parses_free_m() {
    let out = "\
              total        used        free      shared  buff/cache   available\n\
Mem:           15920        4200        8100         200        3620       11200\n\
Swap:           2048           0        2048\n";
    let (used, total) = parse_free_m(out).unwrap();
    assert_eq!(total, 15920);
    assert_eq!(used, 15920 - 11200);
}

#[test]
fn parses_loadavg() {
    assert_eq!(
        parse_loadavg("0.50 0.30 0.20 2/300 12345\n").unwrap(),
        (0.50, 0.30, 0.20)
    );
}

#[test]
fn parses_mpstat_idle() {
    let out = "Linux 6.0 (host) 08/14/26 ...\n\
               %usr %nice %sys %iowait %irq %soft %steal %guest %gnice %idle\n\
               5.00 0.00 2.00 0.00 0.00 0.00 0.00 0.00 0.00 93.00\n";
    assert_eq!(parse_mpstat(out).unwrap(), 93.0);
}

#[test]
fn parses_mpstat_per_core() {
    let out = "Linux 6.0 (host) 08/14/26 ...\n\
               %usr %nice %sys %iowait %irq %soft %steal %guest %gnice %idle\n\
               all  5.00 0.00 2.00 0.00 0.00 0.00 0.00 0.00 0.00 93.00\n\
               0    10.0 0.00 5.00 0.00 0.00 0.00 0.00 0.00 0.00 80.00\n\
               1     0.0 0.00 0.00 0.00 0.00 0.00 0.00 0.00 0.00 99.00\n";
    let per = parse_mpstat_per_core(out);
    assert_eq!(per.len(), 2, "skips the aggregate 'all' row");
    assert_eq!(per[0], 20.0);
    assert_eq!(per[1], 1.0);
}

#[test]
fn parses_mpstat_per_core_empty_for_top_fallback() {
    let out = "top - 12:00:00 up 1 day,  0 users,  load average: 0.00, 0.00, 0.00\n\
               %Cpu(s):  2.0 us,  1.0 sy,  0.0 ni, 97.0 id,  0.0 wa,  0.0 hi\n";
    assert!(parse_mpstat_per_core(out).is_empty());
}

#[test]
fn parses_proc_stat_core_counts() {
    let out = "cpu  100 0 50 800 20 0 0 0 0 0\n\
               cpu0 40 0 20 400 10 0 0 0 0 0\n\
               cpu1 60 0 30 400 10 0 0 0 0 0\n";
    let (agg, per) = parse_proc_stat(out);
    assert!(agg.is_some());
    assert_eq!(per.len(), 2);
    assert_eq!(per[0].0, 0);
    assert_eq!(per[1].0, 1);
}

#[test]
fn proc_stat_per_core_busy() {
    let raw = "cpu  0 0 0 1000 0 0 0 0 0 0\n\
               cpu0 0 0 0 500 0 0 0 0 0 0\n\
               cpu1 0 0 0 500 0 0 0 0 0 0\n\
               ---HIVE-SPLIT---\n\
               cpu  100 0 0 1100 0 0 0 0 0 0\n\
               cpu0 100 0 0 500 0 0 0 0 0 0\n\
               cpu1 0 0 0 600 0 0 0 0 0 0\n";
    let (prev, cur) = split_proc_stat(raw);
    let (agg, per) = cpu_busy_between(&prev, &cur);
    assert_eq!(agg, 50.0);
    assert_eq!(per.len(), 2);
    assert_eq!(per[0], 100.0);
    assert_eq!(per[1], 0.0);
}

#[test]
fn parses_uptime() {
    assert!(parse_uptime(" 12345.67 2.00\n").unwrap() > 12000.0);
}

#[test]
fn builds_stats_struct() {
    let s = MachineStats {
        cores: 8,
        cpu_percent: 7.0,
        cpu_per_core: vec![5.0, 10.0, 3.0, 8.0, 6.0, 4.0, 9.0, 7.0],
        mem_used_mib: 2000,
        mem_total_mib: 15920,
        load1: 0.5,
        load5: 0.3,
        load15: 0.2,
        uptime_secs: 12345.0,
        top_procs: vec![],
    };
    assert!(s.cpu_percent < 100.0);

    let s2 = MachineStats {
        cores: 8,
        cpu_percent: 7.0,
        cpu_per_core: vec![5.0, 10.0, 3.0, 8.0, 6.0, 4.0, 9.0, 7.0],
        mem_used_mib: 2000,
        mem_total_mib: 15920,
        load1: 0.5,
        load5: 0.3,
        load15: 0.2,
        uptime_secs: 12345.0,
        top_procs: vec![],
    };
    assert_eq!(s, s2);

    let def = MachineStats::default();
    assert_eq!(def.cores, 0u32);
    assert_eq!(def.cpu_percent, 0.0);
    assert!(def.cpu_per_core.is_empty());
    assert_eq!(def.mem_used_mib, 0);
    assert_eq!(def.mem_total_mib, 0);
    assert_eq!(def.load1, 0.0);
    assert_eq!(def.load5, 0.0);
    assert_eq!(def.load15, 0.0);
    assert_eq!(def.uptime_secs, 0.0);
}

#[test]
fn parses_ps_output() {
    let out = "  PID COMMAND          %CPU %MEM\n  123 mpirun           45.0  2.1\n  456 bash              0.5  0.2\n  bad line here\n";
    let procs = parse_ps(out);
    assert_eq!(procs.len(), 2, "skips header and malformed lines");
    assert_eq!(procs[0].pid, 123);
    assert_eq!(procs[0].comm, "mpirun");
    assert_eq!(procs[0].cpu, 45.0);
}
