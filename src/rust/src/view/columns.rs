use super::search::{Matches, Search};
use super::View;
use nucleo_matcher::{
    pattern::{CaseMatching, Normalization, Pattern},
    Config, Matcher, Utf32Str,
};
use ratatui::{
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Clear, List, ListItem, ListState, Paragraph},
    Frame,
};
use regex::Regex;

pub struct ColSearchState {
    pub query: String,
    pub regex: Option<Regex>,
    pub matches: Matches,
    pub anchor_col: usize,
}

impl ColSearchState {
    pub fn new(anchor_col: usize) -> Self {
        Self {
            query: String::new(),
            regex: None,
            matches: Matches::default(),
            anchor_col,
        }
    }
}

pub(super) fn highlight_headers(
    headers: Vec<Text<'static>>,
    first_col: usize,
    search: &Search,
) -> Vec<Text<'static>> {
    let Some(cs) = search.cols() else {
        return headers;
    };
    let highlight_style = Style::default().bg(Color::Cyan).fg(Color::Black);

    headers
        .into_iter()
        .enumerate()
        .map(|(i, mut header)| {
            let col_idx = first_col + i;
            if cs.matches.0.binary_search(&col_idx).is_ok() {
                if let Some(line) = header.lines.first_mut() {
                    let name: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
                    line.spans = vec![Span::styled(name, highlight_style)];
                }
            }
            header
        })
        .collect()
}

/// A matched item result from fuzzy column matching.
struct Hit {
    col: usize,
    indices: Vec<usize>,
}

/// Interactive picker widget for column search and navigation.
pub struct ColPicker {
    query: String,
    labels: Vec<String>,
    hits: Vec<Hit>,
    list_state: ListState,
    matcher: Matcher,
}

impl ColPicker {
    pub fn new(labels: Vec<String>, current_col: usize) -> Self {
        let mut picker = Self {
            query: String::new(),
            labels,
            hits: Vec::new(),
            list_state: ListState::default(),
            matcher: Matcher::new(Config::DEFAULT),
        };
        picker.refilter();
        if let Some(pos) = picker.hits.iter().position(|h| h.col == current_col) {
            picker.list_state.select(Some(pos));
        }
        picker
    }

    fn refilter(&mut self) {
        let pattern = Pattern::parse(&self.query, CaseMatching::Smart, Normalization::Smart);
        let mut buf = Vec::new();
        let mut idx = Vec::new();
        let mut scored = Vec::new();

        for (col, label) in self.labels.iter().enumerate() {
            if self.query.is_empty() {
                scored.push((0, Hit { col, indices: Vec::new() }));
                continue;
            }
            idx.clear();
            let hay = Utf32Str::new(label, &mut buf);
            if let Some(score) = pattern.indices(hay, &mut self.matcher, &mut idx) {
                idx.sort_unstable();
                idx.dedup();
                scored.push((
                    score,
                    Hit {
                        col,
                        indices: idx.iter().map(|&i| i as usize).collect(),
                    },
                ));
            }
        }

        scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.col.cmp(&b.1.col)));
        self.hits = scored.into_iter().map(|(_, hit)| hit).collect();
        self.list_state.select(if self.hits.is_empty() { None } else { Some(0) });
    }

    fn step(&mut self, delta: isize) {
        let n = self.hits.len();
        if n == 0 {
            return;
        }
        let cur = self.list_state.selected().unwrap_or(0) as isize;
        self.list_state
            .select(Some((cur + delta).rem_euclid(n as isize) as usize));
    }

    fn selected_col(&self) -> Option<usize> {
        self.list_state
            .selected()
            .and_then(|i| self.hits.get(i))
            .map(|h| h.col)
    }

    pub fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();
        frame.render_widget(Clear, area);

        let [input_area, list_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(area);

        frame.render_widget(
            Paragraph::new(format!("> {}_", self.query)).style(Style::default().fg(Color::Yellow)),
            input_area,
        );

        let match_style = Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD);

        let items: Vec<ListItem> = self
            .hits
            .iter()
            .map(|hit| {
                let label = &self.labels[hit.col];
                let spans = highlight_label_spans(label, &hit.indices, match_style);
                ListItem::new(Line::from(spans))
            })
            .collect();

        let list = List::new(items)
            .highlight_symbol("> ")
            .highlight_style(Style::default().bg(Color::Yellow).fg(Color::Black));
        frame.render_stateful_widget(list, list_area, &mut self.list_state);
    }
}

/// Helper to aggregate character match indices into contiguous styled `Span` blocks.
fn highlight_label_spans(
    label: &str,
    matched_indices: &[usize],
    match_style: Style,
) -> Vec<Span<'static>> {
    if matched_indices.is_empty() {
        return vec![Span::raw(label.to_string())];
    }

    let mut spans = Vec::new();
    let mut current_segment = String::new();
    let mut is_highlighted = matched_indices.contains(&0);

    for (i, c) in label.chars().enumerate() {
        let char_matched = matched_indices.contains(&i);
        if char_matched != is_highlighted {
            if !current_segment.is_empty() {
                let style = if is_highlighted {
                    match_style
                } else {
                    Style::default()
                };
                spans.push(Span::styled(current_segment, style));
                current_segment = String::new();
            }
            is_highlighted = char_matched;
        }
        current_segment.push(c);
    }

    if !current_segment.is_empty() {
        let style = if is_highlighted {
            match_style
        } else {
            Style::default()
        };
        spans.push(Span::styled(current_segment, style));
    }

    spans
}

impl View {
    pub(super) fn refresh_col_matches(&mut self) {
        let ncols = self.schema.len();
        let Some(s) = self.search.cols_mut() else {
            return;
        };

        s.regex = if s.query.is_empty() {
            None
        } else {
            Regex::new(&s.query).ok()
        };

        s.matches.clear();
        if let Some(re) = &s.regex {
            for i in 0..ncols {
                if self.schema.full_name(i).is_some_and(|n| re.is_match(n)) {
                    s.matches.0.push(i);
                }
            }
        }

        let idx = s.matches.0.partition_point(|&c| c < s.anchor_col);
        let target = s
            .matches
            .0
            .get(idx)
            .or_else(|| s.matches.0.first())
            .copied();

        if let Some(col) = target {
            self.move_cursor_to_col(col);
        }
    }

    pub(super) fn move_cursor_to_col(&mut self, col: usize) {
        let (row, _) = self.selected_cell();
        self.state.select_cell(Some((row, col)));

        if col > self.col_end_idx {
            self.col_end_idx = col;
            self.col_start_from_start = false;
        } else if col < self.col_start_idx {
            self.col_start_idx = col;
            self.col_start_from_start = true;
        }
    }

    pub fn open_col_picker(&mut self) {
        if self.schema.is_empty() {
            return;
        }
        let labels = (0..self.schema.len())
            .map(|i| match self.schema.full_name(i) {
                Some(n) if !n.is_empty() => n.to_string(),
                _ => format!("#{}", i + 1),
            })
            .collect();
        let (_, col) = self.selected_cell();
        self.picker = Some(ColPicker::new(labels, col));
    }

    pub fn handle_picker_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let Some(p) = self.picker.as_mut() else {
            return;
        };

        match key.code {
            KeyCode::Esc => self.picker = None,
            KeyCode::Enter => {
                let col = p.selected_col();
                self.picker = None;
                if let Some(col) = col {
                    self.move_cursor_to_col(col);
                }
            }
            KeyCode::Char('n') if ctrl => p.step(1),
            KeyCode::Char('p') if ctrl => p.step(-1),
            KeyCode::Down => p.step(1),
            KeyCode::Up => p.step(-1),
            KeyCode::Char('u') if ctrl => {
                p.query.clear();
                p.refilter();
            }
            KeyCode::Backspace => {
                p.query.pop();
                p.refilter();
            }
            KeyCode::Char(c) if !ctrl => {
                p.query.push(c);
                p.refilter();
            }
            _ => {}
        }
    }
}
