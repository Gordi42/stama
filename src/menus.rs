use crossterm::event::{KeyEvent, MouseEvent};
use ratatui::{
    layout::{Flex, Layout, Rect},
    Frame,
};

use crate::app::Action;
use crate::menus::{
    confirmation::Confirmation,
    help::{HelpContext, HelpMenu},
    job_actions::JobActionsMenu,
    job_overview::JobOverview,
    message::Message,
    node_select::NodeSelectMenu,
    user_options_menu::UserOptionsMenu,
};
use crate::mouse_input::MouseInput;
use crate::{joblist::JobList, user_options::UserOptions};

use self::salloc::salloc_menu::SallocMenu;

pub mod confirmation;
pub mod help;
pub mod job_actions;
pub mod job_overview;
pub mod message;
pub mod node_select;
pub mod salloc;
pub mod user_options_menu;

#[derive(Debug, Clone)]
pub enum OpenMenu {
    UserOptions,
    Help(HelpContext),
    Salloc,
    JobActions,
    /// The node selection popup for ssh-ing into a multi-node job
    NodeSelect {
        job_id: String,
        nodes: Vec<String>,
    },
    Message(message::Message),
}

// ===================================================================
//  MENU TRAIT
// ===================================================================

/// The common interface of all popup menus.
///
/// A menu is either open or closed: an open menu is rendered and
/// receives keyboard and mouse input, a closed one is skipped by the
/// `MenuContainer`. The container only calls `render`, `input` and
/// `mouse_input` while the menu is open, so implementations need no
/// "am I open?" guards of their own.
pub trait Menu {
    /// Whether the menu is currently open
    fn is_open(&self) -> bool;

    /// Render the menu. Only called while the menu is open.
    fn render(&mut self, f: &mut Frame, area: &Rect);

    /// Handle a key event. Only called while the menu is open.
    /// Returns true if the event was consumed (menus below and the
    /// job overview then do not see it).
    fn input(&mut self, action: &mut Action, key_event: KeyEvent) -> bool;

    /// Handle a mouse event. Only called while the menu is open.
    /// Menus mark the event as handled via `MouseInput` so that the
    /// menus below them ignore it.
    fn mouse_input(&mut self, action: &mut Action, mouse_input: &mut MouseInput);
}

// ===================================================================
//  SHARED HELPERS
// ===================================================================

/// The size of one popup dimension
#[derive(Debug, Clone, Copy)]
pub enum PopupSize {
    /// A fraction of the frame dimension (0.0..=1.0)
    Fraction(f32),
    /// A fixed number of terminal cells (clipped to the frame)
    Fixed(u16),
}

impl PopupSize {
    /// Resolve the size to a cell count, never exceeding `total`
    fn resolve(self, total: u16) -> u16 {
        match self {
            PopupSize::Fraction(fraction) => (fraction * total as f32) as u16,
            PopupSize::Fixed(cells) => cells.min(total),
        }
    }
}

/// Compute a centered popup rect inside the given frame area.
/// The returned rect never extends beyond the frame area, so it is
/// safe to render into even on very narrow terminals. The caller
/// renders `Clear` plus its own block into the rect.
pub fn centered_popup(frame_area: Rect, width: PopupSize, height: PopupSize) -> Rect {
    let width = width.resolve(frame_area.width);
    let height = height.resolve(frame_area.height);
    let vertical = Layout::vertical([height]).flex(Flex::Center);
    let horizontal = Layout::horizontal([width]).flex(Flex::Center);
    let [rect] = vertical.areas(frame_area);
    let [rect] = horizontal.areas(rect);
    rect
}

/// Wrap-around index arithmetic shared by the list menus: stepping
/// past the last entry wraps to the first and vice versa. A jump
/// beyond either end also wraps (a click below the last row selects
/// the first entry, matching the previous per-menu implementations).
///
/// `len` is the number of selectable rows; a menu with a synthetic
/// trailing row (like the salloc "Create new" row) passes `len + 1`.
pub fn wrap_index(current: isize, delta: isize, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let max = len as isize - 1;
    let target = current + delta;
    if target > max {
        0
    } else if target < 0 {
        max as usize
    } else {
        target as usize
    }
}

/// The Menu Container that contains all menus and dispatches
/// rendering, keyboard and mouse events to them
pub struct MenuContainer {
    /// The Job Overview (Main Task Manager Window)
    pub job_overview: JobOverview,
    /// A menu that shows the available action for the
    /// selected job
    pub job_actions_menu: JobActionsMenu,
    /// A popup to pick one node of a multi-node job to ssh to
    pub node_select_menu: NodeSelectMenu,
    /// A menu for allocating jobs (salloc)
    pub salloc_menu: SallocMenu,
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
                user_options.refresh_rate,
                &joblist.squeue_command,
                user_options.job_columns.clone(),
            ),
            job_actions_menu: JobActionsMenu::new(),
            node_select_menu: NodeSelectMenu::new(),
            salloc_menu: SallocMenu::new(),
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
    /// The popup menus in front-to-back order (the most modal first).
    /// Rendering, keyboard input and mouse input all derive from this
    /// single ordering, so keys and clicks always go to the same menu.
    fn popups_front_to_back(&mut self) -> [&mut dyn Menu; 7] {
        [
            &mut self.confirmation,
            &mut self.message,
            &mut self.help_menu,
            &mut self.user_options_menu,
            &mut self.salloc_menu,
            // the node selection opens on top of the job actions menu
            // (which closes itself when it emits the ssh action)
            &mut self.node_select_menu,
            &mut self.job_actions_menu,
        ]
    }

    /// Opens a selected menu
    pub fn activate_menu(&mut self, open_menu: OpenMenu, joblist: &JobList) {
        match open_menu {
            OpenMenu::JobActions => {
                self.open_job_action(joblist);
            }
            OpenMenu::Salloc => {
                self.salloc_menu.activate();
            }
            OpenMenu::UserOptions => {
                self.user_options_menu.activate();
            }
            OpenMenu::NodeSelect { job_id, nodes } => {
                self.node_select_menu.activate(&job_id, nodes);
            }
            OpenMenu::Message(message) => {
                self.message = message;
            }
            OpenMenu::Help(context) => {
                self.help_menu.open(context);
            }
        }
    }

    /// Opens the job actions menu
    /// This menu shows all the possible actions for the selected job.
    /// For a selected job-array group row, "Kill" targets the whole
    /// array and the other actions apply to the group's first task.
    fn open_job_action(&mut self, joblist: &JobList) {
        if let Some((base_id, task_count, job)) = joblist.selected_group() {
            self.job_actions_menu
                .activate_group(&base_id, task_count, job);
            return;
        }
        match joblist.get_job() {
            Some(job) => {
                self.job_actions_menu.activate(job);
            }
            None => {
                self.message = Message::new("No job selected");
                self.message.kind = message::MessageKind::Error;
            }
        }
    }
}

// ===================================================================
//  RENDER
// ===================================================================

impl MenuContainer {
    /// Render all menus
    pub fn render(&mut self, f: &mut Frame, area: &Rect, joblist: &JobList) {
        // the job overview is the always-visible base screen
        self.job_overview.render(f, area, joblist);
        // render the popups from back to front so that the
        // frontmost menu is drawn last (on top)
        for menu in self.popups_front_to_back().into_iter().rev() {
            if menu.is_open() {
                menu.render(f, area);
            }
        }
    }
}

// ===================================================================
//  INPUT
// ===================================================================

impl MenuContainer {
    /// Handle keyboard input for all menus
    pub fn input(&mut self, action: &mut Action, key_event: KeyEvent) {
        // pass the key event to the open popups from front to back;
        // the first menu that consumes it wins
        for menu in self.popups_front_to_back() {
            if menu.is_open() && menu.input(action, key_event) {
                return;
            }
        }
        // fall through to the base screen
        self.job_overview.input(action, key_event);
    }

    /// Handle mouse input for all menus
    pub fn mouse_input(
        &mut self,
        action: &mut Action,
        mouse_input: &mut MouseInput,
        mouse_event: MouseEvent,
    ) {
        // first update the mouse input with the event
        mouse_input.handled = false;
        mouse_input.event = Some(mouse_event);

        // pass the mouse event to the open popups from front to back
        // (the same order as keyboard input); a menu that handles the
        // event marks it as handled so the menus below ignore it
        for menu in self.popups_front_to_back() {
            if menu.is_open() {
                menu.mouse_input(action, mouse_input);
            }
        }
        self.job_overview.mouse_input(action, mouse_input);
    }
}

// ===================================================================
//  TESTS
// ===================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEventKind};

    // ----------------------------------------------------------------
    //  wrap_index
    // ----------------------------------------------------------------

    #[test]
    fn test_wrap_index_steps_within_bounds() {
        assert_eq!(wrap_index(0, 1, 5), 1);
        assert_eq!(wrap_index(3, -1, 5), 2);
        assert_eq!(wrap_index(2, 0, 5), 2);
    }

    #[test]
    fn test_wrap_index_wraps_at_the_ends() {
        // stepping past the last entry wraps to the first
        assert_eq!(wrap_index(4, 1, 5), 0);
        // stepping before the first entry wraps to the last
        assert_eq!(wrap_index(0, -1, 5), 4);
    }

    #[test]
    fn test_wrap_index_jump_beyond_the_ends_wraps() {
        // an absolute jump beyond the end selects the first entry
        // (e.g. a click below the last row)
        assert_eq!(wrap_index(17, 0, 5), 0);
        assert_eq!(wrap_index(-3, 0, 5), 4);
    }

    #[test]
    fn test_wrap_index_empty_list() {
        assert_eq!(wrap_index(0, 1, 0), 0);
        assert_eq!(wrap_index(0, -1, 0), 0);
    }

    #[test]
    fn test_wrap_index_with_synthetic_trailing_row() {
        // a menu with a synthetic trailing row passes len + 1:
        // index == len is a valid selection
        assert_eq!(wrap_index(2, 1, 3 + 1), 3);
        assert_eq!(wrap_index(3, 1, 3 + 1), 0);
        assert_eq!(wrap_index(0, -1, 3 + 1), 3);
    }

    // ----------------------------------------------------------------
    //  centered_popup
    // ----------------------------------------------------------------

    #[test]
    fn test_centered_popup_is_centered() {
        let frame = Rect::new(0, 0, 100, 50);
        let rect = centered_popup(frame, PopupSize::Fixed(40), PopupSize::Fixed(10));
        assert_eq!(rect, Rect::new(30, 20, 40, 10));
    }

    #[test]
    fn test_centered_popup_fraction() {
        let frame = Rect::new(0, 0, 100, 50);
        let rect = centered_popup(frame, PopupSize::Fraction(0.8), PopupSize::Fraction(0.8));
        assert_eq!(rect.width, 80);
        assert_eq!(rect.height, 40);
    }

    #[test]
    fn test_centered_popup_clips_to_narrow_frame() {
        // a fixed size larger than the frame must be clipped so that
        // rendering into the rect cannot panic on narrow terminals
        let frame = Rect::new(0, 0, 10, 5);
        let rect = centered_popup(frame, PopupSize::Fixed(40), PopupSize::Fixed(9));
        assert!(rect.width <= frame.width);
        assert!(rect.height <= frame.height);
        assert_eq!(rect.intersection(frame), rect);
    }

    // ----------------------------------------------------------------
    //  MenuContainer input routing
    // ----------------------------------------------------------------

    fn container() -> MenuContainer {
        MenuContainer::new(&UserOptions::default(), &JobList::new())
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn left_click(column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    /// Regression test for the key/mouse ordering inconsistency: with
    /// both the confirmation and the message popup open, a key event
    /// must go to the confirmation (the most modal menu) and leave the
    /// message untouched.
    #[test]
    fn test_key_input_goes_to_confirmation_before_message() {
        let mut container = container();
        container.confirmation = Confirmation::new("Quit?", Action::ConfirmedQuit);
        container.message = Message::new("some message");

        let mut action = Action::None;
        container.input(&mut action, key(KeyCode::Esc));

        // Esc denies the confirmation; the message stays open
        assert!(!container.confirmation.is_open());
        assert!(container.message.is_open());
    }

    /// Regression test for the key/mouse ordering inconsistency: a
    /// mouse event must go to the confirmation first as well (it used
    /// to go to the message while key events went to the confirmation).
    #[test]
    fn test_mouse_input_goes_to_confirmation_before_message() {
        let mut container = container();
        container.confirmation = Confirmation::new("Quit?", Action::ConfirmedQuit);
        container.message = Message::new("some message");

        let mut action = Action::None;
        let mut mouse_input = MouseInput::new();
        // the popups were never rendered, so their rects are empty and
        // the click lands outside of them: it closes the confirmation
        container.mouse_input(&mut action, &mut mouse_input, left_click(0, 0));

        assert!(!container.confirmation.is_open());
        assert!(container.message.is_open());
    }

    /// An open node selection popup consumes key input: navigation
    /// keys move its selection instead of falling through to the job
    /// overview, and Enter emits the ssh action for the chosen node.
    #[test]
    fn test_open_node_select_consumes_key_input() {
        let mut container = container();
        container.activate_menu(
            OpenMenu::NodeSelect {
                job_id: "4242".to_string(),
                nodes: vec!["gpu1".to_string(), "gpu3".to_string()],
            },
            &JobList::new(),
        );
        assert!(container.node_select_menu.is_open());

        let mut action = Action::None;
        container.input(&mut action, key(KeyCode::Char('j')));

        // the popup consumed the key: the selection moved and no
        // action leaked through to the base screen
        assert_eq!(container.node_select_menu.index, 1);
        assert!(matches!(action, Action::None));

        container.input(&mut action, key(KeyCode::Enter));
        match action {
            Action::SshToNode(node) => assert_eq!(node, "gpu3"),
            other => panic!("expected Action::SshToNode, got {:?}", other),
        }
        assert!(!container.node_select_menu.is_open());
    }

    /// Opening the job actions menu on a job-array group row targets
    /// the whole array: the kill action carries the base id and the
    /// labels/title name the array and its task count.
    #[test]
    fn test_job_actions_on_group_row_target_the_array() {
        use crate::job::{Job, JobStatus};
        use crate::menus::job_actions::JobActions;

        let mut container = container();
        let mut joblist = JobList::new();
        for id in ["100_1", "100_2"] {
            joblist.jobs.push(Job::new(
                id,
                "array_job",
                JobStatus::Running,
                "00:00:00",
                "main",
                1,
                "/work",
                "cmd",
                None,
            ));
        }
        // row 0 is the collapsed group header
        assert!(joblist.selected_group().is_some());

        container.activate_menu(OpenMenu::JobActions, &joblist);

        assert!(container.job_actions_menu.is_open());
        assert_eq!(
            container.job_actions_menu.job_name,
            "job array 100 (2 tasks)"
        );
        assert_eq!(
            container.job_actions_menu.labels[0],
            "1. Kill job array (2 tasks)"
        );
        match &container.job_actions_menu.actions[0] {
            JobActions::KillArray {
                base_id,
                task_count,
            } => {
                assert_eq!(base_id, "100");
                assert_eq!(*task_count, 2);
            }
            other => panic!("expected KillArray, got {:?}", other),
        }
        // the other actions apply to the group's first task
        match &container.job_actions_menu.actions[1] {
            JobActions::OpenLog(job) => assert_eq!(job.id, "100_1"),
            other => panic!("expected OpenLog, got {:?}", other),
        }

        // opening the menu for a plain job afterwards restores the
        // standard labels and title
        let mut joblist = JobList::new();
        joblist.jobs.push(Job::new_default());
        container.activate_menu(OpenMenu::JobActions, &joblist);
        assert_eq!(container.job_actions_menu.labels[0], "1. Kill job");
        assert_eq!(container.job_actions_menu.job_name, "jobname");
        assert!(matches!(
            container.job_actions_menu.actions[0],
            JobActions::Kill(_)
        ));
    }

    /// Confirming the dialog with 'y' emits the stored action
    #[test]
    fn test_confirmation_key_confirm_emits_action() {
        let mut container = container();
        container.confirmation = Confirmation::new("Quit?", Action::ConfirmedQuit);

        let mut action = Action::None;
        container.input(&mut action, key(KeyCode::Char('y')));

        assert!(matches!(action, Action::ConfirmedQuit));
        assert!(!container.confirmation.is_open());
    }
}
