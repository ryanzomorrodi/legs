use super::columns::ColSearchState;
use crate::{data::format_col_values, view::View};
use extendr_api::Robj;
use ratatui::{
    style::Style,
    text::{Line, Span, Text},
};
use regex::Regex;
use std::{
    collections::{HashMap, VecDeque},
    fmt::Write as _,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Arc,
    },
    thread,
    time::{Duration, Instant},
};

pub const CHUNK_ROWS: usize = 500;

pub enum ChunkData {
    Text(Vec<String>),
    Int(Vec<i32>),
    Real(Vec<f64>),
    Lgl(Vec<u8>),
}

impl ChunkData {
    fn matches(&self, re: &Regex, base: usize) -> Vec<usize> {
        let mut out = Vec::new();
        let mut buf = String::new();

        match self {
            ChunkData::Text(v) => {
                for (i, s) in v.iter().enumerate() {
                    if re.is_match(s) {
                        out.push(base + i);
                    }
                }
            }
            ChunkData::Int(v) => {
                for (i, &x) in v.iter().enumerate() {
                    buf.clear();
                    if x == i32::MIN {
                        buf.push_str("NA");
                    } else {
                        let _ = write!(buf, "{x}");
                    }
                    if re.is_match(&buf) {
                        out.push(base + i);
                    }
                }
            }
            ChunkData::Real(v) => {
                for (i, &x) in v.iter().enumerate() {
                    buf.clear();
                    if x.is_nan() {
                        // In R's internal representation, NA_real_ is a NaN with lower 32-bits == 1954
                        if x.to_bits() & 0xFFFF_FFFF == 1954 {
                            buf.push_str("NA");
                        } else {
                            buf.push_str("NaN");
                        }
                    } else if x.is_infinite() {
                        buf.push_str(if x > 0.0 { "Inf" } else { "-Inf" });
                    } else {
                        let _ = write!(buf, "{x}");
                    }
                    if re.is_match(&buf) {
                        out.push(base + i);
                    }
                }
            }
            ChunkData::Lgl(v) => {
                for (i, &x) in v.iter().enumerate() {
                    let s = match x {
                        0 => "FALSE",
                        1 => "TRUE",
                        _ => "NA",
                    };
                    if re.is_match(s) {
                        out.push(base + i);
                    }
                }
            }
        }
        out
    }
}

pub struct ColumnCache {
    chunks: Vec<Option<Arc<ChunkData>>>,
}

#[derive(Default)]
pub struct Matches(pub Vec<usize>);

impl Matches {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn next_after(&self, cur: usize) -> Option<usize> {
        let i = self.0.partition_point(|&r| r <= cur);
        self.0.get(i).or_else(|| self.0.first()).copied()
    }

    pub fn prev_before(&self, cur: usize) -> Option<usize> {
        let i = self.0.partition_point(|&r| r < cur);
        if i > 0 {
            self.0.get(i - 1)
        } else {
            self.0.last()
        }
        .copied()
    }

    fn merge(&mut self, new: Vec<usize>) {
        if new.is_empty() {
            return;
        }
        let pos = self.0.partition_point(|&r| r < new[0]);
        self.0.splice(pos..pos, new);
    }
}

pub struct SearchState {
    pub query: String,
    pub regex: Option<Regex>,
    pub col_idx: usize,
    pub matches: Matches,
    pub generation: u64,
    pub anchor_row: usize,
    pub jumped: bool,
    pub total_chunks: usize,
    pub chunks_scanned: usize,
    pub pending: VecDeque<usize>,
}

impl SearchState {
    pub fn new(query: String, col_idx: usize) -> Self {
        Self {
            query,
            regex: None,
            col_idx,
            matches: Matches::default(),
            generation: 0,
            anchor_row: 0,
            jumped: false,
            total_chunks: 0,
            chunks_scanned: 0,
            pending: VecDeque::new(),
        }
    }

    pub fn scanning(&self) -> bool {
        !self.query.is_empty() && self.chunks_scanned < self.total_chunks
    }
}

pub enum Mode {
    Off,
    Rows(SearchState),
    Cols(ColSearchState),
}

pub enum Target {
    Rows,
    Cols,
}

struct ScanRequest {
    generation: u64,
    chunk_idx: usize,
    regex: Regex,
    rows: Arc<ChunkData>,
}

struct ScanResult {
    generation: u64,
    matches: Vec<usize>,
}

pub struct SearchWorker {
    tx: mpsc::Sender<ScanRequest>,
    rx: mpsc::Receiver<ScanResult>,
}

impl SearchWorker {
    pub fn spawn(current_gen: Arc<AtomicU64>) -> Self {
        let (req_tx, req_rx) = mpsc::channel::<ScanRequest>();
        let (res_tx, res_rx) = mpsc::channel::<ScanResult>();

        thread::spawn(move || {
            while let Ok(req) = req_rx.recv() {
                if req.generation != current_gen.load(Ordering::SeqCst) {
                    continue;
                }
                let base = req.chunk_idx * CHUNK_ROWS;
                let matches = req.rows.matches(&req.regex, base);
                if res_tx
                    .send(ScanResult {
                        generation: req.generation,
                        matches,
                    })
                    .is_err()
                {
                    break;
                }
            }
        });

        Self {
            tx: req_tx,
            rx: res_rx,
        }
    }
}

pub struct Search {
    pub mode: Mode,
    pub typing: bool,
    pub worker: SearchWorker,
    pub generation: Arc<AtomicU64>,
    pub cache: HashMap<usize, ColumnCache>,
    pub last_rows: Option<(String, usize)>,
}

impl Search {
    pub fn new() -> Self {
        let generation = Arc::new(AtomicU64::new(0));
        Self {
            mode: Mode::Off,
            typing: false,
            worker: SearchWorker::spawn(Arc::clone(&generation)),
            generation,
            cache: HashMap::new(),
            last_rows: None,
        }
    }

    pub fn active(&self) -> bool {
        !matches!(self.mode, Mode::Off)
    }

    pub fn rows(&self) -> Option<&SearchState> {
        match &self.mode {
            Mode::Rows(s) => Some(s),
            _ => None,
        }
    }

    pub fn rows_mut(&mut self) -> Option<&mut SearchState> {
        match &mut self.mode {
            Mode::Rows(s) => Some(s),
            _ => None,
        }
    }

    pub fn cols(&self) -> Option<&ColSearchState> {
        match &self.mode {
            Mode::Cols(s) => Some(s),
            _ => None,
        }
    }

    pub fn cols_mut(&mut self) -> Option<&mut ColSearchState> {
        match &mut self.mode {
            Mode::Cols(s) => Some(s),
            _ => None,
        }
    }
}

pub fn highlight_matches(
    text: &str,
    re: &Regex,
    base_style: Style,
    highlight_style: Style,
) -> Text<'static> {
    let mut spans = Vec::new();
    let mut last_end = 0;

    for m in re.find_iter(text) {
        if m.start() > last_end {
            spans.push(Span::styled(
                text[last_end..m.start()].to_string(),
                base_style,
            ));
        }
        spans.push(Span::styled(m.as_str().to_string(), highlight_style));
        last_end = m.end();
    }

    if last_end < text.len() {
        spans.push(Span::styled(text[last_end..].to_string(), base_style));
    }

    if spans.is_empty() {
        Text::styled(text.to_string(), base_style)
    } else {
        Text::from(Line::from(spans))
    }
}

fn n_chunks_for(nrow: usize) -> usize {
    (nrow + CHUNK_ROWS - 1) / CHUNK_ROWS
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.next() == Some('[') {
                for n in chars.by_ref() {
                    if ('@'..='~').contains(&n) {
                        break;
                    }
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn pillar_chunk(col: Robj, expected: usize) -> extendr_api::Result<ChunkData> {
    let raw = format_col_values(col)?;
    let skip = raw.len().saturating_sub(expected);
    Ok(ChunkData::Text(
        raw.iter()
            .skip(skip)
            .map(|s| strip_ansi(s).trim().to_string())
            .collect(),
    ))
}

fn chunk_to_data(col: Robj, expected: usize) -> extendr_api::Result<ChunkData> {
    use extendr_api::prelude::*;

    if col.class().is_none() {
        if let Some(s) = col.as_real_slice() {
            return Ok(ChunkData::Real(s.to_vec()));
        }
        if let Some(s) = col.as_integer_slice() {
            return Ok(ChunkData::Int(s.to_vec()));
        }
        if let Some(s) = col.as_logical_slice() {
            return Ok(ChunkData::Lgl(
                s.iter()
                    .map(|b| {
                        if b.is_na() {
                            2
                        } else if b.is_true() {
                            1
                        } else {
                            0
                        }
                    })
                    .collect(),
            ));
        }
        if let Some(v) = col.as_str_vector() {
            return Ok(ChunkData::Text(v.into_iter().map(str::to_owned).collect()));
        }
    }
    pillar_chunk(col, expected)
}

impl View {
    pub fn start_search(&mut self, target: Target) {
        let (_, col) = self.selected_cell();
        self.clear_search();
        self.search.mode = match target {
            Target::Rows => Mode::Rows(SearchState::new(String::new(), col)),
            Target::Cols => Mode::Cols(ColSearchState::new(col)),
        };
        self.search.typing = true;
    }

    pub fn push_search_char(&mut self, c: char) -> extendr_api::Result<()> {
        match &mut self.search.mode {
            Mode::Rows(s) => {
                if c == '/' && s.query.is_empty() {
                    if let Some((query, col)) = &self.search.last_rows {
                        s.query = query.clone();
                        s.col_idx = *col;
                        self.search.typing = false;
                        self.restart_scan();
                        return Ok(());
                    }
                }
                s.query.push(c);
                self.restart_scan();
            }
            Mode::Cols(s) => {
                s.query.push(c);
                self.refresh_col_matches();
            }
            Mode::Off => {}
        }
        Ok(())
    }

    pub fn pop_search_char(&mut self) -> extendr_api::Result<()> {
        match &mut self.search.mode {
            Mode::Rows(s) => {
                s.query.pop();
                self.restart_scan();
            }
            Mode::Cols(s) => {
                s.query.pop();
                self.refresh_col_matches();
            }
            Mode::Off => {}
        }
        Ok(())
    }

    pub fn clear_search(&mut self) {
        if let Mode::Rows(s) = &self.search.mode {
            if !s.query.is_empty() {
                self.search.last_rows = Some((s.query.clone(), s.col_idx));
            }
        }
        self.search.generation.fetch_add(1, Ordering::SeqCst);
        self.search.mode = Mode::Off;
        self.search.typing = false;
    }

    pub fn search_next(&mut self) -> extendr_api::Result<bool> {
        let (row, col) = self.selected_cell();
        match &self.search.mode {
            Mode::Rows(s) => match s.matches.next_after(row) {
                Some(r) => {
                    self.move_cursor_to_row(r);
                    Ok(true)
                }
                None => Ok(false),
            },
            Mode::Cols(s) => match s.matches.next_after(col) {
                Some(c) => {
                    self.move_cursor_to_col(c);
                    Ok(true)
                }
                None => Ok(false),
            },
            Mode::Off => Ok(false),
        }
    }

    pub fn search_prev(&mut self) -> extendr_api::Result<bool> {
        let (row, col) = self.selected_cell();
        match &self.search.mode {
            Mode::Rows(s) => match s.matches.prev_before(row) {
                Some(r) => {
                    self.move_cursor_to_row(r);
                    Ok(true)
                }
                None => Ok(false),
            },
            Mode::Cols(s) => match s.matches.prev_before(col) {
                Some(c) => {
                    self.move_cursor_to_col(c);
                    Ok(true)
                }
                None => Ok(false),
            },
            Mode::Off => Ok(false),
        }
    }

    fn get_or_format_chunk(
        &mut self,
        col_idx: usize,
        chunk_idx: usize,
    ) -> extendr_api::Result<Arc<ChunkData>> {
        let nrow = self.schema.nrow;
        let n_chunks = n_chunks_for(nrow);
        let cache = self
            .search
            .cache
            .entry(col_idx)
            .or_insert_with(|| ColumnCache {
                chunks: vec![None; n_chunks],
            });

        if let Some(c) = &cache.chunks[chunk_idx] {
            return Ok(Arc::clone(c));
        }

        let start = chunk_idx * CHUNK_ROWS;
        let end = (start + CHUNK_ROWS).min(nrow);

        let (_name, col_robj) = self
            .schema
            .column_at(&self.data, col_idx, Some(start..end))?;

        let data = Arc::new(chunk_to_data(col_robj, end - start)?);
        cache.chunks[chunk_idx] = Some(Arc::clone(&data));
        Ok(data)
    }

    fn restart_scan(&mut self) {
        let generation = self.search.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let anchor = self.selected_cell().0;
        let n_chunks = n_chunks_for(self.schema.nrow);

        let Some(s) = self.search.rows_mut() else {
            return;
        };
        s.generation = generation;
        s.anchor_row = anchor;
        s.jumped = false;
        s.matches.clear();
        s.pending.clear();
        s.chunks_scanned = 0;
        s.total_chunks = n_chunks;

        s.regex = if s.query.is_empty() {
            None
        } else {
            Regex::new(&s.query).ok()
        };

        if s.regex.is_none() || n_chunks == 0 {
            s.total_chunks = 0;
            return;
        }

        let first = anchor / CHUNK_ROWS;
        s.pending
            .extend((0..n_chunks).map(|i| (first + i) % n_chunks));
    }

    pub fn search_active(&self) -> bool {
        self.search.rows().is_some_and(|s| s.scanning())
    }

    pub fn pump_search(
        &mut self,
        budget: Duration,
        should_yield: &dyn Fn() -> bool,
    ) -> extendr_api::Result<bool> {
        while let Ok(res) = self.search.worker.rx.try_recv() {
            if let Some(s) = self.search.rows_mut() {
                if res.generation == s.generation {
                    s.chunks_scanned += 1;
                    s.matches.merge(res.matches);
                }
            }
        }

        self.maybe_jump_to_first_match();

        let deadline = Instant::now() + budget;
        while Instant::now() < deadline && !should_yield() {
            let (chunk_idx, col_idx, generation, regex) = match self.search.rows_mut() {
                Some(s) => match (&s.regex, s.pending.pop_front()) {
                    (Some(re), Some(c)) => (c, s.col_idx, s.generation, re.clone()),
                    _ => break,
                },
                None => break,
            };
            let rows = self.get_or_format_chunk(col_idx, chunk_idx)?;
            let _ = self.search.worker.tx.send(ScanRequest {
                generation,
                chunk_idx,
                regex,
                rows,
            });
        }

        Ok(self.search_active())
    }

    fn maybe_jump_to_first_match(&mut self) {
        let Some(s) = self.search.rows_mut() else {
            return;
        };
        if s.jumped || s.matches.is_empty() {
            return;
        }
        let idx = s.matches.0.partition_point(|&r| r < s.anchor_row);
        let target = match s.matches.0.get(idx) {
            Some(&r) => r,
            None if !s.scanning() => s.matches.0[0],
            None => return,
        };
        s.jumped = true;
        self.move_cursor_to_row(target);
    }
}
