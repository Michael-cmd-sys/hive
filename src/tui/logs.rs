use crate::app::AppState;
use crate::tui::banner;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

pub fn render(state: &AppState, f: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(9), Constraint::Min(0)])
        .split(area);

    banner::render(f, chunks[0]);

    let recent: Vec<&crate::app::LogEntry> = state.logs.iter().rev().take(200).collect();
    let lines: Vec<Line> = recent
        .iter()
        .map(|l| Line::from(format!("[{}] {}", l.ts, l.msg)))
        .collect();

    let p = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Logs (newest first)"),
    );
    f.render_widget(p, chunks[1]);
}
