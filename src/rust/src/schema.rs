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

fn matrix_child_names(x: &Robj, ncols: usize) -> Vec<String> {
    let colnames = R!("colnames({{x}})").expect("colnames call failed");
    if let Some(names) = colnames.as_str_vector() {
        if names.len() == ncols {
            return names.iter().map(|n| format!("[, \"{}\"]", n)).collect();
        }
    }
    (0..ncols).map(|j| format!("[, {}]", j + 1)).collect()
}

fn build_schema(x: &Robj, is_root: bool) -> RColumn {
    if x.is_frame() {
        let list = x.as_list().expect("data.frame should coerce to List");
        let children: Vec<(String, RColumn)> = list
            .iter()
            .map(|(name, child)| {
                let label = if is_root {
                    name.to_string()
                } else {
                    format!("${}", name)
                };
                (label, build_schema(&child, false))
            })
            .collect();
        let ncols = children.iter().map(|(_, c)| total_pillar_cols(c)).sum();
        RColumn::Packed { ncols, children }
    } else if x.is_matrix() {
        let ncols = x.ncols();
        let children = matrix_child_names(x, ncols)
            .into_iter()
            .map(|n| (n, RColumn::Flat))
            .collect();
        RColumn::Packed { ncols, children }
    } else {
        RColumn::Flat
    }
}

fn root_children(x: &Robj) -> Vec<(String, RColumn)> {
    match build_schema(x, true) {
        RColumn::Packed { children, .. } => children,
        RColumn::Flat => vec![(String::new(), RColumn::Flat)],
    }
}

fn slice_rows(col: &Robj, rows: Range<usize>) -> Robj {
    match col.dim() {
        Some(dim) if dim.len() > 1 => slice_array_rows(col, rows),
        _ => {
            let indices: Robj = (rows.start..rows.end)
                .map(|idx| (idx + 1) as i32)
                .collect::<Integers>()
                .into();
            col.slice(indices).expect("row slice failed")
        }
    }
}

fn slice_array_rows(x: &Robj, rows: Range<usize>) -> Robj {
    let dim = x.dim().unwrap().as_robj().as_integer_vector().unwrap();
    let nrow = dim[0] as usize;
    let n_chunks: usize = dim[1..].iter().map(|&d| d as usize).product();
    let chunk_len = rows.len();

    let mut indices = Vec::with_capacity(n_chunks * chunk_len);
    for c in 0..n_chunks {
        let base = c * nrow;
        indices.extend((rows.start..rows.end).map(|r| (base + r + 1) as i32));
    }
    let idx: Robj = indices.into_iter().collect::<Integers>().into();

    let mut sliced = x.slice(idx).expect("array row slice failed");
    let mut new_dim = dim;
    new_dim[0] = chunk_len as i32;
    sliced
        .set_attrib("dim", new_dim)
        .expect("failed to set dim");

    if let Some(dimnames) = x.get_attrib("dimnames") {
        if let Some(list) = dimnames.as_list() {
            let new_dimnames: Vec<Robj> = list
                .iter()
                .enumerate()
                .map(|(i, (_, names))| {
                    if i == 0 {
                        slice_dim_names(names, &rows)
                    } else {
                        names
                    }
                })
                .collect();
            sliced
                .set_attrib("dimnames", List::from_values(new_dimnames))
                .expect("failed to set dimnames");
        }
    }

    sliced
}

fn slice_dim_names(names: Robj, rows: &Range<usize>) -> Robj {
    match names.as_str_vector() {
        Some(strs) => rows.clone().map(|i| strs[i]).collect::<Strings>().into(),
        None => names,
    }
}

fn matrix_col_rows(x: &Robj, col_idx: usize, nrow: usize, rows: Option<Range<usize>>) -> Robj {
    let (start, end) = match rows {
        Some(r) => (r.start, r.end),
        None => (0, nrow),
    };
    let base = col_idx * nrow;
    let indices: Robj = ((base + start + 1)..=(base + end))
        .map(|i| i as i32)
        .collect::<Integers>()
        .into();
    x.slice(indices).expect("matrix column/row slice failed")
}

fn get_value(x: &Robj, path: &[usize], rows: Option<Range<usize>>) -> Robj {
    let Some((&first, rest)) = path.split_first() else {
        return match rows {
            Some(r) => slice_rows(x, r),
            None => x.clone(),
        };
    };

    if x.is_frame() {
        let elt = x
            .as_list()
            .expect("expected list-like Robj")
            .elt(first)
            .expect("index out of range");
        get_value(&elt, rest, rows)
    } else if x.is_matrix() {
        debug_assert!(
            rest.is_empty(),
            "matrix pillar should not have further path"
        );
        let nrow = x.dim().unwrap().as_robj().as_integer_vector().unwrap()[0] as usize;
        matrix_col_rows(x, first, nrow, rows)
    } else {
        debug_assert!(rest.is_empty(), "flat pillar should not have further path");
        match rows {
            Some(r) => slice_rows(x, r),
            None => x.clone(),
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
            segs.last().cloned().unwrap()
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
    pub fn build(x: &Robj) -> Self {
        let nrow = nrow(x);
        RSchema {
            pillars: resolve_pillars(&root_children(x)),
            nrow,
        }
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

    pub fn cell(&self, x: &Robj, row: usize, col: usize) -> Robj {
        let pillar = &self.pillars[col];
        let value = get_value(x, &pillar.path, Some(row..row + 1));
        if value.is_list() {
            value.index(1).unwrap()
        } else if value.is_array() {
            let n_dims = value.dim().unwrap().len();
            let idx = 1;
            let commas = ",".repeat(n_dims.saturating_sub(1));
            let code = format!("value[{}{}]", idx, commas);

            R!(r#"
                local({
                    value <- {{value}}
                    eval(parse(text = {{code}}))
                })
            "#)
            .unwrap()
        } else {
            value
        }
    }

    pub fn column_at<'a>(
        &'a self,
        x: &'a Robj,
        idx: usize,
        rows: Option<Range<usize>>,
    ) -> (&'a str, Robj) {
        let pillar = &self.pillars[idx];
        (
            pillar.display_path.as_str(),
            get_value(x, &pillar.path, rows),
        )
    }
}

pub fn nrow(x: &Robj) -> usize {
    if x.is_frame() {
        x.get_attrib("row.names").unwrap().len()
    } else if x.is_matrix() || x.is_array() {
        x.dim().unwrap().as_robj().as_integer_vector().unwrap()[0] as usize
    } else {
        x.len()
    }
}
