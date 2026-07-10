use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent};
use ratatui::{
    prelude::{Alignment, Constraint, Direction, Frame, Layout},
    style::{Color, Style},
    widgets::Paragraph,
};
use std::process::{Command, Stdio};
use std::sync::Arc;

use crate::job::{Job, JobStatus};
use crate::joblist::{JobList, JobListAction, UpdateStatus};
use crate::menus::MenuContainer;
use crate::menus::{
    confirmation::Confirmation,
    job_actions::JobActions,
    message::{Message, MessageKind},
    OpenMenu,
};
use crate::mouse_input::MouseInput;
use crate::notify;
use crate::scheduler::{Scheduler, SlurmScheduler};
use crate::user_options::UserOptions;

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
    /// Remove Salloc Entry (Confirmation Dialog)
    RemoveSallocEntryDialog,
    /// Remove Salloc Entry (Confirmed)
    RemoveSallocEntry,
    /// Updates the user options from the user options menu
    UpdateUserOptions,
    /// Updates the joblist (e.g. job selection, job sorting, etc.)
    UpdateJobList(JobListAction),
    /// Handles a job action (e.g. kill, open log). Boxed because the
    /// embedded [`Job`] makes this variant much larger than the rest.
    JobOption(Box<JobActions>),
    /// Quits stama with an "ssh <node>" exit command to the given node
    /// (emitted by the node selection popup for multi-node jobs)
    SshToNode(String),
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
    /// Executes all non-interactive external commands (scancel,
    /// squeue-for-ssh); the joblist updater shares the same instance.
    scheduler: Arc<dyn Scheduler>,
    /// The last job-update error that was shown in the error popup.
    /// Used to avoid reopening the popup with the same error on every
    /// refresh tick; reset when an update succeeds.
    last_update_error: Option<String>,
}

// ===================================================================
//  CONSTRUCTOR
// ===================================================================

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        Self::with_scheduler(Arc::new(SlurmScheduler))
    }

    /// Creates the app with an injected scheduler (used by tests to
    /// run the app against a fake instead of the real Slurm commands).
    fn with_scheduler(scheduler: Arc<dyn Scheduler>) -> Self {
        // loading user options from config file
        let user_options = UserOptions::load();
        // create the joblist (sharing the scheduler with the app)
        let mut joblist = JobList::with_scheduler(Arc::clone(&scheduler));
        // start the main joblist thread to update the jobs
        joblist.update_jobs(&user_options, None);
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
            scheduler,
            last_update_error: None,
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
                self.handle_job_action((**action).clone());
            }
            Action::SshToNode(node) => {
                let node = node.clone();
                self.ssh_exit(&node);
            }
            Action::StartSalloc(cmd) => {
                self.should_execute_command = true;
                self.command = cmd.to_string();
            }
            Action::RemoveSallocEntryDialog => {
                self.open_remove_salloc_entry_dialog();
            }
            Action::RemoveSallocEntry => {
                self.menus.salloc_menu.delete_current_entry();
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
            self.menus.confirmation = Confirmation::new("Quit?", Action::ConfirmedQuit);
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
        // keep the job table columns of the job overview in sync
        if self.menus.job_overview.columns != self.user_options.job_columns {
            self.menus.job_overview.columns = self.user_options.job_columns.clone();
        }
        // keep the joblist's array-grouping flag in sync so toggling
        // the option takes effect immediately (not only on the next
        // refresh tick)
        self.joblist
            .set_group_job_arrays(self.user_options.group_job_arrays);
    }

    /// Updates the joblist (e.g. job selection, job sorting, etc.)
    fn update_job_list(&mut self, change: JobListAction) {
        self.joblist
            .handle_joblist_action(change, &self.user_options.job_columns);
    }

    /// Handles a job action (e.g. kill, open log)
    fn handle_job_action(&mut self, action: JobActions) {
        match action {
            JobActions::Kill(job) => self.open_kill_confirmation(&job),
            JobActions::KillConfirmed(job) => self.cancel_by_id(&job.id),
            JobActions::KillArray {
                base_id,
                task_count,
            } => self.open_kill_array_confirmation(&base_id, task_count),
            JobActions::KillArrayConfirmed { base_id } => self.cancel_by_id(&base_id),
            JobActions::OpenLog(_) => self.open_log(),
            // the live log view takes the same path as pressing 'L' in
            // the job overview: the menu container resolves the log
            // path of the selected job and opens the fullscreen viewer
            JobActions::ViewLog(_) => self.menus.activate_menu(OpenMenu::LogView, &self.joblist),
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

    /// Open remove salloc entry dialog
    fn open_remove_salloc_entry_dialog(&mut self) {
        self.menus.confirmation = Confirmation::new(
            "Are your sure you want to remove the entry?",
            Action::RemoveSallocEntry,
        );
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
                &msg,
                Action::JobOption(Box::new(JobActions::KillConfirmed(job.clone()))),
            );
        } else {
            self.cancel_by_id(&job.id);
        }
    }

    /// Opens a confirmation dialog to kill a whole job array
    /// (`scancel <base_id>` cancels every task of the array).
    fn open_kill_array_confirmation(&mut self, base_id: &str, task_count: usize) {
        if self.user_options.confirm_before_kill {
            let msg = format!("Kill job array {} ({} tasks)?", base_id, task_count);
            self.menus.confirmation = Confirmation::new(
                &msg,
                Action::JobOption(Box::new(JobActions::KillArrayConfirmed {
                    base_id: base_id.to_string(),
                })),
            );
        } else {
            self.cancel_by_id(base_id);
        }
    }

    /// Cancels a job (or a whole job array, when given an array base
    /// id) with the "scancel" command. If the user has no permission
    /// to kill the job, an error Message will be shown.
    fn cancel_by_id(&mut self, id: &str) {
        // A successful return only means the cancel request was
        // accepted; it does not check whether the job was actually
        // killed.
        if let Err(error) = self.scheduler.cancel_job(id) {
            self.open_error_message(&format!("Error killing job: {}", error));
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
        let output = job.get_stdout();
        let log_path = if let Some(log_path) = &output {
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
            let mes = format!("Job was submitted with: \n {}", job.command);
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
        if let Some(path) = self.vim_path.take() {
            let editor = self.user_options.external_editor.clone();
            let mut parts = editor.split_whitespace();
            let program = parts.next().unwrap_or(" ");
            let args: Vec<&str> = parts.collect();

            let spawn_result = Command::new(program)
                .args(args)
                .arg(&path)
                .stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .spawn();

            // open an error dialog if the editor could not be started
            match spawn_result {
                Err(err) => {
                    let msg = format!("Error opening editor '{}': {}", editor, err);
                    self.open_error_message(&msg);
                }
                Ok(mut child) => {
                    // Wait for the process to finish
                    if let Err(err) = child.wait() {
                        let msg = format!("Error waiting for editor '{}': {}", editor, err);
                        self.open_error_message(&msg);
                    }
                }
            }
        }
        self.open_vim = false;
        self.vim_path = None;
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

    /// Sets the exit command to "ssh <node>" and quits stama.
    /// The command will only be executed in the terminal after closing stama
    /// if a wrapper script is used around stama.
    fn ssh_exit(&mut self, node: &str) {
        self.exit_command = Some(format!("ssh {}", node));
        self.should_quit = true;
    }

    /// Creates an ssh exit command to a node of the selected job.
    /// A job running on a single node is ssh-ed to directly; for a
    /// multi-node job the node selection popup offers the full node
    /// list and its choice comes back as [`Action::SshToNode`], which
    /// takes the same [`App::ssh_exit`] path as the single-node case.
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
        // get the node list of the job; the scheduler expands Slurm's
        // compressed node list, so `nodes` contains every node of the job
        match self.scheduler.job_nodes(&job.id) {
            Ok(nodes) => {
                if nodes.len() > 1 {
                    // several nodes: let the user pick one in the
                    // node selection popup
                    let job_id = job.id.clone();
                    self.menus
                        .activate_menu(OpenMenu::NodeSelect { job_id, nodes }, &self.joblist);
                } else if let Some(node) = nodes.first() {
                    // exactly one node: ssh to it directly
                    let node = node.clone();
                    self.ssh_exit(&node);
                } else {
                    self.open_error_message("Error getting node list: no node found");
                }
            }
            Err(error) => {
                // print an error message if the squeue command to get the
                // node list failed
                self.open_error_message(&format!("Error getting node list: {}", error));
            }
        }
    }

    /// Start the Salloc Command
    pub fn start_salloc(&mut self) {
        println!("{}", self.command);
        let mut parts = self.command.split_whitespace();
        let program = parts.next().unwrap_or(" ");
        let args: Vec<&str> = parts.collect();
        let output_status = Command::new(program)
            .args(args)
            .arg("--no-shell")
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn();

        // open a error dialog if the command could not be executed
        match output_status {
            Err(err) => {
                let msg = format!("Error starting salloc command: {}", err);
                self.open_error_message(&msg);
            }
            Ok(mut child) => {
                // Wait for the process to finish
                if let Err(err) = child.wait() {
                    let msg = format!("Error waiting for salloc command: {}", err);
                    self.open_error_message(&msg);
                }
            }
        }

        self.should_execute_command = false;
    }
}

// ===================================================================
//  Events
// ===================================================================

impl App {
    /// Updates the joblist. Errors from the background update (e.g. a
    /// failing squeue command or a hung worker) are surfaced in the
    /// error popup, and job status transitions are notified to the
    /// user if enabled in the user options.
    pub fn update_jobs(&mut self) {
        // while the log view is open, the background worker also reads
        // the bytes appended to the followed log file since the offset
        // the view has consumed so far
        let log_request = self.menus.log_viewer.follow_request();
        let outcome = self.joblist.update_jobs(&self.user_options, log_request);
        if let Some(update) = outcome.log_follow {
            self.menus.log_viewer.apply_update(update);
        }
        match outcome.status {
            // nothing new this tick; leave the popup state alone
            UpdateStatus::Pending => {}
            // a successful update clears the error memory so that a
            // recurrence of the same error is shown again
            UpdateStatus::Success => {
                self.last_update_error = None;
            }
            UpdateStatus::Error(error) => {
                // only open the popup when the error text changes;
                // otherwise the same error would reopen the popup on
                // every refresh tick
                if self.last_update_error.as_ref() != Some(&error) {
                    self.open_error_message(&error);
                    self.last_update_error = Some(error);
                }
            }
        }
        // notify the user about job state changes (terminal bell and/or
        // OSC 777 desktop notification, per the user options); written
        // to stderr, the same stream the TUI renders to (see main.rs).
        // notification failures must never break the update loop
        if !outcome.transitions.is_empty() {
            let _ = notify::notify(
                &mut std::io::stderr(),
                &outcome.transitions,
                &self.user_options,
            );
        }
    }

    /// Handle keyboard input
    pub fn input(&mut self, key_event: KeyEvent) {
        // Ctrl + C should always quit, regardless of the input mode
        match key_event.code {
            KeyCode::Char('c') | KeyCode::Char('C')
                if key_event.modifiers == KeyModifiers::CONTROL =>
            {
                self.quit();
                return;
            }
            _ => {}
        };

        // pass the key event to the app menus
        self.menus.input(&mut self.action, key_event);

        self.handle_action();
    }

    /// Handles mouse input
    pub fn mouse_input(&mut self, mouse_event: MouseEvent) {
        self.menus
            .mouse_input(&mut self.action, &mut self.mouse_input, mouse_event);

        self.handle_action();
    }

    /// Render the UI
    pub fn render(&mut self, f: &mut Frame) {
        let outer_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(1)].as_ref())
            .split(f.area());

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

// ===================================================================
//  TESTS
// ===================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menus::Menu;
    use crate::scheduler::{FakeScheduler, SchedulerError};

    /// Creates an app that runs against the given fake scheduler.
    /// App construction needs no terminal, only the action handling is
    /// exercised here (no rendering).
    fn app_with_fake(fake: Arc<FakeScheduler>) -> App {
        let mut app = App::with_scheduler(fake);
        // make the kill flow direct instead of opening a confirmation
        app.user_options.confirm_before_kill = false;
        app
    }

    #[test]
    fn kill_flow_cancels_the_selected_job() {
        let fake = Arc::new(FakeScheduler::default());
        let mut app = app_with_fake(Arc::clone(&fake));

        let mut job = Job::new_default();
        job.id = "4242".to_string();
        app.action = Action::JobOption(Box::new(JobActions::Kill(job)));
        app.handle_action();

        // the fake scheduler received exactly one cancel request with
        // the right job id
        assert_eq!(
            *fake.cancelled_jobs.lock().unwrap(),
            vec!["4242".to_string()]
        );
        // a successful cancel opens no error popup
        assert!(!app.menus.message.is_open());
    }

    #[test]
    fn failed_cancel_opens_error_message() {
        let fake = Arc::new(FakeScheduler {
            cancel_response: Err(SchedulerError::CommandFailed {
                program: "scancel".to_string(),
                stderr: "Access/permission denied".to_string(),
            }),
            ..FakeScheduler::default()
        });
        let mut app = app_with_fake(Arc::clone(&fake));

        app.action = Action::JobOption(Box::new(JobActions::KillConfirmed(Job::new_default())));
        app.handle_action();

        assert_eq!(
            *fake.cancelled_jobs.lock().unwrap(),
            vec!["123456".to_string()]
        );
        // the error is routed to the error popup
        assert!(app.menus.message.is_open());
        assert!(app.menus.message.text.contains("Error killing job"));
        assert!(app.menus.message.text.contains("Access/permission denied"));
    }

    #[test]
    fn kill_array_flow_cancels_the_whole_array() {
        let fake = Arc::new(FakeScheduler::default());
        let mut app = app_with_fake(Arc::clone(&fake));

        app.action = Action::JobOption(Box::new(JobActions::KillArray {
            base_id: "12345".to_string(),
            task_count: 50,
        }));
        app.handle_action();

        // the whole array is cancelled with its base id (Slurm applies
        // `scancel 12345` to every task of the array)
        assert_eq!(
            *fake.cancelled_jobs.lock().unwrap(),
            vec!["12345".to_string()]
        );
    }

    #[test]
    fn kill_array_confirmation_names_the_array_and_task_count() {
        let fake = Arc::new(FakeScheduler::default());
        let mut app = app_with_fake(Arc::clone(&fake));
        app.user_options.confirm_before_kill = true;

        app.action = Action::JobOption(Box::new(JobActions::KillArray {
            base_id: "12345".to_string(),
            task_count: 50,
        }));
        app.handle_action();

        // nothing is cancelled yet; the confirmation dialog is open
        // and names the array and its task count
        assert!(fake.cancelled_jobs.lock().unwrap().is_empty());
        assert!(app.menus.confirmation.is_open());
        assert_eq!(
            app.menus.confirmation.message,
            "Kill job array 12345 (50 tasks)?"
        );

        // confirming emits the confirmed action, which cancels the array
        let mut action = Action::None;
        app.menus.confirmation.confirm(&mut action);
        app.action = action;
        app.handle_action();
        assert_eq!(
            *fake.cancelled_jobs.lock().unwrap(),
            vec!["12345".to_string()]
        );
    }

    /// Creates an app whose selected job is running and whose fake
    /// scheduler reports the given nodes for it.
    fn app_with_nodes(nodes: Vec<&str>) -> App {
        let fake = Arc::new(FakeScheduler {
            nodes_response: Ok(nodes.into_iter().map(String::from).collect()),
            ..FakeScheduler::default()
        });
        let mut app = app_with_fake(fake);
        // new_default() creates a running job, so ssh is possible
        app.joblist.jobs.push(Job::new_default());
        app
    }

    #[test]
    fn ssh_to_single_node_job_sets_the_exit_command_directly() {
        let mut app = app_with_nodes(vec!["gpu1"]);

        app.action = Action::JobOption(Box::new(JobActions::SSH(Job::new_default())));
        app.handle_action();

        // one node: no popup, the ssh exit command is set immediately
        assert!(!app.menus.node_select_menu.is_open());
        assert_eq!(app.exit_command.as_deref(), Some("ssh gpu1"));
        assert!(app.should_quit);
    }

    #[test]
    fn ssh_to_multi_node_job_opens_the_node_selection_popup() {
        let mut app = app_with_nodes(vec!["gpu1", "gpu3", "mem1"]);

        app.action = Action::JobOption(Box::new(JobActions::SSH(Job::new_default())));
        app.handle_action();

        // several nodes: the popup opens instead of quitting
        assert!(app.menus.node_select_menu.is_open());
        assert_eq!(
            app.menus.node_select_menu.nodes,
            vec!["gpu1", "gpu3", "mem1"]
        );
        assert!(app.exit_command.is_none());
        assert!(!app.should_quit);

        // the popup's selection takes the same exit-command path
        app.action = Action::SshToNode("gpu3".to_string());
        app.handle_action();
        assert_eq!(app.exit_command.as_deref(), Some("ssh gpu3"));
        assert!(app.should_quit);
    }

    /// End-to-end through the app: opening the log view via the job
    /// actions menu, then ticking the refresh loop, streams the log
    /// file's content (and later appends) into the viewer.
    #[test]
    fn log_view_follows_appended_content_through_update_ticks() {
        use std::io::Write as _;
        use std::thread;
        use std::time::Duration;

        /// Ticks the app's refresh loop until the predicate holds.
        fn tick_until(app: &mut App, predicate: impl Fn(&App) -> bool) {
            for _ in 0..400 {
                app.update_jobs();
                if predicate(app) {
                    return;
                }
                thread::sleep(Duration::from_millis(5));
            }
            panic!("the log view did not receive the expected content in time");
        }

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("job.log");
        std::fs::write(&path, "hello\n").unwrap();

        let mut app = app_with_fake(Arc::new(FakeScheduler::default()));
        let mut job = Job::new_default();
        job.output = Some(path.to_str().unwrap().to_string());
        app.joblist.jobs.push(job);

        // the "View log (live)" entry of the job actions menu opens
        // the fullscreen viewer for the selected job
        app.action = Action::JobOption(Box::new(JobActions::ViewLog(Job::new_default())));
        app.handle_action();
        assert!(app.menus.log_viewer.is_open());
        assert_eq!(app.menus.log_viewer.path, path.to_str().unwrap());

        // the refresh ticks deliver the initial scrollback ...
        tick_until(&mut app, |app| {
            app.menus.log_viewer.lines().iter().any(|l| l == "hello")
        });

        // ... and later appends arrive incrementally
        std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .and_then(|mut file| file.write_all(b"world\n"))
            .unwrap();
        tick_until(&mut app, |app| {
            app.menus.log_viewer.lines().iter().any(|l| l == "world")
        });
        // no line was delivered twice along the way
        assert_eq!(app.menus.log_viewer.lines(), ["hello", "world"]);
    }

    /// End-to-end through the app's key path: 'f' opens the filter
    /// prompt, typing narrows the visible rows live, Enter keeps the
    /// filter and Esc clears it again.
    #[test]
    fn filter_key_flow_narrows_and_clears_the_job_list() {
        let mut app = app_with_fake(Arc::new(FakeScheduler::default()));
        for (id, name) in [("1", "train_model"), ("2", "preprocess"), ("3", "trainer")] {
            let mut job = Job::new_default();
            job.id = id.to_string();
            job.name = name.to_string();
            app.joblist.jobs.push(job);
        }
        assert_eq!(app.joblist.len(), 3);

        let press = |app: &mut App, code: KeyCode| {
            app.input(KeyEvent::new(code, KeyModifiers::NONE));
        };

        // 'f' opens the prompt; typing filters on every keystroke
        press(&mut app, KeyCode::Char('f'));
        assert!(app.menus.job_overview.filter_prompt);
        for c in "train".chars() {
            press(&mut app, KeyCode::Char(c));
        }
        assert_eq!(app.joblist.len(), 2);
        assert_eq!(app.joblist.filter_text(), Some("train"));

        // Enter keeps the filter active and closes the prompt
        press(&mut app, KeyCode::Enter);
        assert!(!app.menus.job_overview.filter_prompt);
        assert_eq!(app.joblist.len(), 2);

        // job actions target the selected *visible* job
        assert!(["train_model", "trainer"].contains(&app.joblist.get_job().unwrap().name.as_str()));

        // Esc clears the filter: all rows are visible again
        press(&mut app, KeyCode::Esc);
        assert_eq!(app.joblist.len(), 3);
        assert_eq!(app.joblist.filter_text(), None);
    }

    #[test]
    fn ssh_with_empty_node_list_opens_an_error_message() {
        let mut app = app_with_nodes(vec![]);

        app.action = Action::JobOption(Box::new(JobActions::SSH(Job::new_default())));
        app.handle_action();

        assert!(!app.menus.node_select_menu.is_open());
        assert!(app.exit_command.is_none());
        assert!(app.menus.message.is_open());
        assert!(app.menus.message.text.contains("no node found"));
    }
}
