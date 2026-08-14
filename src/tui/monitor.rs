use crate::app::{AppState, ConnStatus};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, Paragraph};
use ratatui::Frame;

fn cpu_color(pct: f32) -> Color {
    if pct >= 85.0 {
        Color::Red
    } else if pct >= 70.0 {
        Color::Yellow
    } else {
        Color::Green
    }
}

/// Map a 0-100 load to an 1/8-block glyph so each core can be drawn compactly
/// in a single line.
fn core_glyph(load: f32) -> char {
    let idx = ((load / 100.0) * 8.0).round().clamp(0.0, 8.0) as usize;
    [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'][idx]
}

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
        let inner = chunks[i];

        // Per-node layout: two gauges, then a details + process pane.
        let node_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Min(0),
            ])
            .split(inner);

        let (cpu, mem_ratio, per_core) = match stats {
            Some(s) => (
                s.cpu_percent,
                if s.mem_total_mib > 0 {
                    s.mem_used_mib as f32 / s.mem_total_mib as f32
                } else {
                    0.0
                },
                s.cpu_per_core.as_slice(),
            ),
            None => (0.0, 0.0, &[][..]),
        };

        // Per-core CPU load as a compact strip of 1/8-block glyphs, each
        // colored by its own load. The per-node average is shown as a summary.
        let mut cpu_spans = vec![Span::styled(
            "CPU/core ",
            Style::default().fg(Color::DarkGray),
        )];
        if per_core.is_empty() {
            cpu_spans.push(Span::styled(
                "(no data)",
                Style::default().fg(Color::DarkGray),
            ));
        } else {
            for (i, c) in per_core.iter().enumerate() {
                if i > 0 {
                    cpu_spans.push(Span::raw(" "));
                }
                cpu_spans.push(Span::styled(
                    core_glyph(*c).to_string(),
                    Style::default().fg(cpu_color(*c)),
                ));
            }
            cpu_spans.push(Span::styled(
                format!("  avg {:.0}%", cpu),
                Style::default().fg(cpu_color(cpu)),
            ));
        }
        f.render_widget(Paragraph::new(Line::from(cpu_spans)), node_chunks[0]);

        let mem_gauge = Gauge::default()
            .ratio((mem_ratio as f64).clamp(0.0, 1.0))
            .label(format!(
                "RAM {:.0}%  ({}/{} MiB)",
                mem_ratio * 100.0_f32,
                stats.map(|s| s.mem_used_mib).unwrap_or(0),
                stats.map(|s| s.mem_total_mib).unwrap_or(0)
            ))
            .gauge_style(Style::default().fg(Color::Cyan));
        f.render_widget(mem_gauge, node_chunks[1]);

        let mut detail: Vec<Line> = match stats {
            Some(s) => vec![
                Line::from(format!(
                    "cores {}  load {} {} {}  up {:.0}s",
                    s.cores, s.load1, s.load5, s.load15, s.uptime_secs
                )),
                Line::from(Span::styled(
                    "top processes (by CPU):",
                    Style::default().fg(Color::DarkGray),
                )),
            ],
            None => vec![Line::from("connected, awaiting stats…")],
        };
        if let Some(s) = stats {
            if s.top_procs.is_empty() {
                detail.push(Line::from("  (none)"));
            }
            for p in &s.top_procs {
                detail.push(Line::from(format!(
                    "  {:<7} {:<14} cpu {:.0}%  mem {:.0}%",
                    p.pid, p.comm, p.cpu, p.mem
                )));
            }
        }

        let p = Paragraph::new(detail).block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("{} ({})", m.name, m.host)),
        );
        f.render_widget(p, node_chunks[2]);
    }
}
