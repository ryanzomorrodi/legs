use crate::app::App;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub fn update(app: &mut App, key_event: KeyEvent) {
    match key_event.code {
        KeyCode::Char('q') => app.quit(),
        KeyCode::Esc => {
            app.close_nested();
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
            app.typed_num = Some(
                app.typed_num
                    .map_or(d, |n| n.saturating_mul(10).saturating_add(d)),
            );
            return;
        }
        KeyCode::Char('j') | KeyCode::Down if key_event.modifiers == KeyModifiers::NONE => {
            let n_row = app.get_current_num();
            app.view.next_n_row(n_row);
        }
        KeyCode::Char('k') | KeyCode::Up if key_event.modifiers == KeyModifiers::NONE => {
            let n_row = app.get_current_num();
            app.view.previous_n_row(n_row);
        }
        KeyCode::Char('l') | KeyCode::Right if key_event.modifiers == KeyModifiers::NONE => {
            let n_col = app.get_current_num();
            app.view.next_n_col(n_col);
        }
        KeyCode::Char('h') | KeyCode::Left if key_event.modifiers == KeyModifiers::NONE => {
            let n_col = app.get_current_num();
            app.view.previous_n_col(n_col);
        }
        KeyCode::Char('J') | KeyCode::Down if key_event.modifiers == KeyModifiers::SHIFT => {
            let n_row = app.get_current_num().saturating_mul(app.view.visible_n_row);
            app.view.next_n_row(n_row)
        }
        KeyCode::Char('K') | KeyCode::Up if key_event.modifiers == KeyModifiers::SHIFT => {
            let n_row = app.get_current_num().saturating_mul(app.view.visible_n_row);
            app.view.previous_n_row(n_row)
        }
        KeyCode::Char('L') | KeyCode::Right if key_event.modifiers == KeyModifiers::SHIFT => {
            let n_row = app.get_current_num().saturating_mul(app.view.visible_n_col);
            app.view.next_n_col(n_row)
        }
        KeyCode::Char('H') | KeyCode::Left if key_event.modifiers == KeyModifiers::SHIFT => {
            let n_row = app.get_current_num().saturating_mul(app.view.visible_n_col);
            app.view.previous_n_col(n_row)
        }
        KeyCode::Char('^') => app.view.first_column(),
        KeyCode::Char('$') => app.view.last_column(),
        KeyCode::Char('t') => app.view.toggle_truncate(),
        _ => {}
    };
    app.typed_num = None;
}
