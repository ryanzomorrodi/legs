use crate::{
    col_layout::build_column_layout,
    format::transpose_cols,
    index_col::{get_index_labels, render_index_column},
    schema::RSchema,
};
use extendr_api::prelude::*;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::Line,
    widgets::{Paragraph, Row, Table, TableState},
    Frame,
};
use std::ops::Range;

pub const HEADER_HEIGHT: usize = 2;
pub const DEFAULT_TRUNCATION: usize = 40;

pub struct Viewer {
    pub data: Robj,
    pub schema: RSchema,
    pub col_start_idx: usize,
    pub col_end_idx: usize,
    pub col_start_from_start: bool,
    pub truncate: bool,
    pub truncate_size: usize,
    pub state: TableState,
    pub visible_n_row: usize,
    pub visible_n_col: usize,
}

impl Viewer {
    pub fn new(x: Robj) -> extendr_api::Result<Self> {
        let schema = RSchema::build(&x)?;
        let initial_cell = if schema.nrow > 0 && !schema.is_empty() {
            Some((0, 0))
        } else {
            None
        };
        Ok(Self {
            data: x,
            schema,
            col_start_idx: 0,
            col_end_idx: 0,
            col_start_from_start: true,
            truncate: true,
            truncate_size: DEFAULT_TRUNCATION,
            state: TableState::default().with_selected_cell(initial_cell),
            visible_n_row: 0,
            visible_n_col: 0,
        })
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
        let outer = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(0)])
            .split(frame.area());
        let (summary_area, body_area) = (outer[0], outer[1]);

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
        if let Some((row_position, col_position)) = self.state.selected_cell() {
            let new_col_position = if col_position < self.col_start_idx {
                self.col_start_idx
            } else if col_position > self.col_end_idx {
                self.col_end_idx
            } else {
                col_position
            };
            self.state
                .select_cell(Some((row_position, new_col_position)));
        }

        self.visible_n_row = (table_area.height as usize).saturating_sub(HEADER_HEIGHT);
        self.visible_n_col = layout.headers.len();

        let header_row = Row::new(layout.headers).height(HEADER_HEIGHT as u16);
        let widths: Vec<Constraint> = layout
            .widths
            .iter()
            .map(|&w| Constraint::Length(w as u16))
            .collect();
        let rows = transpose_cols(layout.values)?;

        let table = Table::new(rows, widths)
            .header(header_row)
            .cell_highlight_style(Style::default().bg(Color::Yellow).fg(Color::Black));

        let mut relative_state = self.relative_state(&row_window);

        render_summary_header(&self.data, frame, summary_area)?;
        render_index_column(frame, index_area, &index_labels);
        frame.render_stateful_widget(table, table_area, &mut relative_state);

        Ok(())
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

fn render_summary_header(data: &Robj, frame: &mut Frame, area: Rect) -> extendr_api::Result<()> {
    let obj_sum_fn = R!("pillar::obj_sum")?;
    let args = pairlist!(x = data);
    let obj_summary = obj_sum_fn.call(args)?.as_str().unwrap_or("").to_string();
    let text = format!("# a {}", obj_summary);
    let paragraph = Paragraph::new(text).style(Style::default().fg(Color::Indexed(246)));
    frame.render_widget(paragraph, area);
    Ok(())
}
