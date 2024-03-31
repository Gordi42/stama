use color_eyre::eyre::Result;
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::{
    app::App,
    event::{Event, EventHandler},
    tui::Tui,
    update::update,
    mouse_input::mouse_input,};

pub mod app;
pub mod event;
pub mod ui;
pub mod tui;
pub mod update;
pub mod mouse_input;
pub mod job;
pub mod job_overview;
pub mod job_actions;
pub mod text_field;
pub mod user_options;
pub mod user_options_menu;
pub mod message;


fn main() -> Result<()> {
    // Create an application with 10 jobs.
    let mut app = App::new();
    app.job_overview.set_index(0);
 

    // Initialize the terminal user interface.
    let backend = CrosstermBackend::new(std::io::stderr());
    let terminal = Terminal::new(backend)?;
    let events = EventHandler::new(250);
    let mut tui = Tui::new(terminal, events);
    tui.enter()?;

    // Start the main loop.
    while !app.should_quit {
        // Render the user interface.
        tui.draw(&mut app)?;
        // Handle events.
        match tui.events.next()? {
            Event::Tick => {app.tick();}
            Event::Key(key_event) => update(&mut app, key_event),
            Event::Mouse(mouse_event) => mouse_input(&mut app, mouse_event),
            Event::Resize(_, _) => {}
        };
    }

    // Exit the user interface.
    tui.exit()?;

    Ok(())
}


