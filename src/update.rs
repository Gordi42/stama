use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::App;

pub fn update(app: &mut App, key_event: KeyEvent) {
    // Ctrl + C should always quit, regardless of the input mode
    match key_event.code {
        KeyCode::Char('c') | KeyCode::Char('C') => {
            if key_event.modifiers == KeyModifiers::CONTROL {
                app.quit()
            }
        }
        _ => {}
    };

    // pass the key event to the app menus

    let mut input_handled = false;
    if !input_handled {
        input_handled = app.message.input(&mut app.action, key_event);};
    if !input_handled {
        input_handled = app.confirmation.input(&mut app.action, key_event);};
    if !input_handled {
        input_handled = app.job_actions_menu.input(&mut app.action, key_event);};
    if !input_handled {
        input_handled = app.user_options_menu.input(&mut app.action, key_event);};
    if !input_handled {
        app.job_overview.input(&mut app.action, key_event);};


    app.handle_action();
}

