use color_eyre::eyre::Result;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::process::{Command, Stdio};

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
pub mod update_content;
pub mod mouse_input;
pub mod job;
pub mod job_overview;
pub mod job_actions;
pub mod text_field;
pub mod user_options;
pub mod user_options_menu;
pub mod message;
pub mod confirmation;


fn main() -> Result<()> {
    // Create an application with 10 jobs.
    let mut app = App::new();
    app.job_overview.set_index(0);
 

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
            Event::Tick => {app.tick();}
            Event::Key(key_event) => update(&mut app, key_event),
            Event::Mouse(mouse_event) => mouse_input(&mut app, mouse_event),
            Event::Resize(_, _) => {}
        };
        if app.should_set_frame_rate {
            tui.events.set_tick_rate(app.user_options.refresh_rate as u64);
            app.should_set_frame_rate = false;
        };
        if app.open_vim {
            let editor = app.user_options.external_editor.as_str();
            match app.vim_path {
                Some(path) => {
                    tui.exit()?;
                    open_editor(&editor, &path);
                    app.open_vim = false;
                    app.vim_path = None;
                    tui.enter()?;
                }
                None => {
                }
            }
        }

    }
    // Exit the user interface.
    tui.exit()?;
    match app.exit_command {
        Some(command) => {
            println!("{}", command);
        }
        None => {}
    }

    Ok(())
}


fn open_editor(editor: &str, path: &str) {
    let mut parts = editor.trim().split_whitespace();
    let program = parts.next().unwrap_or(" ");
    let args: Vec<&str> = parts.collect();

    let mut child = Command::new(program)
        .args(args)
        .arg(path)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn().expect("Failed to execute command");

    // Wait for the process to finish
    child.wait().expect("Failed to wait on child");
}
