use crate::app::{AppState, ConnStatus};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};
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
        ConnStatus::Connecting => "connecting…".into(),
        ConnStatus::Connected => "connected".into(),
        ConnStatus::Error(e) => format!("error: {e}"),
    }
}

pub fn render(state: &AppState, f: &mut Frame, area: Rect) {
    if state.machines.is_empty() {
        let hint = "No machines yet.\n\n\
            • Press 'a' to add one interactively (name → host → ssh user → password/key). It connects immediately and saves to cluster.yaml.\n\
            • Or drop a cluster.yaml next to the binary and press 'c' to connect to all.\n\
            • 's' saves the current config.  Tab switches tabs.  q quits.";
        let p = ratatui::widgets::Paragraph::new(hint).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Machines (a=add, c=connect all, s=save)"),
        );
        f.render_widget(p, area);
        return;
    }

    let items: Vec<ListItem> = state
        .machines
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let sel = i == state.selected;
            let style = if sel {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let tags = if m.tags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", m.tags.join(", "))
            };
            let line = Line::from(vec![
                Span::raw(if sel { "> " } else { "  " }),
                Span::styled(
                    format!("{} ({}) ", m.name, m.host),
                    Style::default().fg(Color::White),
                ),
                Span::styled(
                    format!("[{}]", status_label(&m.status)),
                    Style::default().fg(status_color(&m.status)),
                ),
                Span::raw(tags),
            ]);
            ListItem::new(line).style(style)
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(state.selected));

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Machines (a=add, Enter=connect, c=connect all, s=save)"),
        )
        .highlight_symbol("")
        .highlight_style(Style::default());
    f.render_stateful_widget(list, area, &mut list_state);
}
