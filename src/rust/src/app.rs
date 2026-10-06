use crate::{is_viewable, view::View};
use extendr_api::prelude::*;

pub struct App {
    pub view: View,
    pub stack: Vec<View>,
    pub typed_num: Option<usize>,
    pub should_quit: bool,
    pub pending_c: bool,
}

impl App {
    pub fn new(x: Robj) -> extendr_api::Result<Self> {
        Ok(Self {
            view: View::new(x, String::new())?,
            stack: Vec::new(),
            typed_num: None,
            should_quit: false,
            pending_c: false,
        })
    }

    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    pub fn get_current_num(&self) -> Option<usize> {
        self.typed_num
    }

    pub fn push_cell_as_viewer(&mut self) -> extendr_api::Result<()> {
        let Some(value) = self.view.selected_value() else {
            return Ok(());
        };
        let value = value?;
        if !is_viewable(&value) {
            return Ok(());
        }
        if self.view.data.is_vector_atomic() && self.view.data.len() == 1 {
            return Ok(());
        }

        let child_prefix = self.view.path();
        let new_viewer = View::new(value, child_prefix)?;
        let old_viewer = std::mem::replace(&mut self.view, new_viewer);
        self.stack.push(old_viewer);
        Ok(())
    }

    pub fn pop_viewer(&mut self) {
        if let Some(viewer) = self.stack.pop() {
            self.view = viewer;
        }
    }
}
