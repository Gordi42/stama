use crate::{
    job_overview::JobOverview,
    message::{Message, MessageKind},
    mouse_input::MouseInput,};
use crate::job_actions::{JobActionsMenu, JobActions};
use crate::user_options::UserOptions;
use crate::user_options_menu::UserOptionsMenu;

#[derive(Debug, Default)]
pub enum Action {
    #[default]
    None,
    Quit,
    OpenJobAction,
    OpenJobAllocation,
    OpenJobOverview,
    OpenUserOptions,
    UpdateUserOptions,
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
    pub user_options_menu: UserOptionsMenu,
    pub user_options: UserOptions,
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
            user_options_menu: UserOptionsMenu::load(),
            user_options: UserOptions::load(),
            mouse_input: MouseInput::new(),
        }
    }

    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    pub fn tick(&mut self) {
        let job_overview = &mut self.job_overview;
        let user_options = &self.user_options;
        job_overview.update_joblist(user_options);
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
                match self.job_overview.get_jobname() {
                    Some(job_name) => {
                        self.job_actions_menu.activate(&job_name);
                    }
                    None => {
                        self.message = Message::new("No job selected");
                        self.message.kind = MessageKind::Error;
                    }
                }
            }
            Action::OpenJobAllocation => {
                self.message = Message::new("Opening job allocation not implemented");
            }
            Action::OpenUserOptions => {
                self.user_options_menu.activate();
            }
            Action::UpdateUserOptions => {
                self.user_options = self.user_options_menu.to_user_option();
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
