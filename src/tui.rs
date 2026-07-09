use std::{io, panic, sync::Once};

use color_eyre::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};

pub type CrosstermTerminal = ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stderr>>;

/// Guard to make sure the panic hook is only registered once,
/// even if the terminal interface is entered multiple times
/// (e.g. after external editor or salloc round trips).
static PANIC_HOOK: Once = Once::new();

use crate::{app::App, event::EventHandler};

/// Representation of a terminal user interface.
///
/// It is responsible for setting up the terminal,
/// initializing the interface and handling the draw events.
pub struct Tui {
    /// Interface to the Terminal.
    terminal: CrosstermTerminal,
    /// Terminal event handler.
    pub events: EventHandler,
}

impl Tui {
    /// Constructs a new instance of [`Tui`].
    pub fn new(terminal: CrosstermTerminal, events: EventHandler) -> Self {
        Self { terminal, events }
    }

    pub fn enter_alternate_screen() -> Result<()> {
        terminal::enable_raw_mode()?;
        crossterm::execute!(io::stderr(), EnterAlternateScreen, EnableMouseCapture)?;
        Ok(())
    }

    /// Initializes the terminal interface.
    ///
    /// It enables the raw mode and sets terminal properties.
    pub fn enter(&mut self) -> Result<()> {
        Self::enter_alternate_screen()?;
        self.events.start();

        // Define a custom panic hook to reset the terminal properties.
        // This way, you won't have your terminal messed up if an unexpected error happens.
        // The hook is only registered once; the reset is best-effort since
        // panicking inside a panic hook would abort the process.
        PANIC_HOOK.call_once(|| {
            let panic_hook = panic::take_hook();
            panic::set_hook(Box::new(move |panic| {
                let _ = Self::reset();
                panic_hook(panic);
            }));
        });

        self.terminal.hide_cursor()?;
        self.terminal.clear()?;
        Ok(())
    }

    /// [`Draw`] the terminal interface by [`rendering`] the widgets.
    ///
    /// [`Draw`]: tui::Terminal::draw
    /// [`rendering`]: crate::ui:render
    pub fn draw(&mut self, app: &mut App) -> Result<()> {
        if app.should_redraw {
            self.terminal.clear()?;
            app.should_redraw = false;
        }
        self.terminal.draw(|frame| app.render(frame))?;
        Ok(())
    }

    /// Resets the terminal interface.
    ///
    /// This function is also used for the panic hook to revert
    /// the terminal properties if unexpected errors occur.
    pub fn reset() -> Result<()> {
        terminal::disable_raw_mode()?;
        crossterm::execute!(io::stderr(), LeaveAlternateScreen, DisableMouseCapture)?;
        Ok(())
    }

    /// Exits the terminal interface.
    ///
    /// It disables the raw mode and reverts back the terminal properties.
    pub fn exit(&mut self) -> Result<()> {
        Self::reset()?;
        self.terminal.show_cursor()?;
        self.events.stop();
        Ok(())
    }
}

impl Drop for Tui {
    /// Best-effort terminal restore.
    ///
    /// This makes sure the terminal is reset even if the main loop
    /// returns early with an error. All operations are safe to call
    /// even if `exit` has already run (disabling raw mode while not
    /// in raw mode is a no-op).
    fn drop(&mut self) {
        let _ = Self::reset();
        let _ = self.terminal.show_cursor();
        self.events.stop();
    }
}
