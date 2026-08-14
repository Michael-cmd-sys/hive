use hive::config::{Auth, MachineConfig};
use hive::ssh::Session;

#[tokio::test]
#[ignore]
async fn connects_to_localhost_if_available() {
    if std::env::var("HIVE_TEST_SSH").is_err() {
        return;
    }
    let mc = MachineConfig {
        name: "local".into(),
        host: "127.0.0.1".into(),
        port: 22,
        user: std::env::var("USER").unwrap_or_else(|_| "user".into()),
        auth: Auth::Password,
        tags: vec![],
    };
    let mut s = Session::connect(&mc, Some(&std::env::var("HIVE_TEST_PW").unwrap_or_default()))
        .await
        .expect("connect");
    let o = s.exec("echo hi").await.expect("exec");
    assert!(o.stdout.contains("hi"));
}
