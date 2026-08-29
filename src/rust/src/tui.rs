use color_eyre::Result;
use ratatui::{
    buffer::Buffer,
    crossterm::{
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

impl Tui {
    pub fn new(terminal: CrosstermTerminal, events: EventHandler) -> Self {
        Self {
            terminal,
            events,
            last_frame: Buffer::empty(Rect::default()),
        }
    }
    pub fn enter(&mut self) -> Result<()> {
        terminal::enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen)?;
        let panic_hook = panic::take_hook();
        panic::set_hook(Box::new(move |panic| {
            Self::reset().expect("failed to reset the terminal");
            panic_hook(panic);
        }));
        self.terminal.hide_cursor()?;
        self.terminal.clear()?;
        Ok(())
    }
    pub fn draw(&mut self, app: &mut App) -> Result<()> {
        let mut captured = None;
        self.terminal.draw(|frame| {
            app.view.render(frame);
            captured = Some(frame.buffer_mut().clone());
        })?;
        if let Some(buf) = captured {
            self.last_frame = buf;
        }
        Ok(())
    }
    fn reset() -> Result<()> {
        terminal::disable_raw_mode()?;
        execute!(io::stdout(), LeaveAlternateScreen)?;
        Ok(())
    }
    pub fn exit(&mut self) -> Result<()> {
        self.events.stop();
        thread::sleep(Duration::from_millis(50));
        Self::reset()?;
        self.terminal.show_cursor()?;
        let _ = io::stdout().flush();
        Ok(())
    }
}
