use ratatui::{
    prelude::*,
    style::{Color, Style},
    widgets::*,
    layout::{Layout, Flex,},
};
use crossterm::event::{
    KeyCode, KeyEvent, MouseButton, MouseEventKind};

use crate::{app::Action, mouse_input::MouseInput};

use crate::menus::OpenMenu;


/// Job Allocation Menu
///
/// Contains a list of editable presets
pub struct SallocMenu {
    pub should_render: bool,
    pub handle_input: bool,
    /// The rectangle where to render the menu (for mouse input)
    pub rect: Rect,
    // TODO: add vector of presets
    
    /// The selection state
    pub state: ListState,
}

// ====================================================================
//  CONSTRUCTOR
// ====================================================================

impl SallocMenu {
    pub fn new() -> SallocMenu {
        SallocMenu {
            should_render: false,
            handle_input: false,
            rect: Rect::default(),
            state: ListState::default(),
        }
    }
}


// ====================================================================
//  METHODS
// ====================================================================

impl SallocMenu {

    /// Activate the menu
    pub fn activate(&mut self) {
        self.should_render = true;
        self.handle_input = true;
    }

    /// Deactivate the menu
    pub fn deactivate(&mut self) {
        self.should_render = false;
        self.handle_input = false;
    }

    /// Set the index of list state
    pub fn set_index(&mut self, index: i32) {
        // TODO: set max_ind
        // let max_ind = self.entries.len() as i32 - 1;
        let max_ind: i32 = 0;
        let mut new_index = index;
        if index > max_ind {
            new_index = 0;
        } else if index < 0 {
            new_index = max_ind;
        } 
        self.state.select(Some(new_index as usize));
    }

    fn next(&mut self) {
        let index = self.state.selected();
        match index {
            Some(ind) => self.set_index(ind as i32 + 1),
            None => {}
        };
    }

    fn previous(&mut self) {
        let index = self.state.selected();
        match index {
            Some(ind) => self.set_index(ind as i32 - 1),
            None => {}
        };
    }

}


// ====================================================================
//  RENDERING
// ====================================================================

impl SallocMenu {
    pub fn render(&mut self, f: &mut Frame, _area: &Rect) {
        if !self.should_render { return; }

        let window_width = f.size().width;
        let text_area_width = (0.8 * (window_width as f32)) as u16;

        let window_height = f.size().height;
        let text_area_height = (0.8 * (window_height as f32)) as u16;

        let horizontal = Layout::horizontal([text_area_width]).flex(Flex::Center);
        let vertical = Layout::vertical([text_area_height]).flex(Flex::Center);
        let [rect] = vertical.areas(f.size());
        let [rect] = horizontal.areas(rect);
        self.rect = rect;

        // clear the rect
        f.render_widget(Clear, rect); //this clears out the background

        let block = Block::default()
            .title(block::Title::from(" SALLOC ")
                   .alignment(Alignment::Center))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue))
            .title_style(Style::default().fg(Color::Blue)
                         .add_modifier(Modifier::BOLD));

        f.render_widget(block.clone(), rect);
    }
}


// ====================================================================
//  USER INPUT
// ====================================================================

impl SallocMenu {
    /// Handle user input for the user settings window
    /// Always returns true (input is always handled)
    pub fn input(&mut self, action: &mut Action, key_event: KeyEvent) -> bool {
        if !self.handle_input { return false; }

        match key_event.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                *action = Action::UpdateUserOptions;
                self.deactivate();
            },
            KeyCode::Down | KeyCode::Char('j') => {
                self.next();
            },
            KeyCode::Up | KeyCode::Char('k') => {
                self.previous();
            },
            KeyCode::Char('?') => {
                *action = Action::OpenMenu(OpenMenu::Help(2));
            },
            
            _ => {}
        }
        true
    }
}


// ====================================================================
//  MOUSE INPUT
// ====================================================================

impl SallocMenu {
    pub fn mouse_input(&mut self, 
                       _action: &mut Action, 
                       mouse_input: &mut MouseInput) {
        if !self.handle_input { return;}

        if let Some(mouse_event_kind) = mouse_input.kind() {

            match mouse_event_kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    // close the window if the user clicks outside of it
                    if !self.rect.contains(mouse_input.get_position()) {
                        self.deactivate();
                        mouse_input.click();
                        return;
                    } 
                
                }
                // scrolling
                MouseEventKind::ScrollUp => {
                    self.previous();
                },
                MouseEventKind::ScrollDown => {
                    self.next();
                },
                _ => {}
            }
            // Set the mouse event to handled
            mouse_input.handled = true;
        }
    }
}
