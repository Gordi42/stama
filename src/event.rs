use std::{
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use color_eyre::{eyre::eyre, Result};
use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, MouseEvent};

/// Minimum tick rate in milliseconds.
///
/// Tick rates are clamped to this value to prevent busy-spinning
/// when a tick rate of 0 (or another very small value) is requested.
const MIN_TICK_RATE_MS: u64 = 50;

/// Fixed timeout for polling terminal events.
///
/// Kept short (and independent of the tick rate) so that the event
/// thread notices a stop request quickly.
const POLL_TIMEOUT: Duration = Duration::from_millis(50);

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

/// A sender, a receiver, and the handle of the event thread.
///
/// The sender sends a boolean to the thread to stop it.
/// The receiver receives events from the thread.
/// The handle is used to join the thread when stopping it.
pub struct Communicator {
    sender: mpsc::Sender<bool>,
    receiver: mpsc::Receiver<Event>,
    handle: thread::JoinHandle<()>,
}

impl Communicator {
    pub fn new(
        sender: mpsc::Sender<bool>,
        receiver: mpsc::Receiver<Event>,
        handle: thread::JoinHandle<()>,
    ) -> Self {
        Self {
            sender,
            receiver,
            handle,
        }
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
            tick_rate: tick_rate.max(MIN_TICK_RATE_MS),
        }
    }

    pub fn set_tick_rate(&mut self, tick_rate: u64) {
        self.tick_rate = tick_rate.max(MIN_TICK_RATE_MS);
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
    /// Sends a signal to the event handling thread to break out of
    /// the loop, and waits for the thread to finish.
    pub fn stop(&mut self) {
        if let Some(communicator) = self.communicator.take() {
            let _ = communicator.sender.send(true);
            let _ = communicator.handle.join();
        }
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
        let handle = thread::spawn(move || {
            let mut last_tick = Instant::now();
            while stop_receiver.try_recv().is_err() {
                // Poll with a short fixed timeout so that stop requests
                // are noticed quickly, independent of the tick rate.
                let event_available = match event::poll(POLL_TIMEOUT) {
                    Ok(available) => available,
                    Err(_) => break,
                };

                if event_available {
                    let crossterm_event = match event::read() {
                        Ok(crossterm_event) => crossterm_event,
                        Err(_) => break,
                    };
                    let send_result = match crossterm_event {
                        CrosstermEvent::Key(e) => {
                            if e.kind == event::KeyEventKind::Press {
                                sender.send(Event::Key(e))
                            } else {
                                Ok(()) // ignore KeyEventKind::Release on windows
                            }
                        }
                        CrosstermEvent::Mouse(e) => sender.send(Event::Mouse(e)),
                        CrosstermEvent::Resize(w, h) => sender.send(Event::Resize(w, h)),
                        // ignore other events (FocusGained, FocusLost, Paste)
                        _ => Ok(()),
                    };
                    if send_result.is_err() {
                        break;
                    }
                }

                // Emit ticks based on accumulated elapsed time, so the
                // tick rate is independent of the poll timeout.
                if last_tick.elapsed() >= tick_rate {
                    let send_result = sender.send(Event::Tick);
                    if send_result.is_err() {
                        break;
                    }
                    last_tick = Instant::now();
                }
            }
        });
        let communicator = Communicator::new(stop_sender, receiver, handle);
        self.communicator = Some(communicator);
    }
}
