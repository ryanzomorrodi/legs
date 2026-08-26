use extendr_api::prelude::*;
use ratatui::widgets::TableState;

pub struct FrameContext {
    df: Dataframe<Robj>,
    n_rows: usize,
    n_cols: usize,
    state: TableState,
    col_start_idx: usize,
    last_col_visible: bool,
}

pub struct App {
    pub n_rows: usize,
    pub n_cols: usize,
    pub df: Dataframe<Robj>,
    pub state: TableState,
    pub typed_num: Option<usize>,
    pub col_start_idx: usize,
    pub last_col_visible: bool,
    pub should_quit: bool,
    pub stack: Vec<FrameContext>,
}

impl App {
    pub fn new(df: Dataframe<Robj>) -> Self {
        let col_names: Vec<String> = df
            .as_list()
            .unwrap()
            .iter()
            .map(|(name, _)| name.to_owned())
            .collect();
        let n_rows = df.get_attrib("row.names").unwrap().len();
        let n_cols = col_names.len();
        let initial_cell = if n_rows > 0 && n_cols > 0 {
            Some((0, 0))
        } else {
            None
        };
        Self {
            n_rows,
            n_cols,
            df,
            state: TableState::default().with_selected_cell(initial_cell),
            typed_num: None,
            col_start_idx: 0,
            last_col_visible: n_cols == 0,
            should_quit: false,
            stack: Vec::new(),
        }
    }

    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    fn selected_cell(&self) -> (usize, usize) {
        self.state.selected_cell().unwrap_or((0, 0))
    }

    fn nested_df_at(&self, row: usize, col: usize) -> Option<Dataframe<Robj>> {
        let (_, col_robj) = self.df.as_list()?.into_iter().nth(col)?;

        if col_robj.inherits("data.frame") {
            let row_idx = std::iter::once((row + 1) as i32).collect::<Integers>();
            let sub = col_robj.slice(row_idx).ok()?;
            return Dataframe::try_from(sub).ok();
        }

        if col_robj.is_list() {
            let list = col_robj.as_list()?;
            let cell = list.into_iter().nth(row).map(|(_, v)| v)?;
            if cell.inherits("data.frame") {
                return Dataframe::try_from(cell).ok();
            }
        }

        None
    }

    pub fn open_selected_nested(&mut self) {
        let (row, col) = self.selected_cell();
        let Some(nested_df) = self.nested_df_at(row, col) else {
            return;
        };

        let n_rows = nested_df
            .get_attrib("row.names")
            .map(|r| r.len())
            .unwrap_or(0);
        let n_cols = nested_df.as_list().map(|l| l.iter().count()).unwrap_or(0);
        let initial_cell = if n_rows > 0 && n_cols > 0 {
            Some((0, 0))
        } else {
            None
        };

        let old_df = std::mem::replace(&mut self.df, nested_df);
        let old_state = std::mem::replace(
            &mut self.state,
            TableState::default().with_selected_cell(initial_cell),
        );
        let old_col_start_idx = std::mem::replace(&mut self.col_start_idx, 0);
        let old_last_col_visible = std::mem::replace(&mut self.last_col_visible, n_cols == 0);
        let old_n_rows = std::mem::replace(&mut self.n_rows, n_rows);
        let old_n_cols = std::mem::replace(&mut self.n_cols, n_cols);

        self.stack.push(FrameContext {
            df: old_df,
            n_rows: old_n_rows,
            n_cols: old_n_cols,
            state: old_state,
            col_start_idx: old_col_start_idx,
            last_col_visible: old_last_col_visible,
        });
    }

    pub fn close_nested(&mut self) -> bool {
        let Some(ctx) = self.stack.pop() else {
            return false;
        };
        self.df = ctx.df;
        self.n_rows = ctx.n_rows;
        self.n_cols = ctx.n_cols;
        self.state = ctx.state;
        self.col_start_idx = ctx.col_start_idx;
        self.last_col_visible = ctx.last_col_visible;
        true
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

    pub fn go_to_idx(&mut self, idx: usize) {
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
}
