use crate::mouse_input::MouseInput;
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEventKind};
use ratatui::{
    prelude::*,
    style::{Color, Style},
    widgets::*,
};

use crate::app::Action;
use crate::menus::{centered_popup, wrap_index, Menu, PopupSize};

/// A centered list popup that lets the user pick one node of a
/// multi-node job. Selecting a node emits [`Action::SshToNode`], which
/// creates the same "ssh <node>" exit command as the single-node case.
pub struct NodeSelectMenu {
    open: bool, // if the menu is open (rendered and handling input)
    pub index: i32,
    pub state: ListState,
    pub nodes: Vec<String>,
    pub job_id: String,
    pub rect: Rect,
}

// ========================================================================
//  CONSTRUCTOR
// ========================================================================

impl Default for NodeSelectMenu {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeSelectMenu {
    pub fn new() -> Self {
        Self {
            open: false,
            index: 0,
            state: ListState::default(),
            nodes: Vec::new(),
            job_id: String::new(),
            rect: Rect::default(),
        }
    }
}

// ========================================================================
//  METHODS
// ========================================================================

impl NodeSelectMenu {
    pub fn set_index(&mut self, index: i32) {
        self.index = wrap_index(index as isize, 0, self.nodes.len()) as i32;
        self.state.select(Some(self.index as usize));
    }

    fn next(&mut self) {
        self.set_index(self.index + 1);
    }

    fn previous(&mut self) {
        self.set_index(self.index - 1);
    }

    /// Emits the ssh action for the selected node and closes the menu
    fn perform_action(&mut self, action: &mut Action) {
        if let Some(node) = self.nodes.get(self.index as usize) {
            *action = Action::SshToNode(node.clone());
        }
        self.deactivate();
    }

    /// Opens the menu with the nodes of the given job
    pub fn activate(&mut self, job_id: &str, nodes: Vec<String>) {
        self.job_id = job_id.to_string();
        self.nodes = nodes;
        self.open = true;
        self.set_index(0);
    }

    pub fn deactivate(&mut self) {
        self.open = false;
    }
}

// ====================================================================
//  MENU TRAIT (RENDERING + INPUT)
// ====================================================================

impl Menu for NodeSelectMenu {
    fn is_open(&self) -> bool {
        self.open
    }

    fn render(&mut self, f: &mut Frame, _area: &Rect) {
        let rect = centered_popup(
            f.area(),
            PopupSize::Fraction(0.8),
            PopupSize::Fixed(self.nodes.len() as u16 + 2),
        );
        self.rect = rect;
        // clear the area
        f.render_widget(Clear, rect);

        let title = format!("Select node (job {})", self.job_id);

        let list = List::new(self.nodes.clone())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title_top(Line::from(title).alignment(Alignment::Center))
                    .border_type(BorderType::Rounded)
                    .style(Style::default().fg(Color::Blue)),
            )
            .highlight_style(
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .bg(Color::Blue)
                    .fg(Color::Black),
            );

        f.render_stateful_widget(list, rect, &mut self.state);
    }

    /// Handle user input for the node selection menu
    /// Always returns true (input is always consumed)
    fn input(&mut self, action: &mut Action, key_event: KeyEvent) -> bool {
        match key_event.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('h') => {
                self.deactivate();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.next();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.previous();
            }
            KeyCode::Enter | KeyCode::Char('l') => {
                self.perform_action(action);
            }
            _ => {}
        }
        true
    }

    fn mouse_input(&mut self, action: &mut Action, mouse_input: &mut MouseInput) {
        if let Some(mouse_event_kind) = mouse_input.kind() {
            match mouse_event_kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    // close the window if the user clicks outside of it
                    if !self.rect.contains(mouse_input.get_position()) {
                        self.deactivate();
                        mouse_input.click();
                    } else {
                        // find the index of the clicked item
                        let y = mouse_input.get_position().y - self.rect.y;
                        let y_min = 1;
                        let y_max = self.nodes.len() as u16 + 1;
                        if y >= y_min && y < y_max {
                            self.set_index(y as i32 - 1);
                        }
                        if mouse_input.is_double_click() {
                            self.perform_action(action);
                        }
                        mouse_input.click();
                    }
                }
                // scrolling
                MouseEventKind::ScrollUp => {
                    self.previous();
                }
                MouseEventKind::ScrollDown => {
                    self.next();
                }
                _ => {}
            }
            // Set the mouse event to handled
            mouse_input.handled = true;
        }
    }
}

// ====================================================================
//  TESTS
// ====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn open_menu() -> NodeSelectMenu {
        let mut menu = NodeSelectMenu::new();
        menu.activate(
            "4242",
            vec!["gpu1".to_string(), "gpu3".to_string(), "mem1".to_string()],
        );
        menu
    }

    #[test]
    fn test_activate_opens_at_the_first_node() {
        let menu = open_menu();
        assert!(menu.is_open());
        assert_eq!(menu.index, 0);
        assert_eq!(menu.job_id, "4242");
    }

    #[test]
    fn test_navigation_wraps_forwards_and_backwards() {
        let mut menu = open_menu();
        let mut action = Action::None;

        // stepping backwards from the first entry wraps to the last
        menu.input(&mut action, key(KeyCode::Char('k')));
        assert_eq!(menu.index, 2);
        // stepping forwards from the last entry wraps to the first
        menu.input(&mut action, key(KeyCode::Char('j')));
        assert_eq!(menu.index, 0);
        // the arrow keys navigate as well
        menu.input(&mut action, key(KeyCode::Down));
        assert_eq!(menu.index, 1);
        menu.input(&mut action, key(KeyCode::Up));
        assert_eq!(menu.index, 0);
    }

    #[test]
    fn test_enter_emits_ssh_action_with_the_selected_node() {
        let mut menu = open_menu();
        let mut action = Action::None;

        menu.input(&mut action, key(KeyCode::Char('j')));
        menu.input(&mut action, key(KeyCode::Enter));

        match action {
            Action::SshToNode(node) => assert_eq!(node, "gpu3"),
            other => panic!("expected Action::SshToNode, got {:?}", other),
        }
        // selecting a node closes the menu
        assert!(!menu.is_open());
    }

    #[test]
    fn test_escape_closes_without_emitting_an_action() {
        let mut menu = open_menu();
        let mut action = Action::None;

        menu.input(&mut action, key(KeyCode::Esc));

        assert!(!menu.is_open());
        assert!(matches!(action, Action::None));
    }
}
