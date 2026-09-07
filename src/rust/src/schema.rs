use extendr_api::prelude::*;
use std::ops::Range;

#[derive(Debug, Clone)]
pub enum RColumn {
    Flat,
    Packed {
        ncols: usize,
        children: Vec<(String, RColumn)>,
    },
}

fn total_pillar_cols(col: &RColumn) -> usize {
    match col {
        RColumn::Flat => 1,
        RColumn::Packed { ncols, .. } => *ncols,
    }
}

fn matrix_child_names(x: &Robj, ncols: usize) -> extendr_api::Result<Vec<String>> {
    let colnames = R!("colnames({{x}})")?;
    if let Some(names) = colnames.as_str_vector() {
        if names.len() == ncols {
            return Ok(names.iter().map(|n| format!("[, \"{}\"]", n)).collect());
        }
    }
    Ok((0..ncols).map(|j| format!("[, {}]", j + 1)).collect())
}

fn build_schema(x: &Robj, is_root: bool) -> extendr_api::Result<RColumn> {
    if x.is_frame() {
        let list = x.as_list().ok_or_else(|| {
            Error::Other("internal error: data.frame should coerce to a list".to_string())
        })?;
        let mut children: Vec<(String, RColumn)> = Vec::with_capacity(list.len());
        for (name, child) in list.iter() {
            let label = if is_root {
                name.to_string()
            } else {
                format!("${}", name)
            };
            children.push((label, build_schema(&child, false)?));
        }
        let ncols = children.iter().map(|(_, c)| total_pillar_cols(c)).sum();
        Ok(RColumn::Packed { ncols, children })
    } else if x.is_matrix() {
        let ncols = x.ncols();
        let children = matrix_child_names(x, ncols)?
            .into_iter()
            .map(|n| (n, RColumn::Flat))
            .collect();
        Ok(RColumn::Packed { ncols, children })
    } else {
        Ok(RColumn::Flat)
    }
}

fn root_children(x: &Robj) -> extendr_api::Result<Vec<(String, RColumn)>> {
    match build_schema(x, true)? {
        RColumn::Packed { children, .. } => Ok(children),
        RColumn::Flat => Ok(vec![(String::new(), RColumn::Flat)]),
    }
}

fn slice_rows(col: &Robj, rows: Range<usize>) -> extendr_api::Result<Robj> {
    match col.dim() {
        Some(dim) if dim.len() > 1 => slice_array_rows(col, rows),
        _ => {
            let indices: Robj = (rows.start..rows.end)
                .map(|idx| (idx + 1) as i32)
                .collect::<Integers>()
                .into();
            col.slice(indices)
        }
    }
}

fn slice_array_rows(x: &Robj, rows: Range<usize>) -> extendr_api::Result<Robj> {
    let dim_robj = x.dim().ok_or_else(|| {
        Error::Other("internal error: expected an array with a dim attribute".to_string())
    })?;
    let dim = dim_robj.as_robj().as_integer_vector().ok_or_else(|| {
        Error::Other("internal error: array dim attribute was not an integer vector".to_string())
    })?;
    let nrow = *dim
        .first()
        .ok_or_else(|| Error::Other("internal error: array dim attribute was empty".to_string()))?
        as usize;
    let n_chunks: usize = dim[1..].iter().map(|&d| d as usize).product();
    let chunk_len = rows.len();

    let mut indices = Vec::with_capacity(n_chunks * chunk_len);
    for c in 0..n_chunks {
        let base = c * nrow;
        indices.extend((rows.start..rows.end).map(|r| (base + r + 1) as i32));
    }
    let idx: Robj = indices.into_iter().collect::<Integers>().into();

    let mut sliced = x.slice(idx)?;
    let mut new_dim = dim;
    new_dim[0] = chunk_len as i32;
    sliced.set_attrib("dim", new_dim)?;

    if let Some(dimnames) = x.get_attrib("dimnames") {
        if let Some(list) = dimnames.as_list() {
            let mut new_dimnames: Vec<Robj> = Vec::with_capacity(list.len());
            for (i, (_, names)) in list.iter().enumerate() {
                if i == 0 {
                    new_dimnames.push(slice_dim_names(names, &rows)?);
                } else {
                    new_dimnames.push(names);
                }
            }
            sliced.set_attrib("dimnames", List::from_values(new_dimnames))?;
        }
    }

    Ok(sliced)
}

fn slice_dim_names(names: Robj, rows: &Range<usize>) -> extendr_api::Result<Robj> {
    match names.as_str_vector() {
        Some(strs) => {
            let mut out = Vec::with_capacity(rows.len());
            for i in rows.clone() {
                let s = strs.get(i).ok_or_else(|| {
                    Error::Other("internal error: row index out of range for dimnames".to_string())
                })?;
                out.push(*s);
            }
            Ok(out.into_iter().collect::<Strings>().into())
        }
        None => Ok(names),
    }
}

fn matrix_col_rows(
    x: &Robj,
    col_idx: usize,
    nrow: usize,
    rows: Option<Range<usize>>,
) -> extendr_api::Result<Robj> {
    let (start, end) = match rows {
        Some(r) => (r.start, r.end),
        None => (0, nrow),
    };
    let base = col_idx * nrow;
    let indices: Robj = ((base + start + 1)..=(base + end))
        .map(|i| i as i32)
        .collect::<Integers>()
        .into();
    x.slice(indices)
}

fn get_value(x: &Robj, path: &[usize], rows: Option<Range<usize>>) -> extendr_api::Result<Robj> {
    let Some((&first, rest)) = path.split_first() else {
        return match rows {
            Some(r) => slice_rows(x, r),
            None => Ok(x.clone()),
        };
    };

    if x.is_frame() {
        let elt = x
            .as_list()
            .ok_or_else(|| Error::Other("internal error: expected a list-like object".to_string()))?
            .elt(first)?;
        get_value(&elt, rest, rows)
    } else if x.is_matrix() {
        if !rest.is_empty() {
            return Err(Error::Other(
                "internal error: matrix pillar should not have a further path".to_string(),
            ));
        }
        let dim_robj = x.dim().ok_or_else(|| {
            Error::Other("internal error: expected a matrix with a dim attribute".to_string())
        })?;
        let dim = dim_robj.as_robj().as_integer_vector().ok_or_else(|| {
            Error::Other(
                "internal error: matrix dim attribute was not an integer vector".to_string(),
            )
        })?;
        let nrow = *dim.first().ok_or_else(|| {
            Error::Other("internal error: matrix dim attribute was empty".to_string())
        })? as usize;
        matrix_col_rows(x, first, nrow, rows)
    } else {
        if !rest.is_empty() {
            return Err(Error::Other(
                "internal error: flat pillar should not have a further path".to_string(),
            ));
        }
        match rows {
            Some(r) => slice_rows(x, r),
            None => Ok(x.clone()),
        }
    }
}

struct Pillar {
    path: Vec<usize>,
    display_path: String,
    full_display_path: String,
}

fn collect_pillars(
    children: &[(String, RColumn)],
    prefix_path: &[usize],
    prefix_segs: &[String],
    out: &mut Vec<(Vec<usize>, Vec<String>)>,
) {
    for (i, (name, col)) in children.iter().enumerate() {
        let mut path = prefix_path.to_vec();
        path.push(i);
        let mut segs = prefix_segs.to_vec();
        segs.push(name.clone());

        match col {
            RColumn::Flat => out.push((path, segs)),
            RColumn::Packed { children: sub, .. } => collect_pillars(sub, &path, &segs, out),
        }
    }
}

fn resolve_pillars(children: &[(String, RColumn)]) -> Vec<Pillar> {
    let mut raw = Vec::new();
    collect_pillars(children, &[], &[], &mut raw);

    let mut pillars = Vec::with_capacity(raw.len());
    let mut last_parent: Option<Vec<usize>> = None;
    for (path, segs) in raw {
        let full_display_path = segs.concat();
        let parent = path[..path.len() - 1].to_vec();
        let display_path = if last_parent.as_deref() == Some(parent.as_slice()) {
            segs.last().cloned().unwrap_or_default()
        } else {
            full_display_path.clone()
        };
        last_parent = Some(parent);
        pillars.push(Pillar {
            path,
            display_path,
            full_display_path,
        });
    }
    pillars
}

pub struct RSchema {
    pillars: Vec<Pillar>,
    pub nrow: usize,
}

impl RSchema {
    pub fn build(x: &Robj) -> extendr_api::Result<Self> {
        let nrow = nrow(x)?;
        Ok(RSchema {
            pillars: resolve_pillars(&root_children(x)?),
            nrow,
        })
    }

    pub fn len(&self) -> usize {
        self.pillars.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pillars.is_empty()
    }

    pub fn full_name(&self, idx: usize) -> Option<&str> {
        self.pillars.get(idx).map(|p| p.full_display_path.as_str())
    }

    pub fn cell(&self, x: &Robj, row: usize, col: usize) -> extendr_api::Result<Robj> {
        let pillar = self.pillars.get(col).ok_or_else(|| {
            Error::Other(format!("internal error: column index {col} out of range"))
        })?;
        let value = get_value(x, &pillar.path, Some(row..row + 1))?;
        if value.is_list() {
            value.index(1)
        } else if value.is_array() {
            let n_dims = value
                .dim()
                .ok_or_else(|| {
                    Error::Other(
                        "internal error: expected an array with a dim attribute".to_string(),
                    )
                })?
                .len();
            let idx = 1;
            let commas = ",".repeat(n_dims.saturating_sub(1));
            let code = format!("value[{}{}]", idx, commas);

            R!(r#"
                local({
                    value <- {{value}}
                    eval(parse(text = {{code}}))
                })
            "#)
        } else {
            Ok(value)
        }
    }

    pub fn column_at<'a>(
        &'a self,
        x: &'a Robj,
        idx: usize,
        rows: Option<Range<usize>>,
    ) -> extendr_api::Result<(&'a str, Robj)> {
        let pillar = self.pillars.get(idx).ok_or_else(|| {
            Error::Other(format!("internal error: column index {idx} out of range"))
        })?;
        let value = get_value(x, &pillar.path, rows)?;
        Ok((pillar.display_path.as_str(), value))
    }
}

pub fn nrow(x: &Robj) -> extendr_api::Result<usize> {
    if x.is_frame() {
        let row_names = x.get_attrib("row.names").ok_or_else(|| {
            Error::Other("internal error: data.frame is missing row.names".to_string())
        })?;
        Ok(row_names.len())
    } else if x.is_matrix() || x.is_array() {
        let dim_robj = x.dim().ok_or_else(|| {
            Error::Other("internal error: expected a matrix/array with a dim attribute".to_string())
        })?;
        let dim = dim_robj.as_robj().as_integer_vector().ok_or_else(|| {
            Error::Other("internal error: dim attribute was not an integer vector".to_string())
        })?;
        let first = *dim
            .first()
            .ok_or_else(|| Error::Other("internal error: dim attribute was empty".to_string()))?;
        Ok(first as usize)
    } else {
        Ok(x.len())
    }
}
