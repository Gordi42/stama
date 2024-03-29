use crate::{
    job_overview::JobOverview,
    message::Message,};

#[derive(Debug, Default)]
pub enum Action {
    #[default]
    None,
    Quit,
    OpenJobAction,
    OpenJobAllocation,
    OpenJobOverview,
    OpenMessage(Message),
}

pub struct App {
    pub action: Action,
    pub should_quit: bool,
    pub should_redraw: bool,
    pub job_overview: JobOverview,
    pub message: Message,
}

impl App {
    pub fn new() -> Self {
        Self {
            action: Action::None,
            should_quit: false,
            should_redraw: true,
            job_overview: JobOverview::new(),
            message: Message::new_disabled(),
        }
    }

    pub fn quit(&mut self) {
        self.should_quit = true;
    }


    pub fn handle_action(&mut self) {
        match &self.action {
            Action::Quit => { self.quit(); }
            Action::OpenMessage(message) => {
                self.message = message.clone();
            }
            Action::OpenJobOverview => {
                self.message = Message::new("Opening job overview not implemented");
            }
            Action::OpenJobAction => {
                self.message = Message::new("Opening job action not implemented");
            }
            Action::OpenJobAllocation => {
                self.message = Message::new("Opening job allocation not implemented");
            }
            _ => {}
        };
        self.action = Action::None;
    }

}
