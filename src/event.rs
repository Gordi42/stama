use std::{
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use color_eyre::{eyre::eyre, Result};
use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, MouseEvent};

/// Terminal events.
#[derive(Clone, Copy, Debug)]
pub enum Event {
    /// Terminal tick.
    Tick,
    /// Key press.
    Key(KeyEvent),
    /// Mouse click/scroll.
    Mouse(MouseEvent),
    /// Terminal resize.
    Resize(u16, u16),
}

/// A pair of a sender and a receiver
///
/// The sender send a boolean to a thread to stop it.
/// The receiver receives events from the thread.
pub struct Communicator {
    sender: mpsc::Sender<bool>,
    receiver: mpsc::Receiver<Event>,
}

impl Communicator {
    pub fn new(sender: mpsc::Sender<bool>, receiver: mpsc::Receiver<Event>) -> Self {
        Self { sender, receiver }
    }
}

/// Terminal event handler.
/// It spawns a new thread that waits for an event (tick, keystroke,
/// or mouse), and parses the event to the main program.
pub struct EventHandler {
    communicator: Option<Communicator>,
    tick_rate: u64,
}

impl EventHandler {
    pub fn new(tick_rate: u64) -> Self {
        Self {
            communicator: None,
            tick_rate,
        }
    }

    pub fn set_tick_rate(&mut self, tick_rate: u64) {
        self.tick_rate = tick_rate;
        self.stop();
        self.start();
    }

    /// Receive the next event from the handler thread.
    ///
    /// This function will always block the current thread if
    /// there is no data available and it's possible for more data to be sent.
    pub fn next(&self) -> Result<Event> {
        match &self.communicator {
            Some(communicator) => Ok(communicator.receiver.recv()?),
            None => Err(eyre!("event handler not active")),
        }
    }

    /// Stop the event handler thread
    ///
    /// Sends a signal to the event handling thread, to break out of
    /// the loop.
    pub fn stop(&mut self) {
        if let Some(communicator) = &self.communicator {
            communicator.sender.send(true).unwrap();
        }
        self.communicator = None;
    }

    /// Start the event handler thread.
    pub fn start(&mut self) {
        self.stop();
        let tick_rate = Duration::from_millis(self.tick_rate);
        // event pipeline
        let (sender, receiver) = mpsc::channel();
        // stop pipeline
        let (stop_sender, stop_receiver) = mpsc::channel::<bool>();
        // the sender of the event pipeline and the receiver of the stop
        // pipeline move to the thread
        thread::spawn(move || {
            let mut last_tick = Instant::now();
            while !stop_receiver.try_recv().is_ok() {
                let timeout = tick_rate
                    .checked_sub(last_tick.elapsed())
                    .unwrap_or(tick_rate);

                if event::poll(timeout).expect("unable to poll for event") {
                    let send_result = match event::read().expect("unable to read event") {
                        CrosstermEvent::Key(e) => {
                            if e.kind == event::KeyEventKind::Press {
                                sender.send(Event::Key(e))
                            } else {
                                Ok(()) // ignore KeyEventKind::Release on windows
                            }
                        }
                        CrosstermEvent::Mouse(e) => sender.send(Event::Mouse(e)),
                        CrosstermEvent::Resize(w, h) => sender.send(Event::Resize(w, h)),
                        _ => unimplemented!(),
                    };
                    if send_result.is_err() {
                        break;
                    }
                }

                if last_tick.elapsed() >= tick_rate {
                    let send_result = sender.send(Event::Tick);
                    if send_result.is_err() {
                        break;
                    }
                    last_tick = Instant::now();
                }
            }
        });
        let communicator = Communicator::new(stop_sender, receiver);
        self.communicator = Some(communicator);
    }
}
