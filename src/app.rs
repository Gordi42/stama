use crate::{
    job_overview::JobOverview,
    message::Message,
    mouse_input::MouseInput,};
use crate::job_actions::{JobActionsMenu, JobActions};

#[derive(Debug, Default)]
pub enum Action {
    #[default]
    None,
    Quit,
    OpenJobAction,
    OpenJobAllocation,
    OpenJobOverview,
    OpenMessage(Message),
    SortJobList,
    JobOption(JobActions)
}

pub struct App {
    pub action: Action,
    pub should_quit: bool,
    pub should_redraw: bool,
    pub job_overview: JobOverview,
    pub job_actions_menu: JobActionsMenu,
    pub message: Message,
    pub mouse_input: MouseInput,
}

impl App {
    pub fn new() -> Self {
        Self {
            action: Action::None,
            should_quit: false,
            should_redraw: true,
            job_overview: JobOverview::new(),
            job_actions_menu: JobActionsMenu::new(),
            message: Message::new_disabled(),
            mouse_input: MouseInput::new(),
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
                let job_name = self.job_overview.get_job().name.clone();
                self.job_actions_menu.activate(&job_name);
            }
            Action::OpenJobAllocation => {
                self.message = Message::new("Opening job allocation not implemented");
            }
            Action::SortJobList => {
                self.job_overview.sort();
            }
            Action::JobOption(_action) => {
                self.message = Message::new("Performing job action not implemented");
            }
            _ => {}
        };
        self.action = Action::None;
    }

}
