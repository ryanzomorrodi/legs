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
        KeyCode::Enter => {
            app.open_nested();
        }
        KeyCode::Char('g') => app.view.first_row(),
        KeyCode::Char('G') => match app.typed_num {
            Some(n) => app.view.go_to_row(n.saturating_sub(1)),
            None => app.view.last_row(),
        },
        KeyCode::Char(c) if c.is_ascii_digit() => {
            let d = c.to_digit(10).unwrap() as usize;
            app.typed_num = Some(app.typed_num.map_or(d, |n| n * 10 + d));
            return;
        }
        KeyCode::Char('j') | KeyCode::Down => match app.typed_num {
            Some(n) => app.view.next_n_row(n),
            None => app.view.next_row(),
        },
        KeyCode::Char('k') | KeyCode::Up => {
            if let Some(current_num) = app.typed_num {
                app.view.previous_n_row(current_num);
            } else {
                app.view.previous_row();
            }
        }
        KeyCode::Char('l') | KeyCode::Right => {
            if let Some(current_num) = app.typed_num {
                app.view.next_n_col(current_num);
            } else {
                app.view.next_col();
            }
        }
        KeyCode::Char('h') | KeyCode::Left => {
            if let Some(current_num) = app.typed_num {
                app.view.previous_n_col(current_num);
            } else {
                app.view.previous_col();
            }
        }
        KeyCode::Char('^') => app.view.first_column(),
        KeyCode::Char('$') => app.view.last_column(),
        KeyCode::Char('t') => app.view.toggle_truncate(),
        _ => {}
    };
    app.typed_num = None;
}
