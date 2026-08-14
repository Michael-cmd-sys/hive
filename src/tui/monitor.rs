use crate::app::{AppState, ConnStatus};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

pub fn render(state: &AppState, f: &mut Frame, area: Rect) {
    let connected: Vec<&crate::app::MachineView> = state
        .machines
        .iter()
        .filter(|m| matches!(m.status, ConnStatus::Connected))
        .collect();

    if connected.is_empty() {
        let p = Paragraph::new("No connected machines yet — press c")
            .block(Block::default().borders(Borders::ALL).title("Monitor"));
        f.render_widget(p, area);
        return;
    }

    let n = connected.len() as u32;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Ratio(1, n); connected.len()])
        .split(area);

    for (i, m) in connected.iter().enumerate() {
        let stats = state.stats.get(&m.name);
        let (cpu, color) = match stats {
            Some(s) => {
                let c = if s.cpu_percent > 80.0 {
                    Color::Red
                } else if s.cpu_percent > 50.0 {
                    Color::Yellow
                } else {
                    Color::Green
                };
                (s.cpu_percent, c)
            }
            None => (0.0, Color::Gray),
        };
        let lines = match stats {
            Some(s) => vec![
                Line::from(vec![
                    Span::raw("CPU: "),
                    Span::styled(format!("{:.1}%", cpu), Style::default().fg(color)),
                ]),
                Line::from(format!("RAM: {} / {} MiB", s.mem_used_mib, s.mem_total_mib)),
                Line::from(format!("Cores: {}", s.cores)),
                Line::from(format!(
                    "Load: {:.2} {:.2} {:.2}",
                    s.load1, s.load5, s.load15
                )),
                Line::from(format!("Uptime: {:.0}s", s.uptime_secs)),
            ],
            None => vec![Line::from("connected, awaiting stats…")],
        };
        let p = Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("{} ({})", m.name, m.host)),
        );
        f.render_widget(p, chunks[i]);
    }
}
