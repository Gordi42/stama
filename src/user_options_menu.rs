use ratatui::{
    prelude::*,
    style::{Color, Style},
    widgets::*,
    layout::{Layout, Flex,},
};
use crossterm::event::{
    KeyCode, KeyEvent, MouseButton, MouseEventKind};

use crate::text_field::{TextField, TextFieldType};
use crate::app::Action;
use crate::mouse_input::MouseInput;
use crate::user_options::UserOptions;


pub struct UserOptionsMenu {
    pub should_render: bool,
    pub handle_input: bool,
    pub rect: Rect,
    pub entries: Vec<TextField>,
    pub index: i32,
    pub state: ListState,
    pub offset: u16,
    pub max_height: u16,
    pub rects: Vec<Rect>,
}

// ====================================================================
//  CONSTRUCTOR
// ====================================================================

impl UserOptionsMenu {
    pub fn from_options(user_options: UserOptions) -> Self {
        let list = user_options.clone();

        let entries = vec![
            TextField::new(
                "Refresh rate (ms)", 
                TextFieldType::Integer(list.refresh_rate)),
            TextField::new(
                "Show completed jobs", 
                TextFieldType::Boolean(list.show_completed_jobs)),
            TextField::new(
                "Confirm before quitting", 
                TextFieldType::Boolean(list.confirm_before_quit)),
            TextField::new(
                "Confirm before killing a job", 
                TextFieldType::Boolean(list.confirm_before_kill)),
            TextField::new(
                "External editor", 
                TextFieldType::Text(list.external_editor)),
            TextField::new(
                "Create dummy jobs", 
                TextFieldType::Boolean(list.dummy_jobs)),
        ];

        Self {
            should_render: false,
            handle_input: false,
            rect: Rect::default(),
            entries: entries,
            index: 0,
            state: ListState::default(),
            offset: 0,
            max_height: 0,
            rects: vec![],
        }
    }

    pub fn load() -> Self {
        let user_options = UserOptions::load();
        let mut user_options_menu = Self::from_options(user_options);
        user_options_menu.set_focus(0, true);
        user_options_menu
    }
}

// ====================================================================
//  METHODS
// ====================================================================

impl UserOptionsMenu {
    pub fn save(&self) {
        let user_option = self.to_user_option();
        user_option.save();
    }

    pub fn to_user_option(&self) -> UserOptions {
        let mut user_options = UserOptions::default();
        user_options.refresh_rate = match &self.entries[0].field_type {
            TextFieldType::Integer(u) => *u,
            _ => 250,
        };
        user_options.show_completed_jobs = match &self.entries[1].field_type {
            TextFieldType::Boolean(b) => *b,
            _ => true,
        };
        user_options.confirm_before_quit = match &self.entries[2].field_type {
            TextFieldType::Boolean(b) => *b,
            _ => false,
        };
        user_options.confirm_before_kill = match &self.entries[3].field_type {
            TextFieldType::Boolean(b) => *b,
            _ => true,
        };
        user_options.external_editor = match &self.entries[4].field_type {
            TextFieldType::Text(s) => s.clone(),
            _ => "vim".to_string(),
        };
        user_options.dummy_jobs = match &self.entries[5].field_type {
            TextFieldType::Boolean(b) => *b,
            _ => false,
        };
        user_options
    }

    pub fn activate(&mut self) {
        self.should_render = true;
        self.handle_input = true;
    }

    pub fn deactivate(&mut self) {
        self.should_render = false;
        self.handle_input = false;
        self.save();
    }

    pub fn set_index(&mut self, index: i32) {
        self.set_focus(self.index as usize, false);
        let max_ind = self.entries.len() as i32 - 1;
        let mut new_index = index;
        if index > max_ind {
            new_index = 0;
        } else if index < 0 {
            new_index = max_ind;
        } 
        self.index = new_index;
        self.set_focus(self.index as usize, true);
        self.state.select(Some(self.index as usize));
    }

    fn next(&mut self) {
        self.set_index(self.index + 1);
    }

    fn previous(&mut self) {
        self.set_index(self.index - 1);
    }

    fn set_focus(&mut self, index: usize, focus: bool) {
        self.entries[index].focused = focus;
    }
}


// ====================================================================
//  RENDERING
// ====================================================================

impl UserOptionsMenu {
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
        self.max_height = rect.height - 2;

        // clear the rect
        f.render_widget(Clear, rect); //this clears out the background

        let block = Block::default()
            .title(block::Title::from("USER SETTINGS:").alignment(Alignment::Center))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue))
            .title_style(Style::default().fg(Color::Blue)
                         .add_modifier(Modifier::BOLD));

        f.render_widget(block.clone(), rect);

        // update the offset
        while self.index < self.offset as i32 {
            self.offset -= 1;
        }
        while self.index > self.offset as i32 + self.max_height as i32 - 1 {
            self.offset += 1;
        }
        let mut num_rows = self.entries.len() - self.offset as usize;
        if num_rows > self.max_height as usize {
            num_rows = self.max_height as usize;
        }
        let constraints = (0..num_rows)
            .map(|_| Constraint::Length(1)).collect::<Vec<_>>();
        let rects = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(block.inner(rect));
        self.rects = rects.to_vec();

        for (rect, entry) in rects.iter().zip(&mut self.entries[self.offset as usize..]) {
            entry.render(f, rect);
        }


    }
}


// ====================================================================
//  USER INPUT
// ====================================================================

impl UserOptionsMenu {
    /// Handle user input for the user settings window
    /// Always returns true (input is always handled)
    pub fn input(&mut self, action: &mut Action, key_event: KeyEvent) -> bool {
        if !self.handle_input { return false; }

        // check if the user is typing in a text field
        let entry = &mut self.entries[self.index as usize];
        if entry.active {
            match key_event.code {
                _ => {
                    entry.input(key_event, action);
                }
            }
            return true;
        }

        match key_event.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('o') => {
                *action = Action::UpdateUserOptions;
                self.deactivate();
            },
            KeyCode::Down | KeyCode::Char('j') => {
                self.next();
            },
            KeyCode::Up | KeyCode::Char('k') => {
                self.previous();
            },
            KeyCode::Enter | KeyCode::Char('i') => {
                self.entries[self.index as usize].on_enter();
                *action = Action::UpdateUserOptions;
            },
            
            _ => {}
        }
        true
    }
}


// ====================================================================
//  MOUSE INPUT
// ====================================================================

impl UserOptionsMenu {
    pub fn mouse_input(&mut self, 
                       action: &mut Action, 
                       mouse_input: &mut MouseInput) {
        if !self.handle_input { return;}

        if let Some(mouse_event_kind) = mouse_input.kind() {

            // check if the user is editing a text field
            let entry = &mut self.entries[self.index as usize];
            if entry.active {
                return;
            }

            match mouse_event_kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    // close the window if the user clicks outside of it
                    if !self.rect.contains(mouse_input.get_position()) {
                        *action = Action::UpdateUserOptions;
                        self.deactivate();
                        mouse_input.click();
                        return;
                    } 
                    // check if the user clicked on a text field
                    for (i, rect) in self.rects.iter().enumerate() {
                        if rect.contains(mouse_input.get_position()) {
                            self.set_index(i as i32 + self.offset as i32);
                            // check if the click is a double click
                            if mouse_input.is_double_click() {
                                self.entries[self.index as usize].on_enter();
                            }
                            mouse_input.click();
                            return;
                        }
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
