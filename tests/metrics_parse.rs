use hive::metrics::{
    parse_free_m, parse_loadavg, parse_mpstat, parse_nproc, parse_uptime, MachineStats,
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
fn parses_uptime() {
    assert!(parse_uptime(" 12345.67 2.00\n").unwrap() > 12000.0);
}

#[test]
fn builds_stats_struct() {
    let s = MachineStats {
        cores: 8,
        cpu_percent: 7.0,
        mem_used_mib: 2000,
        mem_total_mib: 15920,
        load1: 0.5,
        load5: 0.3,
        load15: 0.2,
        uptime_secs: 12345.0,
    };
    assert!(s.cpu_percent < 100.0);
}
