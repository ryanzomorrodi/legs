use crate::{
    format::{format_col, TableCol},
    schema::RSchema,
};
use extendr_api::prelude::*;
use ratatui::text::{Line, Text};
use std::{collections::VecDeque, ops::Range};

struct ColEntry {
    idx: usize,
    table_col: TableCol,
    robj: Robj,
}

impl ColEntry {
    fn new(idx: usize, name: &str, truncate: Option<usize>, robj: Robj) -> Self {
        let col_name = if name.is_empty() { None } else { Some(name) };
        let table_col = format_col(col_name, truncate, robj.clone());
        Self {
            idx,
            table_col,
            robj,
        }
    }

    fn full_path_width(&self, schema: &RSchema) -> usize {
        self.table_col
            .width
            .max(schema.full_name(self.idx).unwrap().len())
    }
}

pub struct ColumnLayout {
    pub headers: Vec<Text<'static>>,
    pub values: Vec<Vec<Text<'static>>>,
    pub widths: Vec<usize>,
    pub col_range: Range<usize>,
}

pub fn build_column_layout(
    schema: &RSchema,
    data: &Robj,
    row_window: Range<usize>,
    col_range: Range<usize>,
    col_start_from_start: bool,
    viewable_width: usize,
    truncate: Option<usize>,
) -> ColumnLayout {
    let entries = select_visible_columns(
        schema,
        data,
        row_window,
        col_range,
        col_start_from_start,
        viewable_width,
        truncate,
    );
    let col_range = match (entries.front(), entries.back()) {
        (Some(first), Some(last)) => first.idx..(last.idx + 1),
        _ => 0..0,
    };
    let mut headers = Vec::with_capacity(entries.len());
    let mut values = Vec::with_capacity(entries.len());
    let mut widths = Vec::with_capacity(entries.len());
    for entry in entries {
        let mut header_text = entry
            .table_col
            .title_text
            .unwrap_or_else(|| Text::from(Line::from("")));
        header_text.lines.extend(entry.table_col.type_text.lines);
        headers.push(header_text);
        values.push(entry.table_col.values_text);
        widths.push(entry.table_col.width);
    }
    ColumnLayout {
        headers,
        values,
        widths,
        col_range,
    }
}

fn select_visible_columns(
    schema: &RSchema,
    data: &Robj,
    row_window: Range<usize>,
    col_range: Range<usize>,
    col_start_from_start: bool,
    viewable_width: usize,
    truncate: Option<usize>,
) -> VecDeque<ColEntry> {
    let ncols = schema.len();
    if ncols == 0 {
        return VecDeque::new();
    }
    let mut entries = if col_start_from_start {
        let anchor = col_range.start.min(ncols - 1);
        fill_right_from(
            schema,
            data,
            row_window.clone(),
            anchor,
            viewable_width,
            truncate,
        )
    } else {
        let anchor = col_range.end.min(ncols - 1);
        fill_left_from(
            schema,
            data,
            row_window.clone(),
            anchor,
            viewable_width,
            truncate,
        )
    };
    if col_start_from_start {
        extend_left(
            schema,
            data,
            row_window,
            viewable_width,
            truncate,
            &mut entries,
        );
    } else {
        extend_right(
            schema,
            data,
            row_window,
            viewable_width,
            truncate,
            &mut entries,
        );
    }
    finalize_leftmost_header(schema, truncate, &mut entries);
    entries
}

fn fill_right_from(
    schema: &RSchema,
    data: &Robj,
    row_window: Range<usize>,
    anchor_idx: usize,
    viewable_width: usize,
    truncate: Option<usize>,
) -> VecDeque<ColEntry> {
    let mut entries = VecDeque::new();
    let ncols = schema.len();
    if anchor_idx >= ncols {
        return entries;
    }

    let (name, robj) = schema.column_at(data, anchor_idx, Some(row_window.clone()));
    let anchor = ColEntry::new(anchor_idx, name, truncate, robj);
    let leftmost_reserved_width = anchor.full_path_width(schema);
    entries.push_back(anchor);

    let mut used_width = 0usize;
    for idx in (anchor_idx + 1)..ncols {
        let (name, robj) = schema.column_at(data, idx, Some(row_window.clone()));
        let entry = ColEntry::new(idx, name, truncate, robj);
        let candidate_width = used_width + leftmost_reserved_width + entry.table_col.width + 1;
        if candidate_width > viewable_width {
            break;
        }
        used_width += entry.table_col.width + 1;
        entries.push_back(entry);
    }

    entries
}

fn fill_left_from(
    schema: &RSchema,
    data: &Robj,
    row_window: Range<usize>,
    anchor_idx: usize,
    viewable_width: usize,
    truncate: Option<usize>,
) -> VecDeque<ColEntry> {
    let mut entries = VecDeque::new();

    let (name, robj) = schema.column_at(data, anchor_idx, Some(row_window.clone()));
    let anchor = ColEntry::new(anchor_idx, name, truncate, robj);
    let mut used_width = anchor.table_col.width + 1;
    entries.push_back(anchor);

    for idx in (0..anchor_idx).rev() {
        let (name, robj) = schema.column_at(data, idx, Some(row_window.clone()));
        let entry = ColEntry::new(idx, name, truncate, robj);
        let reserved_width = entry.full_path_width(schema);
        let candidate_width = used_width + reserved_width + 1;
        if candidate_width > viewable_width {
            break;
        }
        used_width += entry.table_col.width + 1;
        entries.push_front(entry);
    }

    entries
}

fn extend_left(
    schema: &RSchema,
    data: &Robj,
    row_window: Range<usize>,
    viewable_width: usize,
    truncate: Option<usize>,
    entries: &mut VecDeque<ColEntry>,
) {
    let first_idx = entries.front().map(|entry| entry.idx).unwrap_or(0);
    let mut used_width: usize = total_width(entries);

    for idx in (0..first_idx).rev() {
        let (name, robj) = schema.column_at(data, idx, Some(row_window.clone()));
        let entry = ColEntry::new(idx, name, truncate, robj);
        let reserved_width = entry.full_path_width(schema);
        let candidate_width = used_width + reserved_width + 1;
        if candidate_width > viewable_width {
            break;
        }
        used_width += entry.table_col.width + 1;
        entries.push_front(entry);
    }
}

fn extend_right(
    schema: &RSchema,
    data: &Robj,
    row_window: Range<usize>,
    viewable_width: usize,
    truncate: Option<usize>,
    entries: &mut VecDeque<ColEntry>,
) {
    let last_idx = entries.back().map(|entry| entry.idx).unwrap_or(0);
    let mut used_width: usize = total_width(entries);

    for idx in (last_idx + 1)..schema.len() {
        let (name, robj) = schema.column_at(data, idx, Some(row_window.clone()));
        let entry = ColEntry::new(idx, name, truncate, robj);
        let candidate_width = used_width + entry.table_col.width + 1;
        if candidate_width > viewable_width {
            break;
        }
        used_width += entry.table_col.width + 1;
        entries.push_back(entry);
    }
}

fn total_width(entries: &VecDeque<ColEntry>) -> usize {
    entries.iter().fold(0, |acc, entry| {
        if acc == 0 {
            entry.table_col.width
        } else {
            acc + 1 + entry.table_col.width
        }
    })
}

fn finalize_leftmost_header(
    schema: &RSchema,
    truncate: Option<usize>,
    entries: &mut VecDeque<ColEntry>,
) {
    let Some(leftmost) = entries.front_mut() else {
        return;
    };
    if let Some(full_name) = schema.full_name(leftmost.idx) {
        let col_name = if full_name.is_empty() {
            None
        } else {
            Some(full_name)
        };
        let mut table_col = format_col(col_name, truncate, leftmost.robj.clone());
        table_col.width = table_col.width.max(full_name.len());
        leftmost.table_col = table_col;
    }
}
