use crate::app::App;
use crate::event::{Event, EventHandler};
use crate::print::buffer_to_ansi_string;
use crate::tui::Tui;
use crate::update::update;
use extendr_api::prelude::*;
use ratatui::{backend::CrosstermBackend, Terminal};

mod app;
mod event;
mod print;
mod r_types;
mod tui;
mod update;
mod viewer;

#[extendr]
fn visible_view(x: Robj) -> Result<Robj, Box<dyn std::error::Error>> {
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

    let last_frame = buffer_to_ansi_string(&tui.last_frame);
    rprintln!("{}", last_frame.trim_end_matches('\n'));
    let last_obj = app.view.data.robj().clone();

    Ok(last_obj)
}

extendr_module! {
    mod legs;
    fn visible_view;
}
