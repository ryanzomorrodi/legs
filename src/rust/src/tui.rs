use extendr_api::Error;
use ratatui::{
    buffer::Buffer,
    crossterm::{
        event::{DisableMouseCapture, EnableMouseCapture},
        execute,
        terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
    },
    layout::Rect,
};
use std::{
    io::{self, Write},
    panic, thread,
    time::Duration,
};
pub type CrosstermTerminal = ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>;
use crate::{app::App, event::EventHandler};

pub struct Tui {
    terminal: CrosstermTerminal,
    pub events: EventHandler,
    pub last_frame: Buffer,
}

fn io_err(e: std::io::Error) -> Error {
    Error::Other(e.to_string())
}

impl Tui {
    pub fn new(terminal: CrosstermTerminal, events: EventHandler) -> Self {
        Self {
            terminal,
            events,
            last_frame: Buffer::empty(Rect::default()),
        }
    }

    pub fn enter(&mut self) -> extendr_api::Result<()> {
        terminal::enable_raw_mode().map_err(io_err)?;
        execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture).map_err(io_err)?;
        let panic_hook = panic::take_hook();
        panic::set_hook(Box::new(move |panic| {
            if let Err(e) = Self::reset() {
                eprintln!("failed to reset the terminal after panic: {e}");
            }
            panic_hook(panic);
        }));
        self.terminal.hide_cursor().map_err(io_err)?;
        self.terminal.clear().map_err(io_err)?;
        Ok(())
    }

    pub fn draw(&mut self, app: &mut App) -> extendr_api::Result<()> {
        let mut captured = None;
        let mut render_result = Ok(());
        self.terminal
            .draw(|frame| {
                render_result = app.view.render(frame);
                if render_result.is_ok() && app.show_help {
                    crate::help::render_help(frame);
                }
                captured = Some(frame.buffer_mut().clone());
            })
            .map_err(io_err)?;
        render_result?;
        if let Some(buf) = captured {
            self.last_frame = buf;
        }
        Ok(())
    }

    fn reset() -> Result<(), std::io::Error> {
        terminal::disable_raw_mode()?;
        execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;
        Ok(())
    }

    pub fn exit(&mut self) -> extendr_api::Result<()> {
        self.events.stop();
        thread::sleep(Duration::from_millis(50));
        Self::reset().map_err(io_err)?;
        self.terminal.show_cursor().map_err(io_err)?;
        let _ = io::stdout().flush();
        Ok(())
    }
}
