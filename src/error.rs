use thiserror::Error;

#[derive(Debug, Error)]
pub enum HiveError {
    #[error("ssh error: {0}")]
    Ssh(#[from] russh::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("config error: {0}")]
    Config(String),
    #[error("auth failed for {0}")]
    Auth(String),
    #[error("command failed on {host}: exit {code}\n{stderr}")]
    Command {
        host: String,
        code: i32,
        stderr: String,
    },
}
pub type Result<T> = std::result::Result<T, HiveError>;
