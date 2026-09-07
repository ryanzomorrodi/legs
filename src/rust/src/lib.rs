use crate::{
    app::App,
    event::{Event, EventHandler},
    print::buffer_to_ansi_string,
    tui::Tui,
    update::{update, update_mouse},
};
use extendr_api::prelude::*;
use ratatui::{backend::CrosstermBackend, Terminal};
mod app;
mod col_layout;
mod event;
mod format;
mod help;
mod index_col;
mod movement;
mod print;
mod schema;
mod tui;
mod update;
mod viewer;
mod yank;
#[extendr]
fn visible_view(x: Robj) -> Result<Robj, Box<dyn std::error::Error>> {
    if !is_viewable(&x) {
        return Err(
            "object is not viewable: expected a data.frame, matrix, array, list, or vector".into(),
        );
    }
    let mut app = App::new(x)?;
    let backend = CrosstermBackend::new(std::io::stdout());
    let terminal = Terminal::new(backend)?;
    let events = EventHandler::new(250);
    let mut tui = Tui::new(terminal, events);
    tui.enter()?;
    let run_result = (|| -> Result<(), Box<dyn std::error::Error>> {
        while !app.should_quit {
            tui.draw(&mut app)?;
            if let Ok(event) = tui.events.next() {
                match event {
                    Event::Tick => {}
                    Event::Key(key_event) => update(&mut app, key_event)?,
                    Event::Mouse(mouse_event) => update_mouse(&mut app, mouse_event),
                    Event::Resize(_, _) => {}
                    Event::Error(msg) => return Err(msg.into()),
                };
            }
        }
        Ok(())
    })();
    tui.exit()?;
    run_result?;
    let last_frame = buffer_to_ansi_string(&tui.last_frame);
    rprintln!("{}", last_frame.trim_end_matches('\n'));
    Ok(app.view.data)
}
fn is_viewable(x: &Robj) -> bool {
    x.is_frame() || x.is_list() || x.is_vector() || x.is_matrix() || x.is_array()
}
extendr_module! {
    mod legs;
    fn visible_view;
}
