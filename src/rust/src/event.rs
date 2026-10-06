use ratatui::crossterm::event::{self, Event as CrosstermEvent, KeyEvent, MouseEvent};
use std::{
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
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
    pub input_pending: Arc<AtomicUsize>,
}

impl EventHandler {
    pub fn new(tick_rate: u64) -> Self {
        let tick_rate = Duration::from_millis(tick_rate);
        let (sender, receiver) = mpsc::channel();
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = Arc::clone(&running);
        let input_pending = Arc::new(AtomicUsize::new(0));
        let pending_clone = Arc::clone(&input_pending);
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
                            if e.kind == event::KeyEventKind::Press {
                                pending_clone.fetch_add(1, Ordering::SeqCst);
                                if sender.send(Event::Key(e)).is_err() {
                                    break;
                                }
                            }
                        }
                        Ok(CrosstermEvent::Mouse(e)) => {
                            pending_clone.fetch_add(1, Ordering::SeqCst);
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
            input_pending,
        }
    }

    pub fn next(&self, timeout: Duration) -> Result<Event, mpsc::RecvTimeoutError> {
        let ev = self.receiver.recv_timeout(timeout)?;
        if matches!(ev, Event::Key(_) | Event::Mouse(_)) {
            self.input_pending.fetch_sub(1, Ordering::SeqCst);
        }
        Ok(ev)
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        while self.receiver.try_recv().is_ok() {}
        self.input_pending.store(0, Ordering::SeqCst);
        if let Some(handle) = self.handler.take() {
            let _ = handle.join();
        }
    }
}
