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
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Tabs};
use ratatui::Frame;
use ratatui::Terminal;
use tokio::sync::mpsc::unbounded_channel;

use crate::app::{
    Action, AddDraft, AppState, ConnStatus, InputTarget, LogEntry, MachineView, Tab, UiEvent,
};
use crate::config::{Auth, ClusterConfig, MachineConfig};
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

/// Prompt text + whether the input should be masked, for the current input target.
fn input_prompt(state: &AppState) -> (String, bool) {
    match &state.input_target {
        InputTarget::RunCommand => ("Run command on all nodes:".into(), false),
        InputTarget::MpiCommand => ("MPI job — binary + args (e.g. ./app -n 4):".into(), false),
        InputTarget::AddField => {
            let d = state.add.as_ref().unwrap();
            match d.step {
                0 => ("New machine — name:".into(), false),
                1 => ("New machine — host (ip or hostname):".into(), false),
                2 => ("New machine — ssh user:".into(), false),
                3 => ("Auth — type 'p' (password) or 'k' (ssh key):".into(), false),
                4 => {
                    if d.method == "k" {
                        ("SSH key path:".into(), false)
                    } else {
                        ("Password:".into(), true)
                    }
                }
                _ => ("".into(), false),
            }
        }
        InputTarget::None => ("".into(), false),
    }
}

fn help_text(state: &AppState) -> String {
    if state.editing {
        return "".into();
    }
    match state.tab {
        Tab::Machines => "a add · c connect all · Enter connect · ↑/↓ select · s save · Tab switch · q quit".into(),
        Tab::Monitor => "Tab switch · q quit".into(),
        Tab::Run => "Enter type a command · Tab switch · q quit".into(),
        Tab::Mpi => "Enter type a job · Tab switch · q quit".into(),
        Tab::Logs => "Tab switch · q quit".into(),
    }
}

pub fn run(state: &mut AppState, cfg: Arc<Mutex<ClusterConfig>>) -> io::Result<()> {
    let mut app = std::mem::take(state);
    app.machines = cfg
        .lock()
        .unwrap()
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
        let key_cfg = cfg.clone();
        let key_handle = thread::spawn(move || {
            loop {
                if let Ok(g) = key_state.lock() {
                    if g.quit {
                        break;
                    }
                }
                if event::poll(Duration::from_millis(100)).unwrap_or(false) {
                    if let Ok(Event::Key(k)) = event::read() {
                        let editing = key_state.lock().map(|g| g.editing).unwrap_or(false);
                        if editing {
                            handle_edit(&key_state, &key_cfg, &key_action, k);
                        } else {
                            handle_global(&key_state, &key_action, k);
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

fn all_names(state: &AppState) -> Vec<String> {
    state.machines.iter().map(|m| m.name.clone()).collect()
}

fn handle_global(
    state: &Arc<Mutex<AppState>>,
    action: &tokio::sync::mpsc::UnboundedSender<Action>,
    k: KeyEvent,
) {
    let mut g = state.lock().unwrap();
    match k.code {
        KeyCode::Char('q') => {
            let _ = action.send(Action::Quit);
            g.quit = true;
        }
        KeyCode::Tab => {
            g.tab = next_tab(g.tab.clone());
        }
        KeyCode::Char('c') => {
            let _ = action.send(Action::ConnectAll);
        }
        KeyCode::Char('s') => {
            let _ = action.send(Action::SaveConfig);
        }
        KeyCode::Char('a') if matches!(g.tab, Tab::Machines) => {
            g.editing = true;
            g.input_target = InputTarget::AddField;
            g.add = Some(AddDraft::default());
            g.input.clear();
            g.secret = false;
        }
        KeyCode::Up if matches!(g.tab, Tab::Machines) => {
            g.selected = g.selected.saturating_sub(1);
        }
        KeyCode::Down if matches!(g.tab, Tab::Machines) => {
            let max = g.machines.len().saturating_sub(1);
            g.selected = (g.selected + 1).min(max);
        }
        KeyCode::Enter if matches!(g.tab, Tab::Machines) => {
            if let Some(m) = g.machines.get(g.selected).cloned() {
                let _ = action.send(Action::Connect(m.name));
            }
        }
        KeyCode::Enter if matches!(g.tab, Tab::Run) => {
            g.editing = true;
            g.input_target = InputTarget::RunCommand;
            g.input.clear();
            g.secret = false;
        }
        KeyCode::Enter if matches!(g.tab, Tab::Mpi) => {
            g.editing = true;
            g.input_target = InputTarget::MpiCommand;
            g.input.clear();
            g.secret = false;
        }
        KeyCode::Char('r') => {
            if matches!(g.tab, Tab::Run) {
                let targets = all_names(&g);
                drop(g);
                let _ = action.send(Action::Run {
                    targets,
                    cmd: "uname -a".into(),
                });
            }
        }
        KeyCode::Char('m') => {
            if matches!(g.tab, Tab::Mpi) {
                let targets = all_names(&g);
                let head = targets.first().cloned().unwrap_or_default();
                drop(g);
                let _ = action.send(Action::LaunchMpi {
                    head,
                    workers: targets,
                    binary: "hostname".into(),
                    args: String::new(),
                });
            }
        }
        _ => {}
    }
}

/// Returns true when the wizard was finalized (machine added) so the caller can act.
fn finalize_add(state: &Arc<Mutex<AppState>>, cfg: &Arc<Mutex<ClusterConfig>>, action: &tokio::sync::mpsc::UnboundedSender<Action>) {
    let (mc, name) = {
        let mut g = state.lock().unwrap();
        let d = match g.add.take() {
            Some(d) => d,
            None => return,
        };
        let auth = if d.method == "k" {
            Auth::Key {
                key_path: d.secret.clone(),
            }
        } else {
            Auth::Password {
                password: d.secret.clone(),
            }
        };
        let mc = MachineConfig {
            name: d.name.trim().to_string(),
            host: d.host.trim().to_string(),
            port: 22,
            user: d.user.trim().to_string(),
            auth,
            tags: Vec::new(),
        };
        let name = mc.name.clone();
        g.machines.push(MachineView {
            name: mc.name.clone(),
            host: mc.host.clone(),
            status: ConnStatus::Connecting,
            tags: Vec::new(),
        });
        g.editing = false;
        g.input_target = InputTarget::None;
        g.input.clear();
        g.secret = false;
        (mc, name)
    };
    // Persist into the shared config and connect.
    cfg.lock().unwrap().machines.push(mc);
    let _ = action.send(Action::SaveConfig);
    let _ = action.send(Action::Connect(name));
}

fn handle_edit(
    state: &Arc<Mutex<AppState>>,
    cfg: &Arc<Mutex<ClusterConfig>>,
    action: &tokio::sync::mpsc::UnboundedSender<Action>,
    k: KeyEvent,
) {
    let mut g = state.lock().unwrap();
    match k.code {
        KeyCode::Esc => {
            g.editing = false;
            g.input_target = InputTarget::None;
            g.input.clear();
            g.secret = false;
            g.add = None;
        }
        KeyCode::Backspace => {
            g.input.pop();
        }
        KeyCode::Enter => match g.input_target {
            InputTarget::RunCommand => {
                let cmd = std::mem::take(&mut g.input);
                let targets = all_names(&g);
                g.editing = false;
                g.input_target = InputTarget::None;
                g.secret = false;
                drop(g);
                let _ = action.send(Action::Run { targets, cmd });
            }
            InputTarget::MpiCommand => {
                let raw = std::mem::take(&mut g.input);
                let targets = all_names(&g);
                g.editing = false;
                g.input_target = InputTarget::None;
                g.secret = false;
                let mut parts = raw.split_whitespace();
                let binary = parts.next().unwrap_or_default().to_string();
                let args = parts.collect::<Vec<_>>().join(" ");
                drop(g);
                let head = targets.first().cloned().unwrap_or_default();
                let _ = action.send(Action::LaunchMpi {
                    head,
                    workers: targets,
                    binary,
                    args,
                });
            }
            InputTarget::AddField => {
                let value = std::mem::take(&mut g.input);
                let (step, method_for_secret) = {
                    let d = g.add.get_or_insert_with(AddDraft::default);
                    let step = d.step;
                    match step {
                        0 => d.name = value.clone(),
                        1 => d.host = value.clone(),
                        2 => d.user = value.clone(),
                        3 => {
                            d.method = value.chars().next().unwrap_or('p').to_string().to_lowercase();
                        }
                        _ => {}
                    }
                    (step, d.method != "k")
                };
                if step == 4 {
                    if let Some(d) = g.add.as_mut() {
                        d.secret = value;
                    }
                    drop(g);
                    finalize_add(state, cfg, action);
                    return;
                }
                if step == 3 {
                    g.secret = method_for_secret;
                } else {
                    g.secret = false;
                }
                if let Some(d) = g.add.as_mut() {
                    d.step += 1;
                }
            }
            InputTarget::None => {
                g.editing = false;
            }
        },
        KeyCode::Char(c) => {
            g.input.push(c);
        }
        _ => {}
    }
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
        .constraints([Constraint::Length(3), Constraint::Min(0), Constraint::Length(2)])
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

    // Bottom bar: input prompt when editing, otherwise contextual help.
    if state.editing {
        let (prompt, secret) = input_prompt(state);
        let shown: String = if secret {
            "*".repeat(state.input.chars().count())
        } else {
            state.input.clone()
        };
        let line = Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Yellow)),
            Span::raw(format!("{prompt} ")),
            Span::styled(shown, Style::default().fg(Color::White)),
        ]);
        let bar = Paragraph::new(line).block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(Color::Yellow)),
        );
        f.render_widget(bar, chunks[2]);
    } else {
        let bar = Paragraph::new(help_text(state))
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::TOP));
        f.render_widget(bar, chunks[2]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEvent, KeyModifiers};

    #[test]
    fn pressing_a_opens_add_machine_wizard() {
        let state = Arc::new(Mutex::new(AppState::default()));
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel::<Action>();
        // Default tab is Machines, so 'a' must start the wizard.
        assert!(matches!(state.lock().unwrap().tab, Tab::Machines));
        handle_global(
            &state,
            &tx,
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
        );
        let g = state.lock().unwrap();
        assert!(g.editing, "'a' should enter editing mode");
        assert_eq!(g.input_target, InputTarget::AddField);
        assert!(g.add.is_some());
    }

    #[test]
    fn typing_in_wizard_builds_machine_name() {
        let state = Arc::new(Mutex::new(AppState::default()));
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel::<Action>();
        handle_global(
            &state,
            &tx,
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
        );
        // type "node1" then Enter advances to step 1 (host)
        for ch in ['n', 'o', 'd', 'e', '1'] {
            handle_edit(
                &state,
                &Arc::new(Mutex::new(ClusterConfig::default())),
                &tx,
                KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE),
            );
        }
        handle_edit(
            &state,
            &Arc::new(Mutex::new(ClusterConfig::default())),
            &tx,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        );
        let g = state.lock().unwrap();
        assert_eq!(g.add.as_ref().unwrap().name, "node1");
        assert_eq!(g.add.as_ref().unwrap().step, 1);
    }

    #[test]
    fn add_wizard_prompt_renders_on_screen() {
        use ratatui::backend::TestBackend;
        let state = AppState {
            editing: true,
            input_target: InputTarget::AddField,
            add: Some(AddDraft::default()),
            ..Default::default()
        };
        let backend = TestBackend::new(80, 24);
        let mut term = ratatui::Terminal::new(backend).unwrap();
        term.draw(|f| ui(&state, f)).unwrap();
        let content: String = term
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(
            content.contains("New machine"),
            "add-machine prompt was not rendered: {content}"
        );
    }
}
