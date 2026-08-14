use crate::config::ClusterConfig;
use crate::metrics::MachineStats;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum ConnStatus {
    Disconnected,
    Connecting,
    Connected,
    Error(String),
}

#[derive(Debug, Clone)]
pub struct MachineView {
    pub name: String,
    pub host: String,
    pub status: ConnStatus,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum Tab {
    Machines,
    Monitor,
    Run,
    Mpi,
    Logs,
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub ts: String,
    pub msg: String,
}

/// Messages flowing INTO the TUI from background tasks.
#[derive(Debug, Clone)]
pub enum UiEvent {
    Conn { name: String, status: ConnStatus },
    Stats { name: String, stats: MachineStats },
    Log(String),
    RunOutput { name: String, out: String },
    MpiOutput(String),
}

/// Actions flowing OUT OF the TUI to the dispatcher task.
#[derive(Debug, Clone)]
pub enum Action {
    Connect(String),
    ConnectAll,
    Disconnect(String),
    SaveConfig(ClusterConfig),
    Run {
        targets: Vec<String>,
        cmd: String,
    },
    LaunchMpi {
        head: String,
        workers: Vec<String>,
        binary: String,
        args: String,
    },
    Quit,
}

pub struct AppState {
    pub tab: Tab,
    pub machines: Vec<MachineView>,
    pub stats: HashMap<String, MachineStats>,
    pub logs: Vec<LogEntry>,
    pub run_output: String,
    pub mpi_output: String,
    pub status: String,
    pub quit: bool,
}

impl Default for AppState {
    fn default() -> Self {
        AppState {
            tab: Tab::Machines,
            machines: Vec::new(),
            stats: HashMap::new(),
            logs: Vec::new(),
            run_output: String::new(),
            mpi_output: String::new(),
            status: String::new(),
            quit: false,
        }
    }
}
