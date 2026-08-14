pub mod machines;
pub mod monitor;
pub mod run;
pub mod mpi;
pub mod logs;
pub mod banner;

use std::collections::HashMap;
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
    Action, AddDraft, AppState, ConnStatus, InputTarget, LogEntry, MachineView, Tab, TargetScope,
    UiEvent,
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

fn prev_tab(tab: Tab) -> Tab {
    match tab {
        Tab::Machines => Tab::Logs,
        Tab::Monitor => Tab::Machines,
        Tab::Run => Tab::Monitor,
        Tab::Mpi => Tab::Run,
        Tab::Logs => Tab::Mpi,
    }
}

/// Brief launch splash showing the HIVE banner. Auto-dismisses after ~1.6s or
/// as soon as any key is pressed.
fn show_splash(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    use std::time::Instant;
    let deadline = Instant::now() + Duration::from_millis(1600);
    loop {
        terminal.draw(|f| {
            let constraints: [Constraint; 3] = [
                Constraint::Min(0),
                Constraint::Length(9),
                Constraint::Min(0),
            ];
            let chunks = Layout::vertical(constraints).split(f.area());
            banner::render(f, chunks[1]);
        })?;
        if Instant::now() >= deadline {
            break;
        }
        if event::poll(Duration::from_millis(120)).unwrap_or(false) {
            if let Ok(Event::Key(_)) = event::read() {
                break;
            }
        }
    }
    Ok(())
}

/// Prompt text + whether the input should be masked, for the current input target.
fn input_prompt(state: &AppState) -> (String, bool) {
    match &state.input_target {
        InputTarget::RunCommand => {
            (format!("Run on {}: ", scope_label(state)), false)
        }
        InputTarget::MpiCommand => {
            (format!("MPI on {} — binary + args (e.g. ./app -n 4): ", scope_label(state)), false)
        }
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
        InputTarget::PasswordPrompt(_) => ("Password (not stored on disk):".into(), true),
    }
}

fn help_text(state: &AppState) -> String {
    if state.editing {
        return "".into();
    }
    if state.confirm_wipe {
        return "WIPE ALL? press 'y' to erase every machine · 'n' or Esc to cancel".into();
    }
    match state.tab {
        Tab::Machines => {
            "a add · d delete · D wipe all · Enter connect · ↑/↓ or j/k select · c connect all · s save · Tab/←→/h l switch · q quit".into()
        }
        Tab::Monitor => "Tab/←→/h l switch · q quit".into(),
        Tab::Run => "Enter type · t target (all/selected) · r quick uname -a · Tab/←→/h l switch · q quit".into(),
        Tab::Mpi => "Enter type · t target (all/selected) · m sample · Tab/←→/h l switch · q quit".into(),
        Tab::Logs => "Tab/←→/h l switch · q quit".into(),
    }
}

pub fn run(
    state: &mut AppState,
    cfg: Arc<Mutex<ClusterConfig>>,
    secrets: Arc<Mutex<HashMap<String, String>>>,
) -> io::Result<()> {
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
            auth_method: match &m.auth {
                Auth::Password => "password".into(),
                Auth::Key { .. } => "key".into(),
            },
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
            let secrets = secrets.clone();
            tokio::spawn(async move {
                run_dispatcher(action_rx, ui_tx, cfg, secrets).await;
            });
        }

        let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
        // Splash: show the banner briefly (or until a key is pressed).
        show_splash(&mut terminal)?;

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

/// Targets for a Run/MPI command given the visible scope toggle: the whole
/// fleet, or just the currently selected machine.
fn scope_targets(state: &AppState) -> Vec<String> {
    match state.target_scope {
        TargetScope::All => all_names(state),
        TargetScope::Selected => state
            .machines
            .get(state.selected)
            .map(|m| vec![m.name.clone()])
            .unwrap_or_default(),
    }
}

fn scope_label(state: &AppState) -> String {
    match state.target_scope {
        TargetScope::All => "all nodes".into(),
        TargetScope::Selected => state
            .machines
            .get(state.selected)
            .map(|m| m.name.clone())
            .unwrap_or_else(|| "selected (none)".into()),
    }
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
        KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => {
            g.tab = next_tab(g.tab.clone());
        }
        KeyCode::Left | KeyCode::Char('h') => {
            g.tab = prev_tab(g.tab.clone());
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
        KeyCode::Up | KeyCode::Char('k') if matches!(g.tab, Tab::Machines) => {
            g.selected = g.selected.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j') if matches!(g.tab, Tab::Machines) => {
            let max = g.machines.len().saturating_sub(1);
            g.selected = (g.selected + 1).min(max);
        }
        KeyCode::Enter if matches!(g.tab, Tab::Machines) => {
            if let Some(m) = g.machines.get(g.selected).cloned() {
                // Password auth with no in-memory secret → prompt for it.
                if m.auth_method == "password" && !g.secrets.contains_key(&m.name) {
                    g.editing = true;
                    g.input_target = InputTarget::PasswordPrompt(m.name);
                    g.input.clear();
                    g.secret = true;
                    g.error = None;
                } else {
                    let _ = action.send(Action::Connect(m.name));
                }
            }
        }
        // --- Nuclear wipe flow (confirm then y / cancel with n or Esc) ---
        KeyCode::Char('y') if g.confirm_wipe => {
            g.confirm_wipe = false;
            drop(g);
            let _ = action.send(Action::ClearAll);
        }
        KeyCode::Char('n') if g.confirm_wipe => {
            g.confirm_wipe = false;
        }
        KeyCode::Esc if g.confirm_wipe => {
            g.confirm_wipe = false;
        }
        KeyCode::Char('D')
            if matches!(g.tab, Tab::Machines) && !g.confirm_wipe && !g.machines.is_empty() =>
        {
            g.confirm_wipe = true;
        }
        KeyCode::Char('d') if matches!(g.tab, Tab::Machines) && !g.confirm_wipe => {
            if let Some(m) = g.machines.get(g.selected).cloned() {
                let idx = g.selected;
                g.machines.remove(idx);
                if g.selected >= g.machines.len() && g.selected > 0 {
                    g.selected -= 1;
                }
                drop(g);
                let _ = action.send(Action::RemoveMachine(m.name));
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
        KeyCode::Char('t')
            if matches!(g.tab, Tab::Run) || matches!(g.tab, Tab::Mpi) =>
        {
            // Toggle command scope: whole fleet <-> selected machine.
            g.target_scope = match g.target_scope {
                TargetScope::All => TargetScope::Selected,
                TargetScope::Selected => TargetScope::All,
            };
        }
        KeyCode::Char('r') => {
            if matches!(g.tab, Tab::Run) {
                let targets = scope_targets(&g);
                drop(g);
                let _ = action.send(Action::Run {
                    targets,
                    cmd: "uname -a".into(),
                });
            }
        }
        KeyCode::Char('m') => {
            if matches!(g.tab, Tab::Mpi) {
                let targets = scope_targets(&g);
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

/// Finalizes the add-machine wizard: builds the config, persists it (without
/// the password), seeds the in-memory secret store, and connects.
fn finalize_add(state: &Arc<Mutex<AppState>>, cfg: &Arc<Mutex<ClusterConfig>>, action: &tokio::sync::mpsc::UnboundedSender<Action>) {
    let (mc, name, is_password, secret) = {
        let mut g = state.lock().unwrap();
        let d = match g.add.take() {
            Some(d) => d,
            None => return,
        };
        let (auth, method, is_password, secret) = if d.method == "k" {
            (
                Auth::Key {
                    key_path: d.secret.trim().to_string(),
                },
                "key".to_string(),
                false,
                String::new(),
            )
        } else {
            (Auth::Password, "password".to_string(), true, d.secret.clone())
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
            auth_method: method,
        });
        g.editing = false;
        g.input_target = InputTarget::None;
        g.input.clear();
        g.secret = false;
        (mc, name, is_password, secret)
    };
    // Persist into the shared config (password is NOT written) and connect.
    cfg.lock().unwrap().machines.push(mc);
    if is_password {
        // Keep the password in memory only; never touch disk.
        state.lock().unwrap().secrets.insert(name.clone(), secret.clone());
        let _ = action.send(Action::SeedSecret {
            name: name.clone(),
            password: secret,
        });
    }
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
        KeyCode::Enter => {
            let target = g.input_target.clone();
            match target {
            InputTarget::RunCommand => {
                let cmd = std::mem::take(&mut g.input);
                let targets = scope_targets(&g);
                g.editing = false;
                g.input_target = InputTarget::None;
                g.secret = false;
                drop(g);
                let _ = action.send(Action::Run { targets, cmd });
            }
            InputTarget::MpiCommand => {
                let raw = std::mem::take(&mut g.input);
                let targets = scope_targets(&g);
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
                let step = g.add.as_ref().map(|d| d.step).unwrap_or(0);
                // Validate: required text fields must not be empty.
                let invalid = match step {
                    0 if value.trim().is_empty() => Some("name can't be empty"),
                    1 if value.trim().is_empty() => Some("host can't be empty"),
                    2 if value.trim().is_empty() => Some("ssh user can't be empty"),
                    3 if !matches!(value.to_lowercase().as_str(), "p" | "k") => {
                        Some("type 'p' (password) or 'k' (ssh key)")
                    }
                    4 => {
                        let method = g.add.as_ref().map(|d| d.method.clone()).unwrap_or_else(|| "p".into());
                        if method == "k" && value.trim().is_empty() {
                            Some("ssh key path can't be empty")
                        } else if method != "k" && value.trim().is_empty() {
                            Some("password can't be empty")
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                if let Some(msg) = invalid {
                    g.error = Some(msg.into());
                    return;
                }
                g.error = None;
                let method_for_secret = {
                    let d = g.add.get_or_insert_with(AddDraft::default);
                    match step {
                        0 => d.name = value.clone(),
                        1 => d.host = value.clone(),
                        2 => d.user = value.clone(),
                        3 => {
                            d.method = value.chars().next().unwrap_or('p').to_string().to_lowercase();
                        }
                        _ => {}
                    }
                    d.method != "k"
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
            InputTarget::PasswordPrompt(name) => {
                let value = std::mem::take(&mut g.input);
                if value.trim().is_empty() {
                    g.error = Some("password can't be empty".into());
                    return;
                }
                g.error = None;
                g.secrets.insert(name.clone(), value.clone());
                g.editing = false;
                g.input_target = InputTarget::None;
                g.secret = false;
                drop(g);
                let _ = action.send(Action::SeedSecret {
                    name: name.clone(),
                    password: value,
                });
                let _ = action.send(Action::Connect(name));
            }
            InputTarget::None => {
                g.editing = false;
            }
        }
    },
        KeyCode::Char(c) => {
            g.error = None;
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
        UiEvent::Removed { name } => {
            state.machines.retain(|m| m.name != name);
            if state.selected >= state.machines.len() && state.selected > 0 {
                state.selected -= 1;
            }
        }
        UiEvent::Cleared => {
            state.machines.clear();
            state.selected = 0;
            state.confirm_wipe = false;
            state.error = None;
        }
    }
}

pub fn ui(state: &AppState, f: &mut Frame) {
    let size: Rect = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0), Constraint::Length(3)])
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

    // Bottom bar: input prompt (+ validation error) when editing, otherwise help.
    if state.editing {
        let (prompt, secret) = input_prompt(state);
        let shown: String = if secret {
            "*".repeat(state.input.chars().count())
        } else {
            state.input.clone()
        };
        let mut lines = vec![Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Yellow)),
            Span::raw(format!("{prompt} ")),
            Span::styled(shown, Style::default().fg(Color::White)),
        ])];
        if let Some(err) = &state.error {
            lines.push(
                Line::from(vec![
                    Span::styled("! ", Style::default().fg(Color::Red)),
                    Span::styled(err.clone(), Style::default().fg(Color::Red)),
                ]),
            );
        }
        let bar = Paragraph::new(lines).block(
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

    #[test]
    fn empty_wizard_field_is_rejected() {
        let state = Arc::new(Mutex::new(AppState::default()));
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel::<Action>();
        handle_global(
            &state,
            &tx,
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
        );
        // Press Enter with empty input on the name step → must not advance,
        // and an error should be surfaced.
        handle_edit(
            &state,
            &Arc::new(Mutex::new(ClusterConfig::default())),
            &tx,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        );
        let g = state.lock().unwrap();
        assert_eq!(g.add.as_ref().unwrap().step, 0, "should stay on name step");
        assert!(g.error.is_some(), "empty field should produce an error");
    }

    #[test]
    fn run_command_targets_selected_when_scope_is_selected() {
        let state = Arc::new(Mutex::new(AppState::default()));
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Action>();
        {
            let mut g = state.lock().unwrap();
            g.tab = Tab::Run;
            g.machines = vec![
                MachineView {
                    name: "n1".into(),
                    host: "h".into(),
                    status: ConnStatus::Disconnected,
                    tags: vec![],
                    auth_method: "key".into(),
                },
                MachineView {
                    name: "n2".into(),
                    host: "h".into(),
                    status: ConnStatus::Disconnected,
                    tags: vec![],
                    auth_method: "key".into(),
                },
            ];
            g.selected = 1;
            g.target_scope = TargetScope::Selected;
        }
        // Open the run input, type a command, submit.
        handle_global(
            &state,
            &tx,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        );
        for ch in ['e', 'c', 'h', 'o'] {
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
        match rx.try_recv().unwrap() {
            Action::Run { targets, cmd } => {
                assert_eq!(targets, vec!["n2".to_string()], "should target selected only");
                assert_eq!(cmd, "echo");
            }
            other => panic!("unexpected action: {other:?}"),
        }
    }
}
