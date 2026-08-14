pub mod banner;
pub mod machines;
pub mod monitor;
pub mod run;
pub mod mpi;
pub mod logs;

use std::io;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use chrono::Local;
use crossterm::event::{self, Event, KeyCode};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Tabs};
use ratatui::Frame;
use ratatui::Terminal;
use tokio::sync::mpsc::unbounded_channel;

use crate::app::{Action, AppState, ConnStatus, LogEntry, MachineView, Tab, UiEvent};
use crate::config::ClusterConfig;
use crate::runner::run_dispatcher;

const TAB_NAMES: [&str; 5] = ["Machines", "Monitor", "Run", "MPI", "Logs"];

fn tab_index(tab: &Tab) -> usize {
    match tab {
        Tab::Machines => 0,
        Tab::Monitor => 1,
        Tab::Run => 2,
        Tab::Mpi => 3,
        Tab::Logs => 4,
    }
}

fn next_tab(tab: Tab) -> Tab {
    match tab {
        Tab::Machines => Tab::Monitor,
        Tab::Monitor => Tab::Run,
        Tab::Run => Tab::Mpi,
        Tab::Mpi => Tab::Logs,
        Tab::Logs => Tab::Machines,
    }
}

pub fn run(state: &mut AppState, cfg: Arc<ClusterConfig>) -> io::Result<()> {
    let mut app = std::mem::take(state);
    app.machines = cfg
        .machines
        .iter()
        .map(|m| MachineView {
            name: m.name.clone(),
            host: m.host.clone(),
            status: ConnStatus::Disconnected,
            tags: m.tags.clone(),
        })
        .collect();
    let state = Arc::new(Mutex::new(app));

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async move {
        let (ui_tx, mut ui_rx) = unbounded_channel::<UiEvent>();
        let (action_tx, action_rx) = unbounded_channel::<Action>();

        {
            let ui_tx = ui_tx.clone();
            let cfg = cfg.clone();
            tokio::spawn(async move {
                run_dispatcher(action_rx, ui_tx, cfg).await;
            });
        }

        let key_state = state.clone();
        let key_action = action_tx.clone();
        let key_handle = thread::spawn(move || {
            loop {
                if let Ok(g) = key_state.lock() {
                    if g.quit {
                        break;
                    }
                }
                if event::poll(Duration::from_millis(100)).unwrap_or(false) {
                    if let Ok(Event::Key(k)) = event::read() {
                        match k.code {
                            KeyCode::Char('q') => {
                                let _ = key_action.send(Action::Quit);
                                if let Ok(mut g) = key_state.lock() {
                                    g.quit = true;
                                }
                                break;
                            }
                            KeyCode::Tab => {
                                if let Ok(mut g) = key_state.lock() {
                                    g.tab = next_tab(g.tab.clone());
                                }
                            }
                            KeyCode::Char('c') => {
                                let _ = key_action.send(Action::ConnectAll);
                            }
                            KeyCode::Char('r') => {
                                let targets = key_state
                                    .lock()
                                    .map(|g| g.machines.iter().map(|m| m.name.clone()).collect::<Vec<_>>())
                                    .unwrap_or_default();
                                let _ = key_action.send(Action::Run {
                                    targets,
                                    cmd: "uname -a".into(),
                                });
                            }
                            KeyCode::Char('m') => {
                                let (head, workers) = key_state
                                    .lock()
                                    .map(|g| {
                                        let ws: Vec<String> =
                                            g.machines.iter().map(|m| m.name.clone()).collect();
                                        let h = ws.first().cloned().unwrap_or_default();
                                        (h, ws)
                                    })
                                    .unwrap_or_default();
                                let _ = key_action.send(Action::LaunchMpi {
                                    head,
                                    workers,
                                    binary: "hostname".into(),
                                    args: String::new(),
                                });
                            }
                            _ => {}
                        }
                    }
                }
            }
        });

        let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
        loop {
            while let Ok(ev) = ui_rx.try_recv() {
                if let Ok(mut g) = state.lock() {
                    apply_event(&mut g, ev);
                }
            }
            {
                let g = state.lock().unwrap();
                terminal.draw(|f| ui(&g, f))?;
            }
            if state.lock().unwrap().quit {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        key_handle.join().ok();
        Ok::<(), io::Error>(())
    })
}

pub fn apply_event(state: &mut AppState, ev: UiEvent) {
    match ev {
        UiEvent::Conn { name, status } => {
            if let Some(m) = state.machines.iter_mut().find(|m| m.name == name) {
                m.status = status;
            }
        }
        UiEvent::Stats { name, stats } => {
            state.stats.insert(name, stats);
        }
        UiEvent::Log(msg) => {
            state.logs.push(LogEntry {
                ts: Local::now().format("%H:%M:%S").to_string(),
                msg,
            });
            if state.logs.len() > 1000 {
                state.logs.drain(0..state.logs.len() - 1000);
            }
        }
        UiEvent::RunOutput { name, out } => {
            state
                .run_output
                .push_str(&format!("== {name} ==\n{out}\n"));
            if state.run_output.chars().count() > 5000 {
                state.run_output = state
                    .run_output
                    .chars()
                    .skip(state.run_output.chars().count() - 5000)
                    .collect();
            }
        }
        UiEvent::MpiOutput(out) => {
            state.mpi_output.push_str(&format!("{out}\n"));
            if state.mpi_output.chars().count() > 5000 {
                state.mpi_output = state
                    .mpi_output
                    .chars()
                    .skip(state.mpi_output.chars().count() - 5000)
                    .collect();
            }
        }
    }
}

pub fn ui(state: &AppState, f: &mut Frame) {
    let size: Rect = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(size);

    let tabs = Tabs::new(TAB_NAMES.iter().map(|s| Line::from(*s)).collect::<Vec<_>>())
        .select(tab_index(&state.tab))
        .block(Block::default().borders(Borders::ALL).title("hive"));
    f.render_widget(tabs, chunks[0]);

    match state.tab {
        Tab::Machines => machines::render(state, f, chunks[1]),
        Tab::Monitor => monitor::render(state, f, chunks[1]),
        Tab::Run => run::render(state, f, chunks[1]),
        Tab::Mpi => mpi::render(state, f, chunks[1]),
        Tab::Logs => logs::render(state, f, chunks[1]),
    }
}
