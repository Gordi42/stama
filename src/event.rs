use std::{
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};
use stoppable_thread;

use color_eyre::Result;
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

/// Terminal event handler.
pub struct EventHandler {
    /// Event sender channel.
    #[allow(dead_code)]
    sender: mpsc::Sender<Event>,
    /// Event receiver channel.
    receiver: mpsc::Receiver<Event>,
    // #[allow(dead_code)]
    handler: stoppable_thread::StoppableHandle<()>,
}

impl EventHandler {
    /// Constructs a new instance of [`EventHandler`].
    pub fn new(tick_rate: u64) -> Self {
        let tick_rate = Duration::from_millis(tick_rate);
        let (sender, receiver) = mpsc::channel();
        let handler = {
            let sender = sender.clone();
            stoppable_thread::spawn(move |stopped| {
                let mut last_tick = Instant::now();
                while !stopped.get() {
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
                            CrosstermEvent::Mouse(e) => {
                                sender.send(Event::Mouse(e))
                            }
                            CrosstermEvent::Resize(w, h) => {
                                sender.send(Event::Resize(w, h))
                            }
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
            })
        };
        Self {
            sender,
            receiver,
            handler,
        }
    }

    /// Receive the next event from the handler thread.
    ///
    /// This function will always block the current thread if
    /// there is no data available and it's possible for more data to be sent.
    pub fn next(&self) -> Result<Event> {
        Ok(self.receiver.recv()?)
    }

    pub fn stop(&self) {
        let handle = &self.handler;
        handle.stop();
        // self.handler.stop();
    }
}

