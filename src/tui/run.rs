use crate::app::AppState;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

pub fn render(state: &AppState, f: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(0)])
        .split(area);

    let header = Paragraph::new("Target: all connected. Key: r = run `uname -a` on all (sample).")
        .block(Block::default().borders(Borders::ALL).title("Run"));
    let body = Paragraph::new(state.run_output.clone())
        .block(Block::default().borders(Borders::ALL).title("Run Output"));
    f.render_widget(header, chunks[0]);
    f.render_widget(body, chunks[1]);
}
