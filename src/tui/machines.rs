use crate::app::{AppState, ConnStatus};
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem};
use ratatui::Frame;

fn status_color(s: &ConnStatus) -> Color {
    match s {
        ConnStatus::Disconnected => Color::Gray,
        ConnStatus::Connecting => Color::Yellow,
        ConnStatus::Connected => Color::Green,
        ConnStatus::Error(_) => Color::Red,
    }
}

fn status_label(s: &ConnStatus) -> String {
    match s {
        ConnStatus::Disconnected => "disconnected".into(),
        ConnStatus::Connecting => "connecting".into(),
        ConnStatus::Connected => "connected".into(),
        ConnStatus::Error(e) => format!("error: {e}"),
    }
}

pub fn render(state: &AppState, f: &mut Frame, area: Rect) {
    let items: Vec<ListItem> = state
        .machines
        .iter()
        .map(|m| {
            let style = Style::default().fg(status_color(&m.status));
            let tags = if m.tags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", m.tags.join(", "))
            };
            let line = Line::from(vec![
                Span::styled(
                    format!("{} ({}) ", m.name, m.host),
                    Style::default().fg(Color::White),
                ),
                Span::styled(format!("[{}]", status_label(&m.status)), style),
                Span::raw(tags),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Machines (Tab to switch, c=connect all, q=quit)"),
    );
    f.render_widget(list, area);
}
