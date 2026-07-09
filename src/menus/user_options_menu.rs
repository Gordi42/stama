use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEventKind};
use ratatui::{
    prelude::*,
    style::{Color, Style},
    widgets::*,
};

use crate::app::Action;
use crate::menus::help::HelpContext;
use crate::menus::{centered_popup, wrap_index, Menu, OpenMenu, PopupSize};
use crate::mouse_input::MouseInput;
use crate::text_field::{TextField, TextFieldType};
use crate::user_options::UserOptions;

pub struct UserOptionsMenu {
    open: bool,
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
                TextFieldType::Integer(list.refresh_rate),
            ),
            TextField::new(
                "Show completed jobs",
                TextFieldType::Boolean(list.show_completed_jobs),
            ),
            TextField::new(
                "Confirm before quitting",
                TextFieldType::Boolean(list.confirm_before_quit),
            ),
            TextField::new(
                "Confirm before killing a job",
                TextFieldType::Boolean(list.confirm_before_kill),
            ),
            TextField::new("External editor", TextFieldType::Text(list.external_editor)),
        ];

        Self {
            open: false,
            rect: Rect::default(),
            entries,
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
        UserOptions {
            refresh_rate: match &self.entries[0].field_type {
                TextFieldType::Integer(u) => *u,
                _ => 250,
            },
            show_completed_jobs: match &self.entries[1].field_type {
                TextFieldType::Boolean(b) => *b,
                _ => true,
            },
            confirm_before_quit: match &self.entries[2].field_type {
                TextFieldType::Boolean(b) => *b,
                _ => false,
            },
            confirm_before_kill: match &self.entries[3].field_type {
                TextFieldType::Boolean(b) => *b,
                _ => true,
            },
            external_editor: match &self.entries[4].field_type {
                TextFieldType::Text(s) => s.clone(),
                _ => "vim".to_string(),
            },
        }
    }

    pub fn activate(&mut self) {
        self.open = true;
    }

    pub fn deactivate(&mut self) {
        self.open = false;
        self.save();
    }

    pub fn set_index(&mut self, index: i32) {
        self.set_focus(self.index as usize, false);
        self.index = wrap_index(index as isize, 0, self.entries.len()) as i32;
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
//  MENU TRAIT (RENDERING + INPUT)
// ====================================================================

impl Menu for UserOptionsMenu {
    fn is_open(&self) -> bool {
        self.open
    }

    fn render(&mut self, f: &mut Frame, _area: &Rect) {
        let rect = centered_popup(f.area(), PopupSize::Fraction(0.8), PopupSize::Fraction(0.8));
        self.rect = rect;
        self.max_height = rect.height.saturating_sub(2);

        // clear the rect
        f.render_widget(Clear, rect); //this clears out the background

        let block = Block::default()
            .title_top(Line::from("USER SETTINGS:").alignment(Alignment::Center))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue))
            .title_style(
                Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            );

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
            .map(|_| Constraint::Length(1))
            .collect::<Vec<_>>();
        let rects = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(block.inner(rect));
        self.rects = rects.to_vec();

        for (rect, entry) in rects.iter().zip(&mut self.entries[self.offset as usize..]) {
            entry.render(f, rect);
        }
    }

    /// Handle user input for the user settings window
    /// Always returns true (input is always consumed)
    fn input(&mut self, action: &mut Action, key_event: KeyEvent) -> bool {
        // check if the user is typing in a text field
        let entry = &mut self.entries[self.index as usize];
        if entry.active {
            // a submitted value updates the user options
            if entry.input(key_event) {
                *action = Action::UpdateUserOptions;
            }
            return true;
        }

        match key_event.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('o') => {
                *action = Action::UpdateUserOptions;
                self.deactivate();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.next();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.previous();
            }
            KeyCode::Enter | KeyCode::Char('i') | KeyCode::Char(' ') => {
                self.entries[self.index as usize].on_enter();
                *action = Action::UpdateUserOptions;
            }
            KeyCode::Char('?') => {
                *action = Action::OpenMenu(OpenMenu::Help(HelpContext::StamaSettings));
            }

            _ => {}
        }
        true
    }

    fn mouse_input(&mut self, action: &mut Action, mouse_input: &mut MouseInput) {
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
