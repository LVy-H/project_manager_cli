use crate::engine::stats::WorkspaceStats;
use crate::engine::status::RepoStatus;
use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, MouseEvent};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// Terminal events.
#[derive(Debug)]
pub enum Event {
    /// Terminal tick.
    Tick,
    /// Key press.
    Key(KeyEvent),
    /// Mouse click/scroll.
    Mouse(MouseEvent),
    /// Window resize.
    Resize(u16, u16),
    /// Background data loaded
    Data(DataEvent),
}

#[derive(Debug)]
pub enum DataEvent {
    Stats(WorkspaceStats),
    GitStatus(Vec<RepoStatus>),
}

#[derive(Debug)]
pub struct EventHandler {
    pub sender: mpsc::Sender<Event>,
    receiver: mpsc::Receiver<Event>,
    // handler_thread: thread::JoinHandle<()>, // Kept if we need to join later
}

impl EventHandler {
    pub fn new(tick_rate: u64) -> Self {
        let (sender, receiver) = mpsc::channel();
        let tick_rate = Duration::from_millis(tick_rate);

        let sender_clone = sender.clone();

        let _handler_thread = thread::spawn(move || {
            let mut last_tick = Instant::now();
            loop {
                let timeout = tick_rate
                    .checked_sub(last_tick.elapsed())
                    .unwrap_or_else(|| Duration::from_secs(0));

                if event::poll(timeout).expect("failed to poll new events") {
                    match event::read().expect("unable to read event") {
                        CrosstermEvent::Key(e) => sender_clone.send(Event::Key(e)),
                        CrosstermEvent::Mouse(e) => sender_clone.send(Event::Mouse(e)),
                        CrosstermEvent::Resize(w, h) => sender_clone.send(Event::Resize(w, h)),
                        _ => Ok(()),
                    }
                    .expect("failed to send terminal event")
                }

                if last_tick.elapsed() >= tick_rate {
                    sender_clone
                        .send(Event::Tick)
                        .expect("failed to send tick event");
                    last_tick = Instant::now();
                }
            }
        });

        Self { sender, receiver }
    }

    pub fn next(&self) -> Result<Event, mpsc::RecvError> {
        self.receiver.recv()
    }
}
