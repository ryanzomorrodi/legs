use ratatui::crossterm::event::{self, Event as CrosstermEvent, KeyEvent, MouseEvent};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Debug)]
pub enum Event {
    Tick,
    Key(KeyEvent),
    #[allow(dead_code)]
    Mouse(MouseEvent),
    #[allow(dead_code)]
    Resize(u16, u16),
    Error(String),
}

#[derive(Debug)]
pub struct EventHandler {
    receiver: mpsc::Receiver<Event>,
    running: Arc<AtomicBool>,
    handler: Option<thread::JoinHandle<()>>,
}

impl EventHandler {
    pub fn new(tick_rate: u64) -> Self {
        let tick_rate = Duration::from_millis(tick_rate);
        let (sender, receiver) = mpsc::channel();
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = Arc::clone(&running);
        let handler = thread::spawn(move || {
            let mut last_tick = Instant::now();
            while running_clone.load(Ordering::Relaxed) {
                let timeout = tick_rate
                    .checked_sub(last_tick.elapsed())
                    .unwrap_or(tick_rate)
                    .min(Duration::from_millis(50));

                match event::poll(timeout) {
                    Ok(true) => match event::read() {
                        Ok(CrosstermEvent::Key(e)) => {
                            if e.kind == event::KeyEventKind::Press
                                && sender.send(Event::Key(e)).is_err()
                            {
                                break;
                            }
                        }
                        Ok(CrosstermEvent::Mouse(e)) => {
                            if sender.send(Event::Mouse(e)).is_err() {
                                break;
                            }
                        }
                        Ok(CrosstermEvent::Resize(w, h)) => {
                            if sender.send(Event::Resize(w, h)).is_err() {
                                break;
                            }
                        }
                        Ok(_) => {}
                        Err(e) => {
                            let _ = sender
                                .send(Event::Error(format!("unable to read terminal event: {e}")));
                            break;
                        }
                    },
                    Ok(false) => {}
                    Err(e) => {
                        let _ = sender.send(Event::Error(format!(
                            "unable to poll for terminal event: {e}"
                        )));
                        break;
                    }
                }

                if last_tick.elapsed() >= tick_rate {
                    if sender.send(Event::Tick).is_err() {
                        break;
                    }
                    last_tick = Instant::now();
                }
            }
        });
        Self {
            receiver,
            running,
            handler: Some(handler),
        }
    }

    pub fn next(&self) -> Result<Event, mpsc::RecvTimeoutError> {
        self.receiver.recv_timeout(Duration::from_millis(100))
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        while self.receiver.try_recv().is_ok() {}
        if let Some(handle) = self.handler.take() {
            let _ = handle.join();
        }
    }
}
