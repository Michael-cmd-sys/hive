use std::sync::Arc;

use russh::client::{self, Handle};
use russh::keys::{load_secret_key, PrivateKeyWithHashAlg};
use russh::ChannelMsg;
use russh::Error as RusshError;

use crate::config::{Auth, MachineConfig};
use crate::error::{HiveError, Result};

/// russh `Handler` implementation. Accepts any host key so the lab
/// environment can connect to unknown machines.
pub struct Client;

impl russh::client::Handler for Client {
    type Error = RusshError;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::ssh_key::PublicKey,
    ) -> std::result::Result<bool, Self::Error> {
        Ok(true)
    }
}

/// Output of a remote command.
pub struct CmdOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit: u32,
}

/// An authenticated SSH session to a single machine.
pub struct Session {
    handle: Handle<Client>,
    pub name: String,
}

impl Session {
    /// Connect to `cfg`'s machine, authenticating with a password or a key.
    pub async fn connect(cfg: &MachineConfig) -> Result<Self> {
        let config = Arc::new(russh::client::Config::default());
        let mut handle = client::connect(config, (cfg.host.as_str(), cfg.port), Client)
            .await
            .map_err(HiveError::Ssh)?;

        let auth_result = match &cfg.auth {
            Auth::Password { password } => {
                handle
                    .authenticate_password(&cfg.user, password.clone())
                    .await
                    .map_err(HiveError::Ssh)?
            }
            Auth::Key { key_path } => {
                let expanded = match shellexpand::full(key_path) {
                    Ok(path) => path.to_string(),
                    Err(_) => key_path.to_string(),
                };
                let key = load_secret_key(expanded, None)
                    .map_err(|e| HiveError::Ssh(e.into()))?;
                let key = PrivateKeyWithHashAlg::new(Arc::new(key), None);
                handle
                    .authenticate_publickey(&cfg.user, key)
                    .await
                    .map_err(HiveError::Ssh)?
            }
        };

        if !auth_result.success() {
            return Err(HiveError::Auth(cfg.name.clone()));
        }

        Ok(Session {
            handle,
            name: cfg.name.clone(),
        })
    }

    /// Run `cmd` on the remote host and capture its output.
    pub async fn exec(&mut self, cmd: &str) -> Result<CmdOutput> {
        let mut chan = self.handle.channel_open_session().await.map_err(HiveError::Ssh)?;
        chan.exec(true, cmd.to_string()).await.map_err(HiveError::Ssh)?;

        let mut stdout = String::new();
        let mut stderr = String::new();
        let mut exit: u32 = 0;

        while let Some(msg) = chan.wait().await {
            match msg {
                ChannelMsg::Data { data } => {
                    stdout.push_str(&String::from_utf8_lossy(&data));
                }
                ChannelMsg::ExtendedData { data, ext: 1 } => {
                    stderr.push_str(&String::from_utf8_lossy(&data));
                }
                ChannelMsg::ExitStatus { exit_status } => {
                    exit = exit_status;
                }
                _ => {}
            }
        }

        Ok(CmdOutput {
            stdout,
            stderr,
            exit,
        })
    }
}
