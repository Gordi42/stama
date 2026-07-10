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

/// Guard to make sure the signal handlers are only registered once.
static SIGNAL_HOOKS: Once = Once::new();

/// Escape sequences that undo the terminal modes set in
/// [`Tui::enter_alternate_screen`]: leave the alternate screen
/// (`?1049l`), show the cursor (`?25h`) and disable all
/// mouse-capture modes enabled by crossterm's `EnableMouseCapture`
/// (`?1000`/`?1002`/`?1003`/`?1015`/`?1006`).
const TERMINAL_RESTORE_SEQUENCES: &[u8] =
    b"\x1b[?1049l\x1b[?25h\x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1015l\x1b[?1006l";

/// Writes [`TERMINAL_RESTORE_SEQUENCES`] directly to stderr.
///
/// Unlike [`Tui::reset`], which goes through crossterm's `execute!`
/// (and may allocate or take locks), this only performs plain
/// `write(2)` syscalls on the raw stderr file descriptor, which is
/// async-signal-safe, so it may be called from a signal handler.
///
/// Known limitation: raw mode (termios) is *not* restored here.
/// Crossterm keeps the saved termios behind a mutex, which must not
/// be touched from a signal handler, and there is no other
/// async-signal-safe way to reach it without extra dependencies.
/// In practice this is benign: interactive shells (bash/zsh line
/// editing) reset the termios settings when they print the next
/// prompt, and `stty sane` / `reset` fixes the rest.
fn restore_terminal_signal_safe() {
    use std::io::Write;
    use std::mem::ManuallyDrop;
    use std::os::fd::FromRawFd;

    // SAFETY: fd 2 (stderr) is valid for the lifetime of the process.
    // `ManuallyDrop` keeps the temporary `File` from closing the fd.
    let mut stderr = ManuallyDrop::new(unsafe { std::fs::File::from_raw_fd(2) });
    let _ = stderr.write_all(TERMINAL_RESTORE_SEQUENCES);
}

/// Registers best-effort terminal-restore handlers for the fatal
/// signals that would otherwise leave the user's shell in raw mode
/// with the alternate screen and mouse capture still active
/// (e.g. a plain `kill <pid>` terminates without unwinding, so
/// neither `Drop` nor the panic hook run).
///
/// SIGINT is included because in raw mode Ctrl+C arrives as a key
/// event handled in-app, so a real SIGINT can only come from the
/// outside (`kill -INT`). After the cleanup the default disposition
/// is emulated so the process still dies with the correct
/// death-by-signal wait status.
fn register_signal_hooks() {
    SIGNAL_HOOKS.call_once(|| {
        use signal_hook::consts::{SIGHUP, SIGINT, SIGQUIT, SIGTERM};

        for signal in [SIGTERM, SIGINT, SIGHUP, SIGQUIT] {
            // SAFETY: the handler only performs async-signal-safe
            // operations: `write(2)` to stderr and
            // `emulate_default_handler`, which is documented as
            // async-signal-safe.
            //
            // Registration can only fail for invalid or forbidden
            // signals; if it does, the Drop/panic hooks still cover
            // the normal exit paths, so the error is ignored.
            let _ = unsafe {
                signal_hook::low_level::register(signal, move || {
                    restore_terminal_signal_safe();
                    // Re-raise with the default disposition so exit
                    // codes / wait statuses stay correct.
                    let _ = signal_hook::low_level::emulate_default_handler(signal);
                })
            };
        }
    });
}

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

        // Restore the terminal even when the process is killed by a
        // signal (which terminates without unwinding, bypassing both
        // Drop and the panic hook).
        register_signal_hooks();

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

#[cfg(test)]
mod tests {
    use super::TERMINAL_RESTORE_SEQUENCES;

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack
            .windows(needle.len())
            .any(|window| window == needle)
    }

    #[test]
    fn restore_sequences_leave_alternate_screen() {
        assert!(contains(TERMINAL_RESTORE_SEQUENCES, b"\x1b[?1049l"));
    }

    #[test]
    fn restore_sequences_show_cursor() {
        assert!(contains(TERMINAL_RESTORE_SEQUENCES, b"\x1b[?25h"));
    }

    #[test]
    fn restore_sequences_disable_all_mouse_capture_modes() {
        // The modes enabled by crossterm's `EnableMouseCapture`.
        for mode in ["1000", "1002", "1003", "1015", "1006"] {
            let sequence = format!("\x1b[?{mode}l");
            assert!(
                contains(TERMINAL_RESTORE_SEQUENCES, sequence.as_bytes()),
                "missing disable sequence for mouse mode {mode}"
            );
        }
    }

    #[test]
    fn restore_sequences_contain_no_enabling_private_modes() {
        // Everything except the cursor-show (`?25h`) must be a
        // "reset/disable" (`l`) sequence, so running the restore can
        // never re-enable a mode.
        let text = String::from_utf8_lossy(TERMINAL_RESTORE_SEQUENCES);
        for part in text.split('\x1b').filter(|part| !part.is_empty()) {
            assert!(
                part.ends_with('l') || part == "[?25h",
                "unexpected sequence: ESC{part}"
            );
        }
    }
}
