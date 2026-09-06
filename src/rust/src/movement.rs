use crate::viewer::Viewer;

impl Viewer {
    pub fn move_cursor_to_first_col(&mut self) {
        if self.schema.is_empty() {
            return;
        }
        let (row, _) = self.selected_cell();
        let new_col = 0;
        self.state.select_cell(Some((row, new_col)));
        self.col_start_idx = new_col;
        self.col_start_from_start = true;
    }

    pub fn move_cursor_to_last_col(&mut self) {
        if self.schema.is_empty() {
            return;
        }
        let (row, _) = self.selected_cell();
        let new_col = self.schema.len();
        self.state.select_cell(Some((row, new_col)));
        self.col_end_idx = new_col;
        self.col_start_from_start = false;
    }

    pub fn move_cursor_right(&mut self, n: usize) {
        if self.schema.is_empty() {
            return;
        }
        let (row, col) = self.selected_cell();
        let mut new_col = col.saturating_add(n);
        if new_col >= self.schema.len() {
            new_col = self.schema.len() - 1
        }
        self.state.select_cell(Some((row, new_col)));
        if new_col > self.col_end_idx {
            self.col_end_idx = new_col;
            self.col_start_from_start = false;
        }
    }

    pub fn move_cursor_left(&mut self, n: usize) {
        if self.schema.is_empty() {
            return;
        }
        let (row, col) = self.selected_cell();
        let new_col = col.saturating_sub(n);
        self.state.select_cell(Some((row, new_col)));
        if new_col < self.col_start_idx {
            self.col_start_idx = new_col;
            self.col_start_from_start = true;
        }
    }

    pub fn move_cursor_to_first_row(&mut self) {
        if self.schema.is_empty() {
            return;
        }
        let (_, col) = self.selected_cell();
        let new_row = 0;
        self.state.select_cell(Some((new_row, col)));
    }

    pub fn move_cursor_to_last_row(&mut self) {
        if self.schema.is_empty() {
            return;
        }
        let (_, col) = self.selected_cell();
        let new_row = self.schema.nrow.saturating_sub(1);
        self.state.select_cell(Some((new_row, col)));
    }

    pub fn move_cursor_up(&mut self, n: usize) {
        if self.schema.is_empty() {
            return;
        }
        let (row, col) = self.selected_cell();
        let new_row = row.saturating_sub(n);
        self.state.select_cell(Some((new_row, col)));
    }

    pub fn move_cursor_down(&mut self, n: usize) {
        if self.schema.is_empty() {
            return;
        }
        let (row, col) = self.selected_cell();
        let mut new_row = row.saturating_add(n);
        if new_row >= self.schema.nrow {
            new_row = self.schema.nrow - 1
        }
        self.state.select_cell(Some((new_row, col)));
    }

    pub fn scroll_up(&mut self, n: usize) {
        let (row, col) = self.selected_cell();
        let offset_ref = self.state.offset_mut();
        let new_offset = offset_ref.saturating_sub(n);
        let new_end = (new_offset + self.visible_n_row.saturating_sub(1)).min(self.schema.nrow);

        *offset_ref = new_offset;
        if row > new_end {
            self.state.select_cell(Some((new_end, col)));
        }
    }

    pub fn scroll_down(&mut self, n: usize) {
        let (row, col) = self.selected_cell();
        let offset_ref = self.state.offset_mut();
        let new_offset = offset_ref
            .saturating_add(n)
            .min(self.schema.nrow.saturating_sub(self.visible_n_row));

        *offset_ref = new_offset;
        if row < new_offset {
            self.state.select_cell(Some((new_offset, col)));
        }
    }

    pub fn scroll_left(&mut self, n: usize) {
        let new_col = self.col_start_idx.saturating_sub(n);
        self.col_start_idx = new_col;
        self.col_start_from_start = true;
    }

    pub fn scroll_right(&mut self, n: usize) {
        let mut new_col = self.col_end_idx.saturating_add(n);
        if new_col >= self.schema.len() {
            new_col = self.schema.len() - 1
        }
        self.col_end_idx = new_col;
        self.col_start_from_start = false;
    }

    pub fn scroll_window_up(&mut self, n: usize) {
        let move_rows = self.visible_n_row.saturating_mul(n);

        let (row, col) = self.selected_cell();
        let offset_ref = self.state.offset_mut();
        let offset_from_offset = row - *offset_ref;
        let new_offset = offset_ref.saturating_sub(move_rows);
        *offset_ref = new_offset;

        self.state
            .select_cell(Some((new_offset + offset_from_offset, col)));
    }

    pub fn scroll_window_down(&mut self, n: usize) {
        let move_rows = self.visible_n_row.saturating_mul(n);

        let (row, col) = self.selected_cell();
        let offset_ref = self.state.offset_mut();
        let offset_from_offset = row - *offset_ref;
        let new_offset = offset_ref
            .saturating_add(move_rows)
            .min(self.schema.nrow.saturating_sub(self.visible_n_row));
        *offset_ref = new_offset;

        self.state
            .select_cell(Some((new_offset + offset_from_offset, col)));
    }

    pub fn scroll_window_left(&mut self, n: usize) {
        let move_cols = self.visible_n_col.saturating_mul(n);
        let (row, col) = self.selected_cell();
        let offset_from_start = col - self.col_start_idx;
        let new_col = self.col_start_idx.saturating_sub(move_cols);
        self.col_start_idx = new_col;
        self.col_start_from_start = true;

        self.state
            .select_cell(Some((row, new_col + offset_from_start)));
    }

    pub fn scroll_window_right(&mut self, n: usize) {
        let move_cols = self.visible_n_col.saturating_mul(n);
        let (row, col) = self.selected_cell();
        let offset_from_end = self.col_end_idx - col;
        let mut new_col = self.col_end_idx.saturating_add(move_cols);
        if new_col >= self.schema.len() {
            new_col = self.schema.len() - 1
        }
        self.col_end_idx = new_col;
        self.col_start_from_start = false;

        self.state
            .select_cell(Some((row, new_col - offset_from_end)));
    }
}
