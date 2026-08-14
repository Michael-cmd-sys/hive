use crate::error::HiveError;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "lowercase")]
pub enum Auth {
    /// Password auth. The secret is deliberately NOT serialized: it is kept
    /// only in memory for the life of the process (see `Runner::secrets`) and
    /// the user is prompted for it when needed. Nothing sensitive is written
    /// to cluster.yaml.
    Password,
    Key {
        key_path: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineConfig {
    pub name: String,
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    pub user: String,
    pub auth: Auth,
    #[serde(default)]
    pub tags: Vec<String>,
}
fn default_port() -> u16 {
    22
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MpiConfig {
    #[serde(default = "default_launcher")]
    pub launcher: String,
    #[serde(default)]
    pub default_args: String,
}
fn default_launcher() -> String {
    "mpirun".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterConfig {
    #[serde(default = "default_poll")]
    pub poll_interval_ms: u64,
    #[serde(default)]
    pub mpi: MpiConfig,
    #[serde(default)]
    pub machines: Vec<MachineConfig>,
}
fn default_poll() -> u64 {
    2000
}

impl Default for MachineConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            host: String::new(),
            port: default_port(),
            user: String::new(),
            auth: Auth::Password,
            tags: Vec::new(),
        }
    }
}
impl Default for MpiConfig {
    fn default() -> Self {
        Self {
            launcher: default_launcher(),
            default_args: String::new(),
        }
    }
}
impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            poll_interval_ms: default_poll(),
            mpi: MpiConfig::default(),
            machines: Vec::new(),
        }
    }
}

impl ClusterConfig {
    pub fn load(path: &Path) -> Result<Self, HiveError> {
        if !path.exists() {
            return Ok(ClusterConfig::default());
        }
        let text = std::fs::read_to_string(path)
            .map_err(|e| HiveError::Config(format!("read {}: {e}", path.display())))?;
        let cfg: ClusterConfig = serde_yaml::from_str(&text)
            .map_err(|e| HiveError::Config(format!("parse {}: {e}", path.display())))?;
        cfg.validate()?;
        Ok(cfg)
    }

    pub fn save(&self, path: &Path) -> Result<(), HiveError> {
        let text = serde_yaml::to_string(self)
            .map_err(|e| HiveError::Config(format!("serialize: {e}")))?;
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    HiveError::Config(format!("create dir {}: {e}", parent.display()))
                })?;
            }
        }
        std::fs::write(path, text)
            .map_err(|e| HiveError::Config(format!("write {}: {e}", path.display())))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    fn validate(&self) -> Result<(), HiveError> {
        for m in &self.machines {
            if let Auth::Key { key_path } = &m.auth {
                if key_path.is_empty() {
                    return Err(HiveError::Config(format!("{}: empty key_path", m.name)));
                }
            }
        }
        Ok(())
    }
}
