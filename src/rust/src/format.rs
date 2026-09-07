use ansi_to_tui::IntoText;
use extendr_api::prelude::*;
use ratatui::{
    text::Text,
    widgets::{Cell, Row},
};

#[derive(Debug)]
pub struct TableCol {
    pub title_text: Option<Text<'static>>,
    pub type_text: Text<'static>,
    pub values_text: Vec<Text<'static>>,
    pub width: usize,
}

pub fn format_col(
    col_name: Option<&str>,
    truncate: Option<usize>,
    col: Robj,
) -> extendr_api::Result<TableCol> {
    let pillar_fn = R!("pillar::ctl_new_pillar_list")?;
    let format_fn = R!("format")?;
    let controller = R!("tibble::tibble()")?;
    let mut arg_list = vec![("x", col), ("controller", controller)];
    if let Some(title) = col_name {
        arg_list.push(("title", title.into()));
    }
    let truncation_len = truncate.unwrap_or(100_000);
    arg_list.push(("width", Rint::from(truncation_len as i32).into_robj()));
    let args = Pairlist::from_pairs(arg_list);
    let pillared_col = pillar_fn.call(args)?.index(1)?;
    let formatted_col: Vec<String> = format_fn
        .call(pairlist!(x = pillared_col))?
        .as_string_vector()
        .ok_or_else(|| {
            Error::Other(
                "internal error: expected format() to return a character vector".to_string(),
            )
        })?;

    let mut texts = Vec::with_capacity(formatted_col.len());
    for item in formatted_col {
        let text = item.into_text().map_err(|e| {
            Error::Other(format!(
                "internal error: failed to parse formatted column text: {e}"
            ))
        })?;
        texts.push(text);
    }
    let mut text_iter = texts.into_iter();

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
    Ok(TableCol {
        title_text,
        type_text,
        values_text,
        width,
    })
}

pub fn transpose_cols(mut cols: Vec<Vec<Text<'static>>>) -> extendr_api::Result<Vec<Row<'static>>> {
    if cols.is_empty() || cols[0].is_empty() {
        return Ok(Vec::new());
    }
    let num_rows = cols[0].len();
    let mut col_iters: Vec<_> = cols.iter_mut().map(|col| col.drain(..)).collect();
    let mut rows = Vec::with_capacity(num_rows);
    for _ in 0..num_rows {
        let mut cells = Vec::with_capacity(col_iters.len());
        for it in col_iters.iter_mut() {
            let text = it.next().ok_or_else(|| {
                Error::Other("internal error: table columns have mismatched lengths".to_string())
            })?;
            cells.push(Cell::from(text));
        }
        rows.push(Row::new(cells));
    }
    Ok(rows)
}
