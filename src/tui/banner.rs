use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

/// Bold, weighty block-art wordmark true to the binary's name.
pub const BANNER: &[&str] = &[
    "██      ██  ██  ██      ██  ████████  ",
    "██      ██  ██  ██      ██  ██        ",
    "██  ██  ██  ██  ██      ██  ██        ",
    "██████████  ██   ██    ██   ██████    ",
    "██  ██  ██  ██    ██  ██    ██        ",
    "██      ██  ██     ████     ████████  ",
];

const SUBTITLE: &str = "hive — ssh cluster orchestration for the swarm";

pub fn render(f: &mut Frame, area: Rect) {
    let lines: Vec<Line> = BANNER
        .iter()
        .map(|l| {
            Line::from(Span::styled(
                *l,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ))
        })
        .chain(std::iter::once(Line::from(Span::styled(
            SUBTITLE,
            Style::default().fg(Color::Gray),
        ))))
        .collect();

    let p = Paragraph::new(lines)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).title("hive"));
    f.render_widget(p, area);
}
