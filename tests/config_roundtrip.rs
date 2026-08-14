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
    tags: [gpu, lab]
"#;
    let cfg: ClusterConfig = serde_yaml::from_str(yaml).expect("parse");
    assert_eq!(cfg.machines.len(), 1);
    let m = &cfg.machines[0];
    assert_eq!(m.name, "node1");
    assert_eq!(m.port, 22);
    assert!(matches!(m.auth, Auth::Password));
    let out = serde_yaml::to_string(&cfg).unwrap();
    let cfg2: ClusterConfig = serde_yaml::from_str(&out).unwrap();
    assert_eq!(cfg2.machines[0].name, "node1");
}

#[test]
fn password_secret_is_never_serialized() {
    // A stray `password:` field in the source must be ignored (not carried),
    // proving plaintext secrets are never read from or written to disk.
    let yaml = r#"
machines:
  - name: node1
    host: 192.168.1.20
    user: alice
    auth:
      method: password
      password: "hunter2"
"#;
    let cfg: ClusterConfig = serde_yaml::from_str(yaml).expect("parse");
    assert!(matches!(cfg.machines[0].auth, Auth::Password));
    let out = serde_yaml::to_string(&cfg).unwrap();
    assert!(
        !out.contains("hunter2"),
        "secret leaked into serialized config"
    );
    assert!(!out.contains("password:"), "password field serialized");
}

#[test]
fn rejects_empty_key_path_via_load() {
    let yaml = r#"
machines:
  - name: n
    host: h
    user: u
    auth:
      method: key
      key_path: ""
"#;
    let path = std::env::temp_dir().join(format!("hive_cfg_test_{}.yaml", std::process::id()));
    std::fs::write(&path, yaml).expect("write temp yaml");
    let r = ClusterConfig::load(&path);
    let _ = std::fs::remove_file(&path);
    assert!(r.is_err());
}
