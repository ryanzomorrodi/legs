use crate::{
    app::App,
    event::{Event, EventHandler},
    tui::{buffer_to_ansi_string, Tui},
    update::{update, update_mouse},
};
use extendr_api::prelude::*;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{
    sync::{atomic::Ordering, Arc},
    time::Duration,
};

mod app;
mod data;
mod event;
mod tui;
mod update;
mod view;

/// @title Invoke legs Data Viewer
/// @description Invoke the legs terminal user interface (tui) to interactively explore R data.
/// @param x A data.frame, matrix, list, or atomic vector
/// @return The last viewed item
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
#[extendr(invisible)]
fn view(x: Robj) -> Result<Robj, Box<dyn std::error::Error>> {
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

            let scanning = app.view.search_active();
            let timeout = if scanning {
                Duration::from_millis(1)
            } else {
                Duration::from_millis(100)
            };

            match tui.events.next(timeout) {
                Ok(Event::Tick) => {}
                Ok(Event::Key(key_event)) => update(&mut app, key_event)?,
                Ok(Event::Mouse(mouse_event)) => update_mouse(&mut app, mouse_event),
                Ok(Event::Resize(_, _)) => {}
                Ok(Event::Error(msg)) => return Err(msg.into()),
                Err(_) => {}
            }

            if app.view.search_active() {
                let pending = Arc::clone(&tui.events.input_pending);
                app.view.pump_search(Duration::from_millis(8), &|| {
                    pending.load(Ordering::SeqCst) > 0
                })?;
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
    fn view;
}
