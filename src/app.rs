use std::process::{Command, Stdio};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent};
use ratatui::{
    prelude::{Alignment, Constraint, Direction, Frame, Layout},
    style::{Color, Style},
    widgets::Paragraph,
};

use crate::menus::MenuContainer;
use crate::mouse_input::MouseInput;
use crate::user_options::UserOptions;
use crate::menus::{
    OpenMenu,
    job_actions::JobActions,
    message::{Message, MessageKind},
    confirmation::Confirmation,};
use crate::job::{Job, JobStatus};
use crate::joblist::{JobList, JobListAction};

/// At the end of each tick, the app will handle the action that was set
/// during the tick. This enum represents the possible actions that can be
/// taken.
#[derive(Debug, Default, Clone)]
pub enum Action {
    #[default]
    None,
    /// Opens a confirmation dialog to quit the application
    Quit,
    /// This action always quits the application
    ConfirmedQuit,
    /// Opens a selected menu
    OpenMenu(OpenMenu),
    /// Updates the user options from the user options menu
    UpdateUserOptions,
    /// Updates the joblist (e.g. job selection, job sorting, etc.)
    UpdateJobList(JobListAction),
    /// Handles a job action (e.g. kill, open log)
    JobOption(JobActions),
    /// Start the salloc command with the parameters
    StartSalloc(String),
}



/// The main application struct that holds all information and 
/// states of the application.
pub struct App {
    /// The current action that should be taken
    pub action: Action,
    // booleans of actions that are handled in the main loop
    /// If true, the application should quit
    pub should_quit: bool,
    /// If true, the application should reset the frame rate
    pub should_set_frame_rate: bool,
    /// If true, the application should redraw the tui
    pub should_redraw: bool,
    /// If true, the application should execute the given command
    pub should_execute_command: bool,
    /// To open vim, tui must be closed. Hence they must be handled in 
    /// the main loop
    pub open_vim: bool,
    /// The path to the file that should be opened in vim
    vim_path: Option<String>,
    // This command will be written to a given file (for execution after 
    // closing stama)
    pub exit_command: Option<String>,
    /// A buffer for a command
    command: String,
    /// The user options
    pub user_options: UserOptions,
    // The joblist is the main data structure that holds all the jobs
    pub joblist: JobList,
    // All the menus and dialogs
    pub menus: MenuContainer,
    // Mouse input
    pub mouse_input: MouseInput,
}

// ===================================================================
//  CONSTRUCTOR
// ===================================================================

impl App {
    pub fn new() -> Self {
        // loading user options from config file
        let user_options = UserOptions::load();
        // create the joblist
        let mut joblist = JobList::new();
        // start the main joblist thread to update the jobs
        joblist.update_jobs(&user_options); 
        let menus = MenuContainer::new(&user_options, &joblist);
        // create the app
        Self {
            action: Action::None,
            should_quit: false,
            should_set_frame_rate: false,
            should_redraw: true,
            should_execute_command: false,
            open_vim: false,
            vim_path: None,
            exit_command: None,
            command: "".to_string(),
            user_options,
            joblist,
            menus,
            mouse_input: MouseInput::new(),
        }
    }
}

// ===================================================================
// METHODS
// ===================================================================

impl App {
    /// Handles the action that was set during the tick
    pub fn handle_action(&mut self) {
        match &self.action {
            Action::Quit => { 
                self.quit(); 
            }
            Action::ConfirmedQuit => {
                self.confirmed_quit();
            }
            Action::OpenMenu(menu) => {
                self.menus.activate_menu(menu.clone(), &self.joblist);
            }
            Action::UpdateUserOptions => {
                self.update_user_options();
            }
            Action::UpdateJobList(change) => {
                self.update_job_list(change.clone());
            }
            Action::JobOption(action) => {
                self.handle_job_action(action.clone());
            }
            Action::StartSalloc(cmd) => {
                self.should_execute_command = true;
                self.command = cmd.to_string();
            }
            _ => {}
        };
        // reset the action
        self.action = Action::None;
    }

    /// Either opens a confirmation dialog to quit the application 
    /// or quits the application directly if the user options are set 
    /// to not confirm
    pub fn quit(&mut self) {
        if self.user_options.confirm_before_quit {
            self.menus.confirmation = Confirmation::new(
                "Quit?", Action::ConfirmedQuit);
        } else {
            self.should_quit = true;
        }
    }

    /// Quits the application. Always.
    fn confirmed_quit(&mut self) {
        self.should_quit = true;
    }


    /// Updates the user options from the user options menu
    fn update_user_options(&mut self) {
        // save the old refresh rate to check if it has changed
        let old_rate = self.user_options.refresh_rate;
        // update the user options
        self.user_options = self.menus.user_options_menu.to_user_option();
        let new_rate = self.user_options.refresh_rate;
        // update the job overview refresh rate if it has changed
        if old_rate != new_rate {
            self.menus.job_overview.refresh_rate = new_rate;
            self.should_set_frame_rate = true;
        }
    }

    /// Updates the joblist (e.g. job selection, job sorting, etc.)
    fn update_job_list(&mut self, change: JobListAction) {
        self.joblist.handle_joblist_action(change);
    }

    /// Handles a job action (e.g. kill, open log)
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

    /// Open an error message
    fn open_error_message(&mut self, msg: &str) {
        self.menus.message = Message::new(msg);
        self.menus.message.kind = MessageKind::Error;
    }


}


// ===================================================================
// JOB ACTIONS FUNCTIONS
// ===================================================================

impl App {
    /// Opens a confirmation dialog to kill the selected job
    fn open_kill_confirmation(&mut self, job: &Job) {
        if self.user_options.confirm_before_kill {
            let job_name = job.get_jobname();
            let msg = format!("Kill job {} ({})?", job_name, job.id);
            self.menus.confirmation = Confirmation::new(
                &msg, Action::JobOption(
                    JobActions::KillConfirmed(job.clone())));
        } else {
            self.kill_job(job);
        }
    }

    /// Kills the selected job with the "scancel" command
    /// If the user has no permission to kill the job, an error Message
    /// will be shown.
    fn kill_job(&mut self, job: &Job) {
        // perform the kill command
        let command_status = Command::new("scancel")
            .arg(job.id.to_string())
            .output();
        // check if the command was successful. This will check if the command
        // could be executed. It will not check if the job was actually killed.
        match command_status {
            Ok(output) => {
                // Check the exit status of the command. 
                // If it was not successful, show an error message.
                if !output.status.success() {
                    let error_msg = String::from_utf8_lossy(&output.stderr);
                    self.open_error_message(
                        &format!("Error killing job: {}", error_msg));
                }
            }
            Err(e) => {
                // If the command could not be executed, show an error message.
                self.open_error_message(
                    &format!("Error killing job: {}", e));
            }
        }
    }

    /// Opens the log file of the selected job in vim (or the 
    /// user defined editor)
    /// If no log file is found, an error message will be shown.
    fn open_log(&mut self) {
        // get the current job
        let job = match self.joblist.get_job() {
            Some(job) => job,
            None => {
                // if no job is selected, show an error message
                self.open_error_message("No job selected");
                return;
            }
        };
        // try to get the log file path from the job
        let log_path = if let Some(log_path) = &job.output {
            log_path
        } else {
            self.open_error_message("No log file found");
            return;
        };
        // set the vim path and set the open_vim flag to true
        self.vim_path = Some(log_path.clone());
        self.open_vim = true;
    }

    /// Opens the submission script of the selected job in vim (or the 
    /// user defined editor)
    fn open_submissions(&mut self) {
        let job = match self.joblist.get_job() {
            Some(job) => job,
            None => {
                self.open_error_message("No job selected");
                return;
            }
        };
        // if the sumbission command is "(null)", show an error message
        if job.command == "(null)" {
            self.open_error_message("No submission script found");
            return;
        }
        // if the job command is empty (only whitespace), 
        // show an error message
        if job.command.trim().is_empty() {
            self.open_error_message("No submission script found");
            return;
        }
        // For completed jobs, the command is for example:
        // "sbin/sbatch relative/path/to/job.sh"
        // Or all the sbatch options are given in the command.
        // At the moment, I don't know how to decode it to the file path.
        // So I just show the command.
        if job.is_completed() {
            let mes = format!("Job was submitted with: \n {}", &job.command);
            self.menus.message = Message::new(&mes);
            return;
        }
        // Finally, if all checks are passed, open the submission script
        // by setting the vim path and the open_vim flag to true.
        self.vim_path = Some(job.command.clone());
        self.open_vim = true;
    }

    /// Opens file in external editor
    /// This function is called from the main loop when the open_vim
    /// flag is set to true. (see main.rs)
    pub fn open_file_in_editor(&mut self) {
        let editor = self.user_options.external_editor.as_str();
        match &self.vim_path {
            Some(path) => {

                let mut parts = editor.trim().split_whitespace();
                let program = parts.next().unwrap_or(" ");
                let args: Vec<&str> = parts.collect();

                let mut child = Command::new(program)
                    .args(args)
                    .arg(path)
                    .stdin(Stdio::inherit())
                    .stdout(Stdio::inherit())
                    .stderr(Stdio::inherit())
                    .spawn().expect("Failed to execute command");

                // Wait for the process to finish
                child.wait().expect("Failed to wait on child");

                self.open_vim = false;
                self.vim_path = None;
            }
            None => {}
        }
    }

    /// Opens the working directory of the selected job in the terminal
    /// This only works if the exit command is executed in the terminal
    fn go_workdir(&mut self) {
        // first get the current job
        let job = match self.joblist.get_job() {
            Some(job) => job,
            None => {
                // if no job is selected, show an error message
                self.open_error_message("No job selected");
                return;
            }
        };
        // set the exit command to "cd <workdir>"
        let command = format!("cd {}", job.workdir);
        self.exit_command = Some(command);
        // set the should_quit flag to true to exit the application
        self.should_quit = true;
    }

    /// Get the first node in the node list and create a ssh command to it:
    /// "ssh -Y <node>"
    /// The command will only be executed in the terminal after closing stama
    /// if a wrapper script is used around stama.
    fn ssh_to_node(&mut self) {
        // get the current job
        let job = match self.joblist.get_job() {
            Some(job) => job,
            None => {
                // if no job is selected, show an error message
                self.open_error_message("No job selected");
                return;
            }
        };
        // check if the job is running 
        // if not, there will be no node to ssh to
        if job.status != JobStatus::Running {
            // print an error message if the job is not running
            self.open_error_message("Job not running");
            return;
        }
        // get the node list of the job
        let com_stat = Command::new("squeue")
            .arg("-j")
            .arg(&job.id)
            .arg("--Format=NodeList")
            .arg("--noheader")
            .output();
        // check if the command was successful
        match com_stat {
            Ok(output) => {
                if !output.status.success() {
                    // print an error message if the command was not successful
                    let error_msg = String::from_utf8_lossy(&output.stderr);
                    self.open_error_message(
                        &format!("Error getting node list: {}", error_msg));
                    return;
                }
                // format the node list such that only the first node is taken
                // assume format l[42314-42434], or l[42314,42316,42319]
                let node_list = String::from_utf8_lossy(&output.stdout);
                // remove the brackets
                let node_list = node_list.trim().replace("[", "");
                // discard everything after the first comma or dash
                let mut node = node_list.split("-").collect::<Vec<&str>>()[0];
                node = node.split(",").collect::<Vec<&str>>()[0];
                // create the ssh command
                let command = format!("ssh -Y {}", node);
                // set the exit command to the ssh command and set the
                // exit flag to true
                self.exit_command = Some(command);
                self.should_quit = true;
            }
            Err(e) => {
                // print an error message if the squeue command to get the
                // node list could not be executed
                self.open_error_message(
                    &format!("Error getting node list: {}", e));
            }
        }
    }

    /// Start the Salloc Command
    pub fn start_salloc(&mut self) {
        println!("{}", self.command);
        let mut parts = self.command.trim().split_whitespace();
        let program = parts.next().unwrap_or(" ");
        let args: Vec<&str> = parts.collect();
        let output_status = Command::new(program)
            .args(args)
            .arg("--no-shell")
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn(); //.expect("Failed to execute command");

        // open a error dialog if the command could not be executed
        if output_status.is_err() {
            let msg = format!("Error starting salloc command: {}", 
                              output_status.err().unwrap());
            self.open_error_message(&msg);
        }

        self.should_execute_command = false;
    }
}

// ===================================================================
//  Events
// ===================================================================

impl App {
    /// Updates the joblist
    pub fn update_jobs(&mut self) {
        self.joblist.update_jobs(&self.user_options);
    }

    /// Handle keyboard input
    pub fn input(&mut self, key_event: KeyEvent) {
        // Ctrl + C should always quit, regardless of the input mode
        match key_event.code {
            KeyCode::Char('c') | KeyCode::Char('C') => {
                if key_event.modifiers == KeyModifiers::CONTROL {
                    self.quit();
                    return
                }
            }
            _ => {}
        };

        // pass the key event to the app menus
        self.menus.input(&mut self.action, key_event);

        self.handle_action();
    }

    /// Handles mouse input
    pub fn mouse_input(&mut self, mouse_event: MouseEvent) {
        self.menus.mouse_input(
            &mut self.action,
            &mut self.mouse_input,
            mouse_event,
            );

        self.handle_action();
    }

    /// Render the UI
    pub fn render(&mut self, f: &mut Frame) {

        let outer_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                Constraint::Min(3),
                Constraint::Length(1),
                ]
                .as_ref(),
                )
            .split(f.size());

        // make a info text at the bottom
        f.render_widget(
            Paragraph::new("Press `Ctrl-C` or `q` for exit, `?` for help")
            .style(Style::default().fg(Color::LightCyan))
            .alignment(Alignment::Center),
            outer_layout[1],
            );

        // render the windows
        self.menus.render(f, &outer_layout[0], &self.joblist);

    }
        
}
