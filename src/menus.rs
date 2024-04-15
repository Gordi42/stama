use crossterm::event::{KeyEvent, MouseEvent};
use ratatui::{Frame, layout::Rect};

use crate::app::Action;
use crate::mouse_input::MouseInput;
use crate::{joblist::JobList, user_options::UserOptions};
use crate::menus::{
    confirmation::Confirmation, 
    help::HelpMenu, 
    job_actions::JobActionsMenu, 
    job_overview::JobOverview, 
    message::Message, 
    user_options_menu::UserOptionsMenu};

pub mod job_overview;
pub mod user_options_menu;
pub mod help;
pub mod job_allocation;
pub mod job_actions;
pub mod message;
pub mod confirmation;

#[derive(Debug, Clone)]
pub enum OpenMenu {
    JobOverview,
    UserOptions,
    Help(usize),
    JobAllocation,
    JobActions,
    Message(message::Message),
}

/// The Menu Container that contains all menus and parses
/// rendering and keyboard events to corresponding menus
pub struct MenuContainer {
    /// The Job Overview (Main Task Manager Window)
    pub job_overview: JobOverview,
    /// A menu that shows the available action for the
    /// selected job
    pub job_actions_menu: JobActionsMenu,
    /// A menu that shows the configurable user options
    pub user_options_menu: UserOptionsMenu,
    /// A popup window that shows help for keybindings
    pub help_menu: HelpMenu,
    /// A popup window that displays a message
    pub message: Message,
    /// A popup window that asks for confirmation
    pub confirmation: Confirmation,
}

// ===================================================================
//  CONSTRUCTOR
// ===================================================================

impl MenuContainer {
    /// Construct a new menu container
    pub fn new(user_options: &UserOptions, joblist: &JobList) -> Self {
        Self {
            job_overview: JobOverview::new(
                user_options.refresh_rate, &joblist.squeue_command),
            job_actions_menu: JobActionsMenu::new(),
            help_menu: HelpMenu::new(),
            message: Message::new_disabled(),
            confirmation: Confirmation::new_disabled(),
            user_options_menu: UserOptionsMenu::load(),
        }
    }
}

// ===================================================================
// METHODS
// ===================================================================

impl MenuContainer {

    /// Opens a selected menu
    pub fn activate_menu(&mut self, open_menu: OpenMenu, joblist: &JobList) {
        match open_menu {
            OpenMenu::JobOverview => {
                self.open_job_overview();
            }
            OpenMenu::JobActions => {
                self.open_job_action(joblist);
            }
            OpenMenu::JobAllocation => {
                self.open_job_allocation();
            }
            OpenMenu::UserOptions => {
                self.user_options_menu.activate();
            }
            OpenMenu::Message(message) => {
                self.open_message(message.clone());
            }
            OpenMenu::Help(selected_category) => {
                self.open_help_menu(selected_category);
            }
        }
    }

    /// Opens the job overview menu
    /// This is the main menu that shows all the jobs
    fn open_job_overview(&mut self) {
        self.message = Message::new("Opening job overview not implemented");
    }

    /// Opens the job actions menu
    /// This menu shows all the possible actions for the selected job
    fn open_job_action(&mut self, joblist: &JobList){
        match joblist.get_job() {
            Some(job) => {
                self.job_actions_menu.activate(&job);
            }
            None => {
                self.message = Message::new("No job selected");
                self.message.kind = message::MessageKind::Error;
            }
        }
    }

    /// Opens the job allocation menu (not implemented)
    fn open_job_allocation(&mut self) {
        self.message = Message::new("Opening job allocation not implemented");
    }

    /// Opens the help menu with a focus on the selected category
    fn open_help_menu(&mut self, selected_category: usize) {
        self.help_menu.open(selected_category);
    }

    /// Opens a message dialog with the given message
    fn open_message(&mut self, message: Message) {
        self.message = message;
    }
}

// ===================================================================
//  RENDER
// ===================================================================

impl MenuContainer {
    /// Render all menus
    pub fn render(&mut self, f: &mut Frame, 
                  area: &Rect, joblist: &JobList) {
        // render from back to front
        // so that the frontmost menu is rendered last
        self.job_overview.render(f, area, joblist);
        self.job_actions_menu.render(f, area);
        self.user_options_menu.render(f, area);
        self.help_menu.render(f, area);
        self.message.render(f, area);
        self.confirmation.render(f, area);
    }
}

// ===================================================================
//  INPUT
// ===================================================================

impl MenuContainer {
    /// Handle keyboard input for all menus
    pub fn input(&mut self, action: &mut Action, key_event: KeyEvent) {
        // keep track of whether the input has been handled
        // so that we can stop processing input if it has
        let mut input_handled = false;
        // pass the key event to the app menus
        // from front to back
        if !input_handled {
            input_handled = self.confirmation.input(action, key_event);
        }
        if !input_handled {
            input_handled = self.message.input(action, key_event);
        }
        if !input_handled {
            input_handled = self.help_menu.input(action, key_event);
        }
        if !input_handled {
            input_handled = self.user_options_menu.input(action, key_event);
        }
        if !input_handled {
            input_handled = self.job_actions_menu.input(action, key_event);
        }
        if !input_handled {
            self.job_overview.input(action, key_event);
        }
    }

    /// Handle mouse input for all menus
    pub fn mouse_input(&mut self, 
                       action: &mut Action,
                       mouse_input: &mut MouseInput,
                       mouse_event: MouseEvent) {
        // first update the mouse input with the event
        mouse_input.handled = false;
        mouse_input.event = Some(mouse_event);

        // pass the mouse event to the app menus
        // from front to back
        self.message.mouse_input(action, mouse_input);
        self.confirmation.mouse_input(action, mouse_input);
        self.help_menu.mouse_input(action, mouse_input);
        self.user_options_menu.mouse_input(action, mouse_input);
        self.job_actions_menu.mouse_input(action, mouse_input);
        self.job_overview.mouse_input(action, mouse_input);
    }
}
