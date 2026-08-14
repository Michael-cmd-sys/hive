use hive::config::{Auth, ClusterConfig};

#[test]
fn roundtrips_password_machine() {
    let yaml = r#"
poll_interval_ms: 2000
mpi:
  launcher: mpirun
  default_args: "-bind-to core"
machines:
  - name: node1
    host: 192.168.1.20
    port: 22
    user: alice
    auth:
      method: password
      password: "secret"
    tags: [gpu, lab]
"#;
    let cfg: ClusterConfig = serde_yaml::from_str(yaml).expect("parse");
    assert_eq!(cfg.machines.len(), 1);
    let m = &cfg.machines[0];
    assert_eq!(m.name, "node1");
    assert_eq!(m.port, 22);
    assert!(matches!(m.auth, Auth::Password { .. }));
    let out = serde_yaml::to_string(&cfg).unwrap();
    let cfg2: ClusterConfig = serde_yaml::from_str(&out).unwrap();
    assert_eq!(cfg2.machines[0].name, "node1");
}

#[test]
fn rejects_missing_auth_secret() {
    let yaml = r#"
machines:
  - name: n
    host: h
    user: u
    auth:
      method: password
"#;
    let r: Result<ClusterConfig, _> = serde_yaml::from_str(yaml);
    assert!(r.is_err());
}

#[test]
fn rejects_empty_password_via_load() {
    let yaml = r#"
machines:
  - name: n
    host: h
    user: u
    auth:
      method: password
      password: ""
"#;
    let path = std::env::temp_dir().join(format!("hive_cfg_test_{}.yaml", std::process::id()));
    std::fs::write(&path, yaml).expect("write temp yaml");
    let r = ClusterConfig::load(&path);
    let _ = std::fs::remove_file(&path);
    assert!(r.is_err());
}
