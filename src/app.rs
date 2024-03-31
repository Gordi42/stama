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
    pub should_set_frame_rate: bool,
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
        let user_options = UserOptions::load();
        let refresh_rate = user_options.refresh_rate;
        Self {
            action: Action::None,
            should_quit: false,
            should_set_frame_rate: false,
            should_redraw: true,
            job_overview: JobOverview::new(refresh_rate),
            job_actions_menu: JobActionsMenu::new(),
            message: Message::new_disabled(),
            user_options_menu: UserOptionsMenu::load(),
            user_options: user_options,
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
                self.open_message(message.clone());
            }
            Action::OpenJobOverview => {
                self.open_job_overview();
            }
            Action::OpenJobAction => {
                self.open_job_action();
            }
            Action::OpenJobAllocation => {
                self.open_job_allocation();
            }
            Action::OpenUserOptions => {
                self.user_options_menu.activate();
            }
            Action::UpdateUserOptions => {
                self.update_user_options();
            }
            Action::SortJobList => {
                self.job_overview.sort();
            }
            Action::JobOption(action) => {
                self.handle_job_action(action.clone());
            }
            _ => {}
        };
        self.action = Action::None;
    }

    fn open_message(&mut self, message: Message) {
        self.message = message;
    }

    fn open_job_overview(&mut self) {
        self.message = Message::new("Opening job overview not implemented");
    }

    fn open_job_action(&mut self) {
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

    fn open_job_allocation(&mut self) {
        self.message = Message::new("Opening job allocation not implemented");
    }

    fn update_user_options(&mut self) {
        // check if refresh rate has changed
        let old_rate = self.user_options.refresh_rate;
        self.user_options = self.user_options_menu.to_user_option();
        if old_rate != self.user_options.refresh_rate {
            self.job_overview.refresh_rate = self.user_options.refresh_rate;
            self.should_set_frame_rate = true;
        }
    }

    fn handle_job_action(&mut self, action: JobActions) {
        match action {
            _ => {
                self.message = Message::new("Job action not implemented");
            }
        }
    }

}
