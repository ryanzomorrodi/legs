use crate::app::App;
use ansi_to_tui::IntoText;
use extendr_api::prelude::*;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Text},
    widgets::{Cell, Paragraph, Row, Table},
    Frame,
};

const HEADER_HEIGHT: usize = 2;

pub struct ColumnData {
    pub title_text: Text<'static>,
    pub type_text: Text<'static>,
    pub values: Vec<Text<'static>>,
    pub width: usize,
}

pub fn get_cols(
    df: &Dataframe<Robj>,
    offset: usize,
    end: usize,
    col_start_idx: usize,
    available_width: usize,
) -> Vec<ColumnData> {
    let pillar_fn = R!("pillar::pillar").unwrap();
    let format_fn = R!("format").unwrap();
    let mut cols = vec![];
    let mut total_width = 0usize;
    for (col_name, col) in df.as_list().unwrap().iter().skip(col_start_idx) {
        let row_idxs = ((1 + offset as i32)..(1 + end as i32)).collect::<Integers>();
        let truncated_col = col.slice(row_idxs).unwrap();
        let pillared_col = pillar_fn
            .call(pairlist!(x = truncated_col, title = col_name))
            .unwrap();
        let formatted_col = format_fn
            .call(pairlist!(x = pillared_col))
            .unwrap()
            .as_string_vector()
            .unwrap();

        // format() now returns [title, type, value_1, .., value_N] since we
        // pass `title` — two header rows instead of one. Guard against a
        // pathologically short vector rather than assuming both are present.
        let (title_text, type_text, values): (Text<'static>, Text<'static>, Vec<Text<'static>>) =
            if formatted_col.len() >= 2 {
                let title_str = &formatted_col[0];
                let type_str = &formatted_col[1];
                let value_strs = &formatted_col[2..];

                let title_text = title_str
                    .into_text()
                    .unwrap_or_else(|_| Text::raw(title_str.clone()));
                let type_text = type_str
                    .into_text()
                    .unwrap_or_else(|_| Text::raw(type_str.clone()));
                let values = value_strs
                    .iter()
                    .map(|s| s.into_text().unwrap_or_else(|_| Text::raw(s.clone())))
                    .collect();
                (title_text, type_text, values)
            } else {
                (Text::raw(col_name), Text::raw(""), Vec::new())
            };

        let values_max_width = values.iter().map(|t| t.width()).max().unwrap_or(0);
        // pillar already sized title/type/values to a shared width, so these
        // should already agree — max() here is just a defensive fallback.
        let col_width = title_text
            .width()
            .max(type_text.width())
            .max(values_max_width);

        let sep = if cols.is_empty() { 0 } else { 1 };
        let new_total = total_width + sep + col_width;
        if new_total > available_width && !cols.is_empty() {
            break;
        }

        cols.push(ColumnData {
            title_text,
            type_text,
            values,
            width: col_width,
        });
        total_width = new_total;
        if total_width >= available_width {
            break;
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

struct RowWindow {
    offset: usize,
    end: usize,
    height: usize,
}

fn compute_row_window(app: &App, available_height: usize) -> RowWindow {
    let height = available_height
        .saturating_sub(HEADER_HEIGHT)
        .min(app.n_rows);

    let absolute_row = app.state.selected_cell().map(|(r, _)| r).unwrap_or(0);
    let initial_offset = app.state.offset();
    let offset = if absolute_row < initial_offset {
        absolute_row
    } else if height > 0 && absolute_row > initial_offset + height - 1 {
        absolute_row + 1 - height
    } else {
        initial_offset
    };
    let end = (offset + height).min(app.n_rows);

    RowWindow {
        offset,
        end,
        height,
    }
}

struct ColLayout {
    cols: Vec<Vec<Text<'static>>>,
    headers: Vec<Text<'static>>,
    widths: Vec<usize>,
    last_col_visible: bool,
    type_style: Style,
}

fn compute_col_layout(app: &mut App, row_window: &RowWindow, available_width: usize) -> ColLayout {
    let selected_col = app.state.selected_cell().map(|(_, c)| c).unwrap_or(0);

    // Selection moved left of the visible window: scroll left to meet it.
    // Cheap — no R call needed, just a direct index move.
    if selected_col < app.col_start_idx {
        app.col_start_idx = selected_col;
    }

    let mut col_data = get_cols(
        &app.df,
        row_window.offset,
        row_window.end,
        app.col_start_idx,
        available_width,
    );

    if app.col_start_idx + col_data.len() <= selected_col {
        let mut lo = app.col_start_idx;
        let mut hi = selected_col;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let len = get_cols(
                &app.df,
                row_window.offset,
                row_window.end,
                mid,
                available_width,
            )
            .len();
            if mid + len > selected_col {
                hi = mid;
            } else {
                lo = mid + 1;
            }
        }
        app.col_start_idx = lo;
        col_data = get_cols(
            &app.df,
            row_window.offset,
            row_window.end,
            app.col_start_idx,
            available_width,
        );
    }

    let last_col_visible = app.n_cols == 0 || app.col_start_idx + col_data.len() >= app.n_cols;

    let mut cols = Vec::with_capacity(col_data.len());
    let mut headers = Vec::with_capacity(col_data.len());
    let mut widths = Vec::with_capacity(col_data.len());
    let mut type_style = Style::default().fg(Color::DarkGray);
    let mut style_captured = false;

    for ((col_name, _), col) in app
        .df
        .as_list()
        .unwrap()
        .iter()
        .skip(app.col_start_idx)
        .zip(col_data)
    {
        let type_line = col
            .type_text
            .lines
            .into_iter()
            .next()
            .unwrap_or_else(|| Line::from(""));

        if !style_captured {
            if let Some(styled_span) = type_line
                .spans
                .iter()
                .find(|span| span.style != Style::default())
            {
                type_style = styled_span.style;
                style_captured = true;
            }
        }

        let title_line = col
            .title_text
            .lines
            .into_iter()
            .next()
            .unwrap_or_else(|| Line::from(col_name));

        headers.push(Text::from(vec![title_line, type_line]));
        widths.push(col.width);
        cols.push(col.values);
    }

    ColLayout {
        cols,
        headers,
        widths,
        last_col_visible,
        type_style,
    }
}

fn render_summary_header(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    index_col_width: usize,
    type_style: Style,
) {
    let text = format!(
        "#{}a data.frame: {} x {}",
        " ".repeat(index_col_width - 1),
        app.n_rows,
        app.n_cols
    );
    let paragraph = Paragraph::new(text).style(type_style);
    frame.render_widget(paragraph, area);
}

fn render_index_column(frame: &mut Frame, area: Rect, row_window: &RowWindow, type_style: Style) {
    let mut lines: Vec<Line> = (0..HEADER_HEIGHT).map(|_| Line::from("")).collect();
    for i in 0..row_window.height {
        let row_idx = row_window.offset + i + 1;
        lines.push(Line::from(format!("{}", row_idx)).style(type_style));
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
        .cell_highlight_style(Style::default().bg(Color::Yellow).fg(Color::Black))
}

pub fn render(app: &mut App, frame: &mut Frame) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(frame.area());
    let (summary_area, body_area) = (outer[0], outer[1]);

    let row_window = compute_row_window(app, body_area.height as usize);
    let index_col_width = row_window.end.to_string().len() + 1;

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(index_col_width as u16),
            Constraint::Min(0),
        ])
        .split(body_area);
    let (index_area, table_area) = (body[0], body[1]);

    let col_layout = compute_col_layout(app, &row_window, table_area.width as usize);
    app.last_col_visible = col_layout.last_col_visible;

    render_summary_header(
        frame,
        summary_area,
        app,
        index_col_width,
        col_layout.type_style,
    );
    render_index_column(frame, index_area, &row_window, col_layout.type_style);

    let table = build_table(col_layout);

    let (absolute_row, absolute_col) = app.state.selected_cell().unwrap_or((0, 0));
    let relative_row = absolute_row.saturating_sub(row_window.offset);
    let relative_col = absolute_col.saturating_sub(app.col_start_idx);
    app.state.select_cell(Some((relative_row, relative_col)));
    *app.state.offset_mut() = 0;

    frame.render_stateful_widget(table, table_area, &mut app.state);

    app.state.select_cell(Some((absolute_row, absolute_col)));
    *app.state.offset_mut() = row_window.offset;
}
