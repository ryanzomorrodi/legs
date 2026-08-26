use crate::app::App;
use ratatui::crossterm::event::{KeyCode, KeyEvent};

pub fn update(app: &mut App, key_event: KeyEvent) {
    match key_event.code {
        KeyCode::Char('q') => app.quit(),
        KeyCode::Esc => {
            if !app.close_nested() {
                app.quit();
            }
        }
        KeyCode::Enter => app.open_selected_nested(),
        KeyCode::Char('g') => app.first_row(),
        KeyCode::Char('G') => {
            if let Some(current_num) = app.typed_num {
                app.go_to_idx(current_num.saturating_sub(1))
            } else {
                app.last_row();
            }
        }
        KeyCode::Char(c) if c.is_ascii_digit() => {
            let digit_typed = c.to_digit(10).unwrap() as usize;
            app.typed_num = if let Some(current_num) = app.typed_num {
                Some(current_num * 10 + digit_typed)
            } else {
                Some(digit_typed)
            };
            return;
        }
        KeyCode::Char('j') | KeyCode::Down => {
            if let Some(current_num) = app.typed_num {
                app.next_n_row(current_num);
            } else {
                app.next_row();
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if let Some(current_num) = app.typed_num {
                app.previous_n_row(current_num);
            } else {
                app.previous_row();
            }
        }
        KeyCode::Char('l') | KeyCode::Right => {
            if let Some(current_num) = app.typed_num {
                app.next_n_col(current_num);
            } else {
                app.next_col();
            }
        }
        KeyCode::Char('h') | KeyCode::Left => {
            if let Some(current_num) = app.typed_num {
                app.previous_n_col(current_num);
            } else {
                app.previous_col();
            }
        }
        KeyCode::Char('^') => app.first_column(),
        KeyCode::Char('$') => app.last_column(),
        _ => {}
    };
    app.typed_num = None;
}
