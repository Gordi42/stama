use std::process::{Command};

use crate::{
    job_overview::JobOverview,
    message::{Message, MessageKind},
    mouse_input::MouseInput,};
use crate::job_actions::{JobActionsMenu, JobActions};
use crate::user_options::UserOptions;
use crate::user_options_menu::UserOptionsMenu;
use crate::confirmation::Confirmation;
use crate::job::Job;

#[derive(Debug, Default, Clone)]
pub enum Action {
    #[default]
    None,
    Quit,
    ConfirmedQuit,
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
    pub open_vim: bool,
    pub vim_path: Option<String>,
    pub exit_command: Option<String>,
    pub job_overview: JobOverview,
    pub job_actions_menu: JobActionsMenu,
    pub message: Message,
    pub confirmation: Confirmation,
    pub user_options_menu: UserOptionsMenu,
    pub user_options: UserOptions,
    pub mouse_input: MouseInput,
}

impl App {
    pub fn new() -> Self {
        let user_options = UserOptions::load();
        let refresh_rate = user_options.refresh_rate;
        let mut job_overview = JobOverview::new(refresh_rate);
        job_overview.update_joblist(&user_options);
        Self {
            action: Action::None,
            should_quit: false,
            should_set_frame_rate: false,
            should_redraw: true,
            open_vim: false,
            vim_path: None,
            exit_command: None,
            job_overview: job_overview,
            job_actions_menu: JobActionsMenu::new(),
            message: Message::new_disabled(),
            confirmation: Confirmation::new_disabled(),
            user_options_menu: UserOptionsMenu::load(),
            user_options: user_options,
            mouse_input: MouseInput::new(),
        }
    }


    pub fn tick(&mut self) {
        let job_overview = &mut self.job_overview;
        let user_options = &self.user_options;
        job_overview.update_joblist(user_options);
    }


    pub fn handle_action(&mut self) {
        match &self.action {
            Action::Quit => { 
                self.quit(); 
            }
            Action::ConfirmedQuit => {
                self.confirmed_quit();
            }
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
                self.job_overview.set_index(0);
            }
            Action::JobOption(action) => {
                self.handle_job_action(action.clone());
            }
            _ => {}
        };
        self.action = Action::None;
    }

    pub fn quit(&mut self) {
        if self.user_options.confirm_before_quit {
            self.confirmation = Confirmation::new(
                "Quit?", Action::ConfirmedQuit);
        } else {
            self.should_quit = true;
        }
    }

    pub fn confirmed_quit(&mut self) {
        self.should_quit = true;
    }

    fn open_message(&mut self, message: Message) {
        self.message = message;
    }

    fn open_job_overview(&mut self) {
        self.message = Message::new("Opening job overview not implemented");
    }

    fn open_job_action(&mut self) {
        match self.job_overview.get_job() {
            Some(job) => {
                self.job_actions_menu.activate(&job);
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
            JobActions::Kill(job) => self.open_kill_confirmation(&job),
            JobActions::KillConfirmed(job) => self.kill_job(&job),
            JobActions::OpenLog(_) => self.open_log(),
            JobActions::OpenSubmission(_) => self.open_submissions(),
            JobActions::GoWorkDir(_) => self.go_workdir(),
            JobActions::SSH(_) => self.ssh_to_node(),
        }
    }

    fn open_kill_confirmation(&mut self, job: &Job) {
        if self.user_options.confirm_before_kill {
            let job_name = job.get_jobname();
            let msg = format!("Kill job {} ({})?", job_name, job.id);
            self.confirmation = Confirmation::new(
                &msg, Action::JobOption(
                    JobActions::KillConfirmed(job.clone())));
        } else {
            self.kill_job(job);
        }
    }

    fn kill_job(&mut self, job: &Job) {
        let command_status = Command::new("scancel")
            .arg(job.id.to_string())
            .output();
        match command_status {
            Ok(output) => {
                if !output.status.success() {
                    let error_msg = String::from_utf8_lossy(&output.stderr);
                    self.message = Message::new(&format!("Error killing job: {}", error_msg));
                    self.message.kind = MessageKind::Error;
                }
            }
            Err(e) => {
                self.message = Message::new(&format!("Error killing job: {}", e));
                self.message.kind = MessageKind::Error;
            }
        }
    }

    fn open_log(&mut self) {
        let job = if let Some(job) = self.job_overview.get_job() {
            job
        } else {
            self.message = Message::new("No job selected");
            self.message.kind = MessageKind::Error;
            return;
        };
        let log_path = if let Some(log_path) = &job.output {
            log_path
        } else {
            self.message = Message::new("No log file found");
            self.message.kind = MessageKind::Error;
            return;
        };
        self.vim_path = Some(log_path.clone());
        self.open_vim = true;
    }

    fn open_submissions(&mut self) {
        let job = if let Some(job) = self.job_overview.get_job() {
            job
        } else {
            self.message = Message::new("No job selected");
            self.message.kind = MessageKind::Error;
            return;
        };
        if job.command == "(null)" {
            self.message = Message::new("No submission script found");
            self.message.kind = MessageKind::Error;
            return;
        }
        let parts = job.command.split_whitespace().collect::<Vec<&str>>();
        if parts.len() == 0 {
            self.message = Message::new("No submission script found");
            self.message.kind = MessageKind::Error;
            return;
        }
        if job.is_completed() {
            let mes = format!("Job was submitted with: \n {}", &job.command);
            self.message = Message::new(&mes);
            return;
        }
        self.vim_path = Some(job.command.clone());
        self.open_vim = true;
    }

    fn go_workdir(&mut self) {
        let job = if let Some(job) = self.job_overview.get_job() {
            job
        } else {
            self.message = Message::new("No job selected");
            self.message.kind = MessageKind::Error;
            return;
        };
        let command = format!("cd {}", job.workdir);
        self.exit_command = Some(command);
        self.should_quit = true;
    }

    fn ssh_to_node(&mut self) {
        self.message = Message::new("SSH to node not implemented");
    }



}
