use crossterm::event::MouseEvent;

use crate::app::App;

pub fn mouse_input(app: &mut App, mouse_event: MouseEvent) {
    let mut input_handled = false;
    if !input_handled {
        input_handled = app.message.mouse_input(&mut app.action, mouse_event);
    }
    if !input_handled {
        app.job_overview.mouse_input(&mut app.action, mouse_event);
    }
}
