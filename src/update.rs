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
    // let input_functions = [
    //     || app.message.input(&mut app.action, key_event),
    //     || app.job_overview.input(&mut app.action, key_event),
    // ];
    //
    // let mut input_handled = false;
    // for input_function in input_functions.iter() {
    //     if input_handled {
    //         break;
    //     }
    //     input_handled = input_function();
    // }

    // app.message.input(&mut app.action, key_event);
    // app.job_overview.input(&mut app.action, key_event);


    app.handle_action();
}

