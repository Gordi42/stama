use color_eyre::eyre::Result;
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::{
    app::App,
    event::{Event, EventHandler},
    tui::Tui,
    update::update};

pub mod app;
pub mod event;
pub mod ui;
pub mod tui;
pub mod update;
pub mod job;
pub mod job_overview;
pub mod message;

use crate::job::{Job, JobStatus};

fn main() -> Result<()> {
    // Create an application.
    let mut app = App::new();
    app.job_overview.joblist
        .push(Job::new(1, "job1", JobStatus::Running, 10, "partition1", 1));
    app.job_overview.joblist
        .push(Job::new(1, "job2", JobStatus::Pending, 235, "partition2", 2));
    app.job_overview.joblist
        .push(Job::new(1, "job3", JobStatus::Completed, 5123, "partition3", 120));

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
            Event::Tick => {}
            Event::Key(key_event) => update(&mut app, key_event),
            Event::Mouse(_) => {}
            Event::Resize(_, _) => {}
        };
    }

    // Exit the user interface.
    tui.exit()?;

    Ok(())
}


