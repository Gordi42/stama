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
    app.menus.input(&mut app.action, key_event);

    app.handle_action();
}

