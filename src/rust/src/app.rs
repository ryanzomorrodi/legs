use crate::r_types::UnsupportedTypeError;
use crate::viewer::Viewer;
use extendr_api::prelude::*;

pub struct App {
    pub view: Viewer,
    pub stack: Vec<Viewer>,
    pub typed_num: Option<usize>,
    pub should_quit: bool,
}

impl App {
    pub fn new(x: Robj) -> Result<Self, UnsupportedTypeError> {
        Ok(Self {
            view: Viewer::new(x)?,
            stack: Vec::new(),
            typed_num: None,
            should_quit: false,
        })
    }

    fn get_cell_robj(&self, row: usize, col: usize) -> Option<Robj> {
        self.view.data.index(row, col)
    }

    pub fn open_nested(&mut self) {
        let (row, col) = self.view.selected_cell();
        let Some(cell_robj) = self.get_cell_robj(row, col) else {
            return;
        };
        let Ok(new_view) = Viewer::new(cell_robj) else {
            return;
        };
        let old_view = std::mem::replace(&mut self.view, new_view);
        self.stack.push(old_view);
    }

    pub fn close_nested(&mut self) -> bool {
        if let Some(previous_view) = self.stack.pop() {
            self.view = previous_view;
            true
        } else {
            false
        }
    }

    pub fn quit(&mut self) {
        self.should_quit = true;
    }
}
