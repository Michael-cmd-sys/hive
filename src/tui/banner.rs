use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

const ART: &str = r#"
  _   _ ___ ___    _    __      __
 | | | | _ \ _ \  /_\   \ \    / /
 | |_| |  _/   / / _ \   \ \/\/ /
  \___/|_| |_|_\/_/ \_\   \_/\_/
"#;

pub fn render(f: &mut Frame, area: Rect) {
    let mut lines: Vec<Line> = ART
        .lines()
        .map(|l| {
            Line::from(Span::styled(
                l.to_string(),
                Style::default().fg(Color::Yellow),
            ))
        })
        .collect();

    lines.push(Line::from(vec![
        Span::styled("hive", Style::default().fg(Color::White)),
        Span::raw("  —  cluster command & control"),
    ]));

    let para = Paragraph::new(lines).alignment(Alignment::Center).block(
        Block::default()
            .borders(Borders::ALL)
            .title("welcome")
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    f.render_widget(para, area);
}
