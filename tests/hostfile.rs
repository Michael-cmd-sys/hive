use hive::jobs::build_hostfile;
use hive::jobs::Worker;

#[test]
fn builds_hostfile_lines() {
    let workers = vec![
        Worker {
            host: "192.168.1.20".into(),
            slots: 8,
        },
        Worker {
            host: "192.168.1.21".into(),
            slots: 4,
        },
    ];
    let hf = build_hostfile(&workers);
    assert_eq!(hf, "192.168.1.20:8\n192.168.1.21:4\n");
}
