use crate::r_types::{classify, RobjContainer, UnsupportedTypeError};
use ansi_to_tui::IntoText;
use extendr_api::prelude::*;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Text},
    widgets::{Cell, Paragraph, Row, Table, TableState},
    Frame,
};

pub struct Viewer {
    n_rows: usize,
    n_cols: usize,
    pub data: RobjContainer,
    state: TableState,
    col_start_idx: usize,
    last_col_visible: bool,
    truncate: bool,
}

impl Viewer {
    pub fn new(x: Robj) -> Result<Self, UnsupportedTypeError> {
        let data = classify(x)?;

        let n_rows = match data {
            RobjContainer::DataFrame { ref robj } => robj.get_attrib("row.names").unwrap().len(),
            RobjContainer::Matrix { ref robj } => {
                robj.get_attrib("dim").unwrap().as_integer_slice().unwrap()[0] as usize
            }
            RobjContainer::List { ref robj } | RobjContainer::Vector { ref robj } => robj.len(),
        };
        let n_cols = match data {
            RobjContainer::DataFrame { ref robj } => robj.as_list().unwrap().len(),
            RobjContainer::Matrix { ref robj } => {
                robj.get_attrib("dim").unwrap().as_integer_slice().unwrap()[1] as usize
            }
            RobjContainer::List { .. } | RobjContainer::Vector { .. } => 1,
        };
        let initial_cell = if n_rows > 0 && n_cols > 0 {
            Some((0, 0))
        } else {
            None
        };

        Ok(Self {
            n_rows,
            n_cols,
            data,
            state: TableState::default().with_selected_cell(initial_cell),
            col_start_idx: 0,
            last_col_visible: n_cols == 0,
            truncate: true,
        })
    }

    pub fn selected_cell(&self) -> (usize, usize) {
        self.state.selected_cell().unwrap_or((0, 0))
    }

    pub fn next_row(&mut self) {
        if self.n_rows == 0 {
            return;
        }
        let (row, col) = self.selected_cell();
        let next_row = if row >= self.n_rows - 1 { 0 } else { row + 1 };
        self.state.select_cell(Some((next_row, col)));
    }

    pub fn next_n_row(&mut self, n: usize) {
        if self.n_rows == 0 {
            return;
        }
        let (row, col) = self.selected_cell();
        let mut new_row = row + n;
        if new_row >= self.n_rows {
            new_row = self.n_rows - 1
        }
        self.state.select_cell(Some((new_row, col)));
    }

    pub fn previous_row(&mut self) {
        if self.n_rows == 0 {
            return;
        }
        let (row, col) = self.selected_cell();
        let prev_row = if row == 0 { self.n_rows - 1 } else { row - 1 };
        self.state.select_cell(Some((prev_row, col)));
    }

    pub fn previous_n_row(&mut self, n: usize) {
        if self.n_rows == 0 {
            return;
        }
        let (row, col) = self.selected_cell();
        let new_row = row.saturating_sub(n);
        self.state.select_cell(Some((new_row, col)));
    }

    pub fn go_to_row(&mut self, idx: usize) {
        if self.n_rows == 0 {
            return;
        }
        let (_, col) = self.selected_cell();
        let row = idx.min(self.n_rows - 1);
        self.state.select_cell(Some((row, col)));
    }

    pub fn first_row(&mut self) {
        if self.n_rows == 0 {
            return;
        }
        let (_, col) = self.selected_cell();
        self.state.select_cell(Some((0, col)));
    }

    pub fn last_row(&mut self) {
        if self.n_rows == 0 {
            return;
        }
        let (_, col) = self.selected_cell();
        self.state.select_cell(Some((self.n_rows - 1, col)));
    }

    pub fn next_col(&mut self) {
        if self.n_cols == 0 {
            return;
        }
        let (row, col) = self.selected_cell();
        if col < self.n_cols - 1 {
            self.state.select_cell(Some((row, col + 1)));
        }
    }

    pub fn next_n_col(&mut self, n: usize) {
        if self.n_cols == 0 {
            return;
        }
        let (row, col) = self.selected_cell();
        let mut new_col = col + n;
        if new_col >= self.n_cols {
            new_col = self.n_cols - 1
        }
        self.state.select_cell(Some((row, new_col)));
    }

    pub fn previous_col(&mut self) {
        if self.n_cols == 0 {
            return;
        }
        let (row, col) = self.selected_cell();
        if col > 0 {
            self.state.select_cell(Some((row, col - 1)));
        }
    }

    pub fn previous_n_col(&mut self, n: usize) {
        if self.n_rows == 0 {
            return;
        }
        let (row, col) = self.selected_cell();
        let new_col = col.saturating_sub(n);
        self.state.select_cell(Some((row, new_col)));
    }

    pub fn first_column(&mut self) {
        let (row, _) = self.selected_cell();
        self.state.select_cell(Some((row, 0)));
        self.col_start_idx = 0;
    }

    pub fn last_column(&mut self) {
        if self.n_cols == 0 {
            return;
        }
        let (row, _) = self.selected_cell();
        self.state.select_cell(Some((row, self.n_cols - 1)));
    }

    pub fn toggle_truncate(&mut self) {
        self.truncate = !self.truncate;
    }

    pub fn render(&mut self, frame: &mut Frame) {
        let outer = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(0)])
            .split(frame.area());
        let (summary_area, body_area) = (outer[0], outer[1]);

        let row_window = compute_row_window(self, body_area.height as usize);

        let (index_labels, has_names) = get_index_labels(&self.data, &row_window);
        let max_label_len = index_labels.iter().map(|s| s.len()).max().unwrap_or(0);
        let index_col_width = max_label_len + 1;

        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(index_col_width as u16),
                Constraint::Min(0),
            ])
            .split(body_area);
        let (index_area, table_area) = (body[0], body[1]);

        let col_data =
            scroll_col_window(self, &row_window, table_area.width as usize, self.truncate);
        let col_layout = build_col_layout(col_data);

        render_summary_header(
            &self.data,
            frame,
            summary_area,
            index_col_width,
            col_layout.type_style,
        );

        render_index_column(
            frame,
            index_area,
            &index_labels,
            has_names,
            col_layout.type_style,
        );

        let table = build_table(col_layout);

        let (absolute_row, absolute_col) = self.selected_cell();
        let relative_row = absolute_row.saturating_sub(row_window.offset);
        let relative_col = absolute_col.saturating_sub(self.col_start_idx);
        self.state.select_cell(Some((relative_row, relative_col)));
        *self.state.offset_mut() = 0;

        frame.render_stateful_widget(table, table_area, &mut self.state);

        self.state.select_cell(Some((absolute_row, absolute_col)));
        *self.state.offset_mut() = row_window.offset;
    }
}

struct RowWindow {
    offset: usize,
    end: usize,
    height: usize,
}

const HEADER_HEIGHT: usize = 2;

fn compute_row_window(viewer: &Viewer, available_height: usize) -> RowWindow {
    let height = available_height
        .saturating_sub(HEADER_HEIGHT)
        .min(viewer.n_rows);

    let (absolute_row, _) = viewer.selected_cell();
    let initial_offset = viewer.state.offset();
    let offset = if absolute_row < initial_offset {
        absolute_row
    } else if height > 0 && absolute_row > initial_offset + height - 1 {
        absolute_row + 1 - height
    } else {
        initial_offset
    };
    let end = (offset + height).min(viewer.n_rows);

    RowWindow {
        offset,
        end,
        height,
    }
}

fn matrix_dimnames(robj: &Robj) -> (Option<Vec<String>>, Option<Vec<String>>) {
    let Some(dn) = robj.get_attrib("dimnames") else {
        return (None, None);
    };
    let list = dn.as_list().unwrap();
    let extract = |r: Robj| -> Option<Vec<String>> {
        if r.is_null() {
            None
        } else {
            r.as_str_vector()
                .map(|v| v.into_iter().map(String::from).collect())
        }
    };
    (extract(list.elt(0).unwrap()), extract(list.elt(1).unwrap()))
}

pub struct ColumnData {
    pub title_text: Option<Text<'static>>,
    pub type_text: Text<'static>,
    pub values_text: Vec<Text<'static>>,
    pub width: usize,
}

pub fn format_col(col: Robj, col_name: Option<&str>, truncate: bool) -> ColumnData {
    let pillar_fn = R!("pillar::pillar").unwrap();
    let format_fn = R!("format").unwrap();

    let mut arg_list = vec![("x", col)];
    if let Some(title) = col_name {
        arg_list.push(("title", title.into()));
    }
    if truncate {
        arg_list.push(("width", 50.into()));
    }
    let args = Pairlist::from_pairs(arg_list);
    let pillared_col = pillar_fn.call(args).unwrap();

    let formatted_col: Vec<String> = format_fn
        .call(pairlist!(x = pillared_col))
        .unwrap()
        .as_string_vector()
        .unwrap();
    let mut text_iter = formatted_col
        .into_iter()
        .map(|item| item.into_text().unwrap());
    let title_text = match col_name {
        Some(_) => text_iter.next(),
        None => None,
    };
    let type_text = text_iter.next().unwrap_or_else(|| Text::raw(""));
    let values_text: Vec<Text<'static>> = text_iter.collect();
    let values_max_width = values_text.iter().map(|t| t.width()).max().unwrap_or(0);
    let width = title_text
        .as_ref()
        .map_or(0, |t| t.width())
        .max(type_text.width())
        .max(values_max_width);

    ColumnData {
        title_text,
        type_text,
        values_text,
        width,
    }
}

pub fn get_cols(
    data: &RobjContainer,
    offset: usize,
    end: usize,
    col_start_idx: usize,
    available_width: usize,
    truncate: bool,
) -> Vec<ColumnData> {
    let mut cols = vec![];
    let mut total_width = 0usize;

    match data {
        RobjContainer::DataFrame { robj } => {
            for (col_name, col) in robj.as_list().unwrap().iter().skip(col_start_idx) {
                let row_idxs = ((1 + offset as i32)..(1 + end as i32)).collect::<Integers>();
                let truncated_col = col.slice(row_idxs).unwrap();
                let formatted_col_data = format_col(truncated_col, Some(col_name), truncate);

                let sep = if cols.is_empty() { 0 } else { 1 };
                let new_total = total_width + sep + formatted_col_data.width;
                if new_total > available_width && !cols.is_empty() {
                    break;
                }

                cols.push(formatted_col_data);
                total_width = new_total;
                if total_width >= available_width {
                    break;
                }
            }
        }
        RobjContainer::List { robj } | RobjContainer::Vector { robj } => {
            let row_idxs = ((1 + offset as i32)..(1 + end as i32)).collect::<Integers>();
            let truncated_col = robj.slice(row_idxs).unwrap();
            let formatted_col_data = format_col(truncated_col, None, truncate);
            cols.push(formatted_col_data);
        }
        RobjContainer::Matrix { robj } => {
            let dim = robj.get_attrib("dim").unwrap();
            let dim_slice = dim.as_integer_slice().unwrap();
            let n_rows = dim_slice[0] as usize;
            let n_cols = dim_slice[1] as usize;
            let (_, col_names) = matrix_dimnames(robj);

            for col_idx in col_start_idx..n_cols {
                let row_idxs: Integers = (offset..end)
                    .map(|r| 1 + r as i32 + (col_idx as i32) * (n_rows as i32))
                    .collect();
                let truncated_col = robj.slice(row_idxs).unwrap();

                let col_name = col_names.as_ref().map(|names| names[col_idx].clone());
                let title = col_name.unwrap_or_else(|| format!("[,{}]", col_idx + 1));
                let formatted_col_data = format_col(truncated_col, Some(&title), truncate);

                let sep = if cols.is_empty() { 0 } else { 1 };
                let new_total = total_width + sep + formatted_col_data.width;
                if new_total > available_width && !cols.is_empty() {
                    break;
                }
                cols.push(formatted_col_data);
                total_width = new_total;
                if total_width >= available_width {
                    break;
                }
            }
        }
    }

    cols
}

pub fn transpose_cols(mut cols: Vec<Vec<Text<'static>>>) -> Vec<Row<'static>> {
    if cols.is_empty() || cols[0].is_empty() {
        return Vec::new();
    }

    let num_rows = cols[0].len();
    let mut col_iters: Vec<_> = cols.iter_mut().map(|col| col.drain(..)).collect();

    (0..num_rows)
        .map(|_| {
            let cells = col_iters
                .iter_mut()
                .map(|it| Cell::from(it.next().unwrap()));
            Row::new(cells)
        })
        .collect()
}

fn scroll_col_window(
    viewer: &mut Viewer,
    row_window: &RowWindow,
    available_width: usize,
    truncate: bool,
) -> Vec<ColumnData> {
    let (_, selected_col) = viewer.selected_cell();

    if selected_col < viewer.col_start_idx {
        viewer.col_start_idx = selected_col;
    }

    let mut col_data = get_cols(
        &viewer.data,
        row_window.offset,
        row_window.end,
        viewer.col_start_idx,
        available_width,
        truncate,
    );

    if viewer.col_start_idx + col_data.len() <= selected_col {
        let mut lo = viewer.col_start_idx;
        let mut hi = selected_col;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let len = get_cols(
                &viewer.data,
                row_window.offset,
                row_window.end,
                mid,
                available_width,
                truncate,
            )
            .len();
            if mid + len > selected_col {
                hi = mid;
            } else {
                lo = mid + 1;
            }
        }
        viewer.col_start_idx = lo;
        col_data = get_cols(
            &viewer.data,
            row_window.offset,
            row_window.end,
            viewer.col_start_idx,
            available_width,
            truncate,
        );
    }

    viewer.last_col_visible =
        viewer.n_cols == 0 || viewer.col_start_idx + col_data.len() >= viewer.n_cols;

    col_data
}

struct ColLayout {
    cols: Vec<Vec<Text<'static>>>,
    headers: Vec<Text<'static>>,
    widths: Vec<usize>,
    type_style: Style,
}

fn build_col_layout(col_data: Vec<ColumnData>) -> ColLayout {
    let mut cols = Vec::with_capacity(col_data.len());
    let mut headers = Vec::with_capacity(col_data.len());
    let mut widths = Vec::with_capacity(col_data.len());
    let type_style = col_data
        .first()
        .and_then(|first_col| first_col.type_text.lines.first())
        .and_then(|first_line| first_line.spans.first())
        .map(|first_span| first_span.style)
        .unwrap_or_else(|| Style::default().fg(Color::DarkGray));

    for c in col_data {
        let header_lines = match c.title_text {
            Some(title) => title
                .lines
                .into_iter()
                .chain(c.type_text.lines)
                .collect::<Vec<_>>(),
            _ => {
                let mut lines = vec![Line::default()];
                lines.extend(c.type_text.lines);
                lines
            }
        };
        let header_text = Text::from(header_lines);

        cols.push(c.values_text);
        headers.push(header_text);
        widths.push(c.width);
    }

    ColLayout {
        cols,
        headers,
        widths,
        type_style,
    }
}

fn render_summary_header(
    data: &RobjContainer,
    frame: &mut Frame,
    area: Rect,
    index_col_width: usize,
    type_style: Style,
) {
    let obj_sum_fn = R!("pillar::obj_sum").unwrap();
    let args = pairlist!(x = data.robj());
    let obj_summary = obj_sum_fn
        .call(args)
        .unwrap()
        .as_str()
        .unwrap_or("")
        .to_string();
    let text = format!("#{}a {}", " ".repeat(index_col_width - 1), obj_summary);
    let paragraph = Paragraph::new(text).style(type_style);
    frame.render_widget(paragraph, area);
}

fn get_index_labels(data: &RobjContainer, row_window: &RowWindow) -> (Vec<String>, bool) {
    let robj = data.robj();

    let names: Option<Vec<String>> = match data {
        RobjContainer::List { .. } | RobjContainer::Vector { .. } => robj
            .names()
            .map(|iter| iter.map(|name| name.to_string()).collect::<Vec<_>>()),
        _ => None,
    };

    let has_names = matches!(
        data,
        RobjContainer::List { .. } | RobjContainer::Vector { .. }
    );
    let mut labels = Vec::with_capacity(row_window.height);

    for i in 0..row_window.height {
        let row_idx = row_window.offset + i;
        let one_based_idx = row_idx + 1;

        let label = match data {
            RobjContainer::DataFrame { .. } => format!("{}", one_based_idx),
            RobjContainer::List { .. } => {
                if let Some(n) = &names {
                    if row_idx < n.len() && !n[row_idx].is_empty() {
                        n[row_idx].clone()
                    } else {
                        format!("[[{}]]", one_based_idx)
                    }
                } else {
                    format!("[[{}]]", one_based_idx)
                }
            }
            RobjContainer::Vector { .. } => {
                if let Some(n) = &names {
                    if row_idx < n.len() && !n[row_idx].is_empty() {
                        n[row_idx].clone()
                    } else {
                        format!("[{}]", one_based_idx)
                    }
                } else {
                    format!("[{}]", one_based_idx)
                }
            }
            RobjContainer::Matrix { robj } => {
                let (row_names, _) = matrix_dimnames(robj);
                if let Some(names) = &row_names {
                    if row_idx < names.len() && !names[row_idx].is_empty() {
                        names[row_idx].clone()
                    } else {
                        format!("[{},]", one_based_idx)
                    }
                } else {
                    format!("[{},]", one_based_idx)
                }
            }
        };

        labels.push(label);
    }

    (labels, has_names)
}

fn render_index_column(
    frame: &mut Frame,
    area: Rect,
    labels: &[String],
    has_names: bool,
    type_style: Style,
) {
    let mut lines: Vec<Line> = (0..HEADER_HEIGHT).map(|_| Line::from("")).collect();

    for label in labels {
        let display_string = if has_names {
            format!("{} ", label)
        } else {
            label.clone()
        };
        lines.push(Line::from(display_string).style(type_style));
    }

    frame.render_widget(Paragraph::new(lines), area);
}

fn build_table(col_layout: ColLayout) -> Table<'static> {
    let header_row = Row::new(col_layout.headers).height(HEADER_HEIGHT as u16);
    let widths: Vec<Constraint> = col_layout
        .widths
        .iter()
        .map(|&w| Constraint::Length(w as u16))
        .collect();
    let rows = transpose_cols(col_layout.cols);

    Table::new(rows, widths)
        .header(header_row)
        .cell_highlight_style(Style::default().add_modifier(Modifier::REVERSED))
}
