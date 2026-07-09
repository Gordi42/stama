use color_eyre::eyre::Result;
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::{
    app::App,
    event::{Event, EventHandler},
    tui::Tui,
    write_output::{parse_args, write_output_file},
};

pub mod app;
pub mod event;
pub mod job;
pub mod joblist;
pub mod menus;
pub mod mouse_input;
pub mod scheduler;
pub mod text_field;
pub mod tui;
pub mod update_content;
pub mod user_options;
pub mod write_output;

fn main() -> Result<()> {
    // Parse the command line arguments first, so that `--help`
    // (and invalid arguments) exit before the TUI starts.
    let output_file = parse_args();

    let mut app = App::new();
    app.menus.job_overview.set_index(0);

    // Initialize the terminal user interface.
    let backend = CrosstermBackend::new(std::io::stderr());
    let terminal = Terminal::new(backend)?;
    let tick_rate = app.user_options.refresh_rate as u64;
    let events = EventHandler::new(tick_rate);
    let mut tui = Tui::new(terminal, events);
    tui.enter()?;

    // Start the main loop.
    while !app.should_quit {
        // Render the user interface.
        tui.draw(&mut app)?;
        // Handle events.
        match tui.events.next()? {
            Event::Tick => {
                app.update_jobs();
            }
            Event::Key(key_event) => app.input(key_event),
            Event::Mouse(mouse_event) => app.mouse_input(mouse_event),
            Event::Resize(_, _) => {}
        };
        if app.should_set_frame_rate {
            tui.events
                .set_tick_rate(app.user_options.refresh_rate as u64);
            app.should_set_frame_rate = false;
        };
        if app.open_vim {
            tui.exit()?;
            app.open_file_in_editor();
            tui.enter()?;
        }
        if app.should_execute_command {
            tui.exit()?;
            app.start_salloc();
            tui.enter()?;
        }
    }
    // Exit the user interface.
    tui.exit()?;
    if let Some(command) = app.exit_command {
        write_output_file(output_file.as_deref(), &command);
    }

    Ok(())
}
