use crate::app::App;
use crate::view::Target;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};

pub fn update(app: &mut App, key_event: KeyEvent) -> extendr_api::Result<()> {
    if app.view.picker.is_some() {
        app.view.handle_picker_key(key_event);
        return Ok(());
    }

    if app.view.search.typing {
        match key_event.code {
            KeyCode::Enter => {
                app.view.search.typing = false;
            }
            KeyCode::Esc => {
                app.view.clear_search();
            }
            KeyCode::Backspace => {
                app.view.pop_search_char()?;
            }
            KeyCode::Char(c) => {
                app.view.push_search_char(c)?;
            }
            _ => {}
        }
        return Ok(());
    }

    if app.pending_c {
        app.pending_c = false;
          match key_event.code {
            KeyCode::Char('/') => {
                app.view.start_search(Target::Cols);
                app.typed_num = None;
                return Ok(());
            }
            KeyCode::Char('v') => {
                app.view.open_col_picker();
                app.typed_num = None;
                return Ok(());
            }
            _ => {}
        }
    }

    match key_event.code {
        KeyCode::Char('q') => app.quit(),
        KeyCode::Char('c') => {
            app.pending_c = true;
            return Ok(());
        }
        KeyCode::Char('/') => {
            app.view.start_search(Target::Rows);
            return Ok(());
        }
        KeyCode::Char('d') => {
            app.view.clear_search();
        }
        KeyCode::Char('n') => {
            let count = app.get_current_num().unwrap_or(1);
            for _ in 0..count {
                if !app.view.search_next()? {
                    break;
                }
            }
        }
        KeyCode::Char('N') => {
            let count = app.get_current_num().unwrap_or(1);
            for _ in 0..count {
                if !app.view.search_prev()? {
                    break;
                }
            }
        }
        KeyCode::Char(c) if c.is_ascii_digit() => {
            let d = c.to_digit(10).unwrap_or(0) as usize;
            app.typed_num = Some(
                app.typed_num
                    .map_or(d, |n| n.saturating_mul(10).saturating_add(d)),
            );
            return Ok(());
        }

        KeyCode::Char('j') | KeyCode::Down if key_event.modifiers.contains(KeyModifiers::NONE) => {
            let n = app.get_current_num().unwrap_or(1);
            app.view.move_cursor_down(n);
        }
        KeyCode::Char('k') | KeyCode::Up if key_event.modifiers.contains(KeyModifiers::NONE) => {
            let n = app.get_current_num().unwrap_or(1);
            app.view.move_cursor_up(n);
        }
        KeyCode::Char('l') | KeyCode::Right if key_event.modifiers.contains(KeyModifiers::NONE) => {
            let n = app.get_current_num().unwrap_or(1);
            app.view.move_cursor_right(n);
        }
        KeyCode::Char('h') | KeyCode::Left if key_event.modifiers.contains(KeyModifiers::NONE) => {
            let n = app.get_current_num().unwrap_or(1);
            app.view.move_cursor_left(n);
        }

        KeyCode::Char('J') if key_event.modifiers.contains(KeyModifiers::SHIFT) => {
            let n = app.get_current_num().unwrap_or(1);
            app.view.scroll_window_down(n);
        }
        KeyCode::Char('K') if key_event.modifiers.contains(KeyModifiers::SHIFT) => {
            let n = app.get_current_num().unwrap_or(1);
            app.view.scroll_window_up(n);
        }
        KeyCode::Char('L') if key_event.modifiers.contains(KeyModifiers::SHIFT) => {
            let n = app.get_current_num().unwrap_or(1);
            app.view.scroll_window_right(n);
        }
        KeyCode::Char('H') if key_event.modifiers.contains(KeyModifiers::SHIFT) => {
            let n = app.get_current_num().unwrap_or(1);
            app.view.scroll_window_left(n);
        }

        KeyCode::Char('^') => app.view.move_cursor_to_first_col(),
        KeyCode::Char('$') => app.view.move_cursor_to_last_col(),
        KeyCode::Char('g') => app.view.move_cursor_to_first_row(),
        KeyCode::Char('G') => {
            if let Some(n) = app.get_current_num() {
                app.view.move_cursor_to_row(n.saturating_sub(1));
            } else {
                app.view.move_cursor_to_last_row();
            }
        }

        KeyCode::Char('t') => {
            if let Some(n) = app.get_current_num().filter(|&n| n >= 30) {
                app.view.truncate_size = n;
                app.view.truncate = true;
            } else {
                app.view.toggle_truncate();
            }
        }
        KeyCode::Char('y') => app.view.yank()?,
        KeyCode::Enter => app.push_cell_as_viewer()?,
        KeyCode::Esc | KeyCode::Backspace => app.pop_viewer(),
        _ => {}
    };

    app.typed_num = None;
    Ok(())
}

pub fn update_mouse(app: &mut App, event: MouseEvent) {
    match event.kind {
        MouseEventKind::ScrollDown if event.modifiers.contains(KeyModifiers::SHIFT) => {
            app.view.scroll_right(1);
        }
        MouseEventKind::ScrollUp if event.modifiers.contains(KeyModifiers::SHIFT) => {
            app.view.scroll_left(1);
        }
        MouseEventKind::ScrollUp => app.view.scroll_up(1),
        MouseEventKind::ScrollDown => app.view.scroll_down(1),
        _ => {}
    }
}
