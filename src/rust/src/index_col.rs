use crate::viewer::HEADER_HEIGHT;
use extendr_api::prelude::*;
use ratatui::{layout::Rect, style::Color, text::Line, widgets::Paragraph, Frame};
use std::ops::Range;

pub fn render_index_column(frame: &mut Frame, area: Rect, labels: &[String]) {
    let mut lines: Vec<Line> = (0..HEADER_HEIGHT).map(|_| Line::from("")).collect();
    for label in labels {
        lines.push(Line::from(label.as_str()).style(Color::Indexed(248)));
    }
    frame.render_widget(Paragraph::new(lines), area);
}

pub fn get_index_labels(data: &Robj, row_window: Range<usize>) -> Vec<String> {
    if data.is_frame() {
        row_window.map(|i| (i + 1).to_string()).collect()
    } else if data.is_list() {
        let names = list_names(data);
        row_window
            .map(|i| {
                label_or_index(
                    names.as_deref(),
                    i,
                    |n| format!("[[\"{n}\"]]"),
                    |i| format!("[[{}]]", i + 1),
                )
            })
            .collect()
    } else if data.is_matrix() {
        let row_names = first_dimname(data);
        row_window
            .map(|i| {
                label_or_index(
                    row_names.as_deref(),
                    i,
                    |n| format!("[\"{n}\", ]"),
                    |i| format!("[{}, ]", i + 1),
                )
            })
            .collect()
    } else if data.is_array() {
        let ndim = data
            .get_attrib(&Robj::from("dim"))
            .and_then(|d| d.as_integer_vector())
            .map(|v| v.len())
            .unwrap_or(2);
        let trailing_commas = ", ".repeat(ndim.saturating_sub(1));
        let row_names = first_dimname(data);
        row_window
            .map(|i| match row_names.as_ref().and_then(|n| n.get(i)) {
                Some(name) if !name.is_empty() => {
                    format!("[\"{}\"{}]", name, trailing_commas)
                }
                _ => format!("[{}{}]", i + 1, trailing_commas),
            })
            .collect()
    } else {
        let names = list_names(data);
        row_window
            .map(|i| match names.as_ref().and_then(|n| n.get(i)) {
                Some(name) if !name.is_empty() => format!("[\"{}\"]", name),
                _ => format!("[{}]", i + 1),
            })
            .collect()
    }
}

fn label_or_index(
    names: Option<&[String]>,
    i: usize,
    named: impl Fn(&str) -> String,
    indexed: impl Fn(usize) -> String,
) -> String {
    match names.and_then(|n| n.get(i)) {
        Some(name) if !name.is_empty() => named(name),
        _ => indexed(i),
    }
}

fn list_names(data: &Robj) -> Option<Vec<String>> {
    data.names()
        .map(|iter| iter.map(|s| s.to_string()).collect())
}

fn first_dimname(data: &Robj) -> Option<Vec<String>> {
    let dimnames = data.get_attrib(&Robj::from("dimnames"))?;
    let list = dimnames.as_list()?;
    let first = list.elt(0).ok()?;
    if first.is_null() {
        return None;
    }
    first
        .as_str_iter()
        .map(|iter| iter.map(|s| s.to_string()).collect())
}
