#[derive(Debug, Default)]
pub enum Action {
    #[default]
    None,
    Quit,
}

#[derive(Default)]
pub struct App {
    pub action: Action,
    pub should_quit: bool,
    pub should_redraw: bool,
}

impl App {
    pub fn new() -> Self {
        let mut new_app = Self::default();
        new_app.should_redraw = false;
        new_app
    }

    pub fn quit(&mut self) {
        self.should_quit = true;
    }


    pub fn handle_action(&mut self) {
        match self.action {
            Action::Quit => { self.quit(); }
            _ => {}
        };
        self.action = Action::None;
    }

}
