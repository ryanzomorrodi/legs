use crate::app::App;
use crate::event::{Event, EventHandler};
use crate::tui::Tui;
use crate::update::update;
use extendr_api::prelude::*;
use ratatui::{backend::CrosstermBackend, Terminal};

mod app;
mod event;
mod r_types;
mod tui;
mod update;
mod viewer;

/// @title Invoke legs Data Viewer
/// @description Invoke the legs terminal user interface (tui) to interactively explore R data.
/// @param x A data.frame, matrix, list, or atomic vector
/// @return No return value.
/// @examples
/// if (interactive()) {
///   df <- data.frame(x = 1:10, y = LETTERS[1:10])
///   view(df)
///   view(as.matrix(df))
///   view(as.list(df))
///   view(df$x)
/// }
///
/// @export
#[extendr]
fn view(x: Robj) -> Result<(), Box<dyn std::error::Error>> {
    let mut app = App::new(x)?;
    let backend = CrosstermBackend::new(std::io::stdout());
    let terminal = Terminal::new(backend)?;
    let events = EventHandler::new(250);
    let mut tui = Tui::new(terminal, events);
    tui.enter()?;
    while !app.should_quit {
        tui.draw(&mut app)?;
        if let Ok(event) = tui.events.next() {
            match event {
                Event::Tick => {}
                Event::Key(key_event) => update(&mut app, key_event),
                Event::Mouse(_) => {}
                Event::Resize(_, _) => {}
            };
        }
    }
    tui.exit()?;
    Ok(())
}

extendr_module! {
    mod legs;
    fn view;
}
