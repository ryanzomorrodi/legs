use crate::data::{transpose_cols, RSchema};
use col_layout::build_column_layout;
use columns::ColPicker;
use extendr_api::prelude::*;
use index_col::{get_index_labels, render_index_column};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Text},
    widgets::{Paragraph, Row, Table, TableState},
    Frame,
};
use search::{Mode, Search};
use std::ops::Range;

mod col_layout;
mod columns;
mod index_col;
mod movement;
mod search;
mod yank;

pub use search::Target;

pub const HEADER_HEIGHT: usize = 2;
pub const DEFAULT_TRUNCATION: usize = 40;

pub struct View {
    pub data: Robj,
    pub path_prefix: String,
    pub schema: RSchema,
    pub col_start_idx: usize,
    pub col_end_idx: usize,
    pub col_start_from_start: bool,
    pub truncate: bool,
    pub truncate_size: usize,
    pub state: TableState,
    pub visible_n_row: usize,
    pub visible_n_col: usize,
    pub search: Search,
    pub picker: Option<ColPicker>,
}

impl View {
    pub fn new(x: Robj, path_prefix: String) -> extendr_api::Result<Self> {
        let schema = RSchema::build(&x)?;
        let initial_cell = if schema.nrow > 0 && !schema.is_empty() {
            Some((0, 0))
        } else {
            None
        };

        Ok(Self {
            data: x,
            path_prefix,
            schema,
            col_start_idx: 0,
            col_end_idx: 0,
            col_start_from_start: true,
            truncate: true,
            truncate_size: DEFAULT_TRUNCATION,
            state: TableState::default().with_selected_cell(initial_cell),
            visible_n_row: 0,
            visible_n_col: 0,
            search: Search::new(),
            picker: None,
        })
    }

    pub fn path(&self) -> String {
        append_segment(&self.path_prefix, &self.selected_segment())
    }

    fn selected_segment(&self) -> String {
        if self.schema.is_empty() || self.schema.nrow == 0 {
            return String::new();
        }
        let (row, col) = self.selected_cell();
        self.schema.cell_path(&self.data, row, col)
    }

    pub fn selected_cell(&self) -> (usize, usize) {
        self.state.selected_cell().unwrap_or((0, 0))
    }

    pub fn selected_value(&self) -> Option<extendr_api::Result<Robj>> {
        if self.schema.is_empty() || self.schema.nrow == 0 {
            return None;
        }
        let (row, col) = self.selected_cell();
        Some(self.schema.cell(&self.data, row, col))
    }

    pub fn toggle_truncate(&mut self) {
        self.truncate = !self.truncate;
    }

    pub fn render(&mut self, frame: &mut Frame) -> extendr_api::Result<()> {
        let path = self.path();
        let (summary_area, body_area, search_area) = self.compute_layout_areas(frame.area());

        let row_window = self.compute_row_window(body_area.height as usize);
        *self.state.offset_mut() = row_window.start;

        let index_labels = get_index_labels(&self.data, row_window.clone());
        let max_label_len = index_labels
            .iter()
            .map(|s| Line::from(s.as_str()).width())
            .max()
            .unwrap_or(0);
        let index_col_width = max_label_len + 1;

        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(index_col_width as u16),
                Constraint::Min(0),
            ])
            .split(body_area);
        let (index_area, table_area) = (body[0], body[1]);

        let truncate = if self.truncate {
            Some(self.truncate_size)
        } else {
            None
        };

        let layout = build_column_layout(
            &self.schema,
            &self.data,
            row_window.clone(),
            self.col_start_idx..self.col_end_idx,
            self.col_start_from_start,
            table_area.width as usize,
            truncate,
        )?;

        self.col_start_idx = layout.col_range.start;
        self.col_end_idx = layout.col_range.end.saturating_sub(1);

        if let Some((row_pos, col_pos)) = self.state.selected_cell() {
            let clamped_col = col_pos.clamp(self.col_start_idx, self.col_end_idx);
            self.state.select_cell(Some((row_pos, clamped_col)));
        }

        self.visible_n_row = (table_area.height as usize).saturating_sub(HEADER_HEIGHT);
        self.visible_n_col = layout.headers.len();

        let styled_values = self.apply_cell_highlights(layout.col_range.start, layout.values);
        let headers =
            columns::highlight_headers(layout.headers, layout.col_range.start, &self.search);
        let header_row = Row::new(headers).height(HEADER_HEIGHT as u16);
        let widths: Vec<Constraint> = layout
            .widths
            .iter()
            .map(|&w| Constraint::Length(w as u16))
            .collect();

        let rows = transpose_cols(styled_values)?;

        let table = Table::new(rows, widths)
            .header(header_row)
            .cell_highlight_style(Style::default().bg(Color::Yellow).fg(Color::Black));

        let mut relative_state = self.relative_state(&row_window);

        render_summary_header(&self.data, &path, frame, summary_area)?;
        render_index_column(frame, index_area, &index_labels);
        frame.render_stateful_widget(table, table_area, &mut relative_state);

        if let Some(area) = search_area {
            self.render_search_bar(frame, area);
        }

        if let Some(p) = &mut self.picker {
            p.render(frame);
        }

        Ok(())
    }

    fn compute_layout_areas(&self, total_area: Rect) -> (Rect, Rect, Option<Rect>) {
        if self.search.active() {
            let outer = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1),
                    Constraint::Min(0),
                    Constraint::Length(1),
                ])
                .split(total_area);
            (outer[0], outer[1], Some(outer[2]))
        } else {
            let outer = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Min(0)])
                .split(total_area);
            (outer[0], outer[1], None)
        }
    }

    fn apply_cell_highlights(
        &self,
        col_start_idx: usize,
        values: Vec<Vec<Text<'static>>>,
    ) -> Vec<Vec<Text<'static>>> {
        let search_info = self
            .search
            .rows()
            .and_then(|s| s.regex.as_ref().map(|re| (s.col_idx, re)));

        let match_style = Style::default().bg(Color::Cyan).fg(Color::Black);

        values
            .into_iter()
            .enumerate()
            .map(|(rel_col_idx, col_cells)| {
                let abs_col_idx = col_start_idx + rel_col_idx;
                let active_regex = search_info.and_then(|(target_col, re)| {
                    if target_col == abs_col_idx {
                        Some(re)
                    } else {
                        None
                    }
                });

                col_cells
                    .into_iter()
                    .map(|cell_text| {
                        if let Some(re) = active_regex {
                            search::highlight_matches(
                                &cell_text.to_string(),
                                re,
                                Style::default(),
                                match_style,
                            )
                        } else {
                            cell_text
                        }
                    })
                    .collect()
            })
            .collect()
    }

    fn render_search_bar(&self, frame: &mut Frame, area: Rect) {
        let text = match &self.search.mode {
            Mode::Off => String::new(),
            Mode::Rows(s) if self.search.typing => format!("/{}_", s.query),
            Mode::Rows(s) => {
                let in_col = self
                    .schema
                    .full_name(s.col_idx)
                    .filter(|n| !n.is_empty())
                    .map_or(String::new(), |n| format!(" in {n}"));

                let info = if s.scanning() {
                    format!(
                        "{} matches, scanned {}%{}",
                        s.matches.len(),
                        s.chunks_scanned * 100 / s.total_chunks,
                        in_col
                    )
                } else {
                    format!("{} matches{}", s.matches.len(), in_col)
                };
                format!("/{} ({})", s.query, info)
            }
            Mode::Cols(s) if self.search.typing => format!("c/{}_", s.query),
            Mode::Cols(s) => format!("c/{} ({} columns)", s.query, s.matches.len()),
        };

        let paragraph = Paragraph::new(text).style(Style::default().fg(Color::Yellow));
        frame.render_widget(paragraph, area);
    }

    fn compute_row_window(&self, available_height: usize) -> Range<usize> {
        let height = available_height
            .saturating_sub(HEADER_HEIGHT)
            .min(self.schema.nrow);

        let (absolute_row, _) = self.selected_cell();
        let current_offset = self.state.offset();

        let offset = if absolute_row < current_offset {
            absolute_row
        } else if height > 0 && absolute_row > current_offset + height - 1 {
            absolute_row + 1 - height
        } else {
            current_offset
        };

        offset..(offset + height).min(self.schema.nrow)
    }

    fn relative_state(&self, row_window: &Range<usize>) -> TableState {
        let (absolute_row, absolute_col) = self.selected_cell();
        let relative_row = absolute_row.saturating_sub(row_window.start);
        let relative_col = absolute_col.saturating_sub(self.col_start_idx);
        TableState::default().with_selected_cell(Some((relative_row, relative_col)))
    }
}

fn render_summary_header(
    data: &Robj,
    path: &str,
    frame: &mut Frame,
    area: Rect,
) -> extendr_api::Result<()> {
    let obj_sum_fn = R!("pillar::obj_sum")?;
    let args = pairlist!(x = data);
    let obj_summary = obj_sum_fn.call(args)?.as_str().unwrap_or("").to_string();
    let text = if path.is_empty() {
        format!("# a {}", obj_summary)
    } else {
        format!("# a {}  |  {}", obj_summary, path)
    };
    let paragraph = Paragraph::new(text).style(Style::default().fg(Color::Indexed(246)));
    frame.render_widget(paragraph, area);
    Ok(())
}

fn append_segment(path: &str, segment: &str) -> String {
    let is_array_slice =
        segment.starts_with('[') && !segment.starts_with("[[") && !segment.contains('"');

    if is_array_slice && path.ends_with(", ]") {
        let without_bracket = &path[..path.len() - 1];
        let base = without_bracket.trim_end_matches(", ");
        let inner = &segment[1..segment.len() - 1];
        format!("{base}, {inner}]")
    } else {
        format!("{path}{segment}")
    }
}
