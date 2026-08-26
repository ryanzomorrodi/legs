use crate::app::App;
use crate::event::{Event, EventHandler};
use crate::tui::Tui;
use crate::update::update;
use extendr_api::prelude::*;
use ratatui::{backend::CrosstermBackend, Terminal};

mod app;
mod event;
mod tui;
mod ui;
mod update;

/// Render a dataframe inside a full terminal view.
/// @export
#[extendr]
fn view_df(df: Dataframe<Robj>) -> Result<(), Box<dyn std::error::Error>> {
    let mut app = App::new(df);

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
    fn view_df;
}
