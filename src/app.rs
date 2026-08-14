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
    /// "password" or "key" — so the UI knows whether a secret must be supplied.
    pub auth_method: String,
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
    Conn {
        name: String,
        status: ConnStatus,
    },
    Stats {
        name: String,
        stats: MachineStats,
    },
    Log(String),
    RunOutput {
        name: String,
        out: String,
    },
    MpiOutput(String),
    /// A machine was removed (e.g. via the UI); drop it from the list.
    Removed {
        name: String,
    },
    /// All machines were wiped (nuclear option); clear the list.
    Cleared,
}

/// Actions flowing OUT OF the TUI to the dispatcher task.
#[derive(Debug, Clone)]
pub enum Action {
    Connect(String),
    ConnectAll,
    Disconnect(String),
    /// Remove a single machine from the config and disconnect it.
    RemoveMachine(String),
    /// Wipe every machine from the config (`cluster.yaml`) and disconnect all.
    ClearAll,
    /// Push an in-memory password for `name` (never persisted to disk).
    SeedSecret {
        name: String,
        password: String,
    },
    SaveConfig,
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

/// What the current text-input line is for.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum InputTarget {
    #[default]
    None,
    RunCommand,
    MpiCommand,
    AddField,
    /// Prompting for a password to connect to `String` (machine name).
    PasswordPrompt(String),
}

/// Whether a typed Run/MPI command targets the whole fleet or just the
/// selected machine. Kept visible in the UI so there is no hidden mode.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum TargetScope {
    #[default]
    All,
    Selected,
}

/// Partial fields collected while adding a machine via the TUI wizard.
#[derive(Debug, Clone, Default)]
pub struct AddDraft {
    pub step: usize,
    pub name: String,
    pub host: String,
    pub user: String,
    /// "p" = password, "k" = ssh key
    pub method: String,
    pub secret: String,
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
    // Interactive input state
    pub editing: bool,
    pub input: String,
    pub input_target: InputTarget,
    pub secret: bool,
    pub add: Option<AddDraft>,
    pub selected: usize,
    /// In-memory passwords keyed by machine name. NEVER written to disk.
    pub secrets: HashMap<String, String>,
    /// Transient validation/error message shown in the bottom bar.
    pub error: Option<String>,
    /// When true the next 'y'/'n' confirms or cancels a nuclear wipe.
    pub confirm_wipe: bool,
    /// Run/MPI command scope: the whole fleet or just the selected machine.
    pub target_scope: TargetScope,
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
            editing: false,
            input: String::new(),
            input_target: InputTarget::None,
            secret: false,
            add: None,
            selected: 0,
            secrets: HashMap::new(),
            error: None,
            confirm_wipe: false,
            target_scope: TargetScope::All,
        }
    }
}
