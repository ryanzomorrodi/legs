use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

const KEYBINDINGS: &[(&str, &str)] = &[
    ("hjkl (or ← ↓ ↑ →)", "Move cursor"),
    ("HJKL (or Shift + ← ↓ ↑ →)", "Page over"),
    ("g", "Select top row"),
    ("<n>G", "Select bottom row (or <n> row)"),
    ("^", "Select first column"),
    ("$", "Select last column"),
    (
        "<n>t",
        "Toggle truncation (or set truncation to <n> characters)",
    ),
    ("Enter", "View highlighted cell"),
    ("esc", "View parent data structure"),
    ("y", "Yank (copy) selected cell"),
    ("?", "View this help screen"),
    ("q", "Quit"),
];

pub fn render_help(frame: &mut Frame) {
    let area = centered_rect(90, 90, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(" Keybindings (esc to close) ")
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::White));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Split into two columns with a 1-cell gap between them.
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Length(1),
            Constraint::Percentage(50),
        ])
        .split(inner);
    let (left_area, right_area) = (columns[0], columns[2]);

    let split_at = KEYBINDINGS.len().div_ceil(2);
    let (left_bindings, right_bindings) = KEYBINDINGS.split_at(split_at);

    frame.render_widget(bindings_paragraph(left_bindings), left_area);
    frame.render_widget(bindings_paragraph(right_bindings), right_area);
}

fn bindings_paragraph(bindings: &[(&str, &str)]) -> Paragraph<'static> {
    let mut lines: Vec<Line> = Vec::new();

    for (key, desc) in bindings {
        lines.push(Line::from(Span::styled(
            key.to_string(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(Span::raw(format!("  {desc}"))));
        lines.push(Line::from(""));
    }

    Paragraph::new(lines).wrap(Wrap { trim: true })
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}
