use ratatui::{
    prelude::*,
    style::{Color, Style},
    widgets::*,
    layout::{Layout, Flex,},
};
use crossterm::event::{
    KeyCode, KeyEvent, MouseButton, MouseEventKind};

use crate::app::Action;
use crate::mouse_input::MouseInput;


pub struct Confirmation {
    pub should_render: bool,
    pub handle_input: bool,
    pub action: Action,
    pub select_yes: bool,
    pub message: String,
    pub confirm_rect: Rect,
    pub yes_rect: Rect,
    pub no_rect: Rect,
}

// ====================================================================
//  CONSTRUCTOR
// ====================================================================

impl Confirmation {
    pub fn new(message: &str, action: Action) -> Self {
        Self {
            should_render: true,
            handle_input: true,
            action: action,
            select_yes: false,
            message: message.to_string(),
            confirm_rect: Rect::default(),
            yes_rect: Rect::default(),
            no_rect: Rect::default(),
        }
    }

    pub fn new_disabled() -> Self {
        Self {
            should_render: false,
            handle_input: false,
            action: Action::None,
            select_yes: false,
            message: "".to_string(),
            confirm_rect: Rect::default(),
            yes_rect: Rect::default(),
            no_rect: Rect::default(),
        }
    }
}

// ====================================================================
//  METHODS
// ====================================================================

impl Confirmation {
    pub fn confirm(&mut self, action: &mut Action) {
        self.should_render = false;
        self.handle_input = false;
        *action = self.action.clone();
    }

    pub fn deny(&mut self) {
        self.should_render = false;
        self.handle_input = false;
    }

    pub fn toggle(&mut self) {
        self.select_yes = !self.select_yes;
    }

    pub fn select(&mut self, action: &mut Action) {
        if self.select_yes {
            self.confirm(action);
        } else {
            self.deny();
        }
    }
}


// ====================================================================
//  RENDERING
// ====================================================================

impl Confirmation {
    pub fn render(&mut self, f: &mut Frame, _area: &Rect) {
        if !self.should_render { return; }

        let window_width = f.area().width;
        let mut text_area_width = 40;
        text_area_width = text_area_width.min(window_width as u16);

        let window_height = f.area().height;
        let mut text_area_height = 9;
        text_area_height = text_area_height.min(window_height as u16);

        let horizontal = Layout::horizontal([text_area_width]).flex(Flex::Center);
        let vertical = Layout::vertical([text_area_height]).flex(Flex::Center);
        let [rect] = vertical.areas(f.area());
        let [rect] = horizontal.areas(rect);
        self.confirm_rect = rect;

        // clear the area
        f.render_widget(Clear, rect);

        // render the border
        let border = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Thick)
            .title_top(Line::from("CONFIRM:")
                   .alignment(Alignment::Center))
            .style(Style::default().fg(Color::Blue)
                   .add_modifier(Modifier::BOLD));

        f.render_widget(border.clone(), rect);

        let outer_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1),
                          Constraint::Min(1),
                          Constraint::Length(3)])
            .split(border.inner(rect));

        let text = Paragraph::new(self.message.clone())
            .style(Style::default().fg(Color::White))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });

        f.render_widget(text, outer_layout[1]);

        let buttons_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(0),
                          Constraint::Length(11),
                          Constraint::Length(3),
                          Constraint::Length(10),
                            Constraint::Min(0)])
            .split(outer_layout[2]);

        self.yes_rect = buttons_layout[1];
        self.no_rect = buttons_layout[3];
        
        let mut yes_button = Paragraph::new("Yes")
            .style(Style::default().fg(Color::White))
            .alignment(Alignment::Center)
            .block(Block::default()
                   .borders(Borders::ALL)
                   .border_type(BorderType::Rounded));

        if self.select_yes {
            yes_button = yes_button.style(Style::default().fg(Color::Blue)
                                         .add_modifier(Modifier::BOLD))
        }

        let mut no_button = Paragraph::new("No")
            .style(Style::default().fg(Color::White))
            .alignment(Alignment::Center)
            .block(Block::default()
                   .borders(Borders::ALL)
                   .border_type(BorderType::Rounded));

        if !self.select_yes {
            no_button = no_button.style(Style::default().fg(Color::Blue)
                                         .add_modifier(Modifier::BOLD))
        }

        f.render_widget(yes_button, buttons_layout[1]);
        f.render_widget(no_button, buttons_layout[3]);

    }
}


// ====================================================================
//  USER INPUT
// ====================================================================

impl Confirmation {
    /// Handle user input for the message window
    /// Always returns true (input is always handled)
    pub fn input(&mut self, action: &mut Action, key_event: KeyEvent) -> bool {
        if !self.handle_input { return false; }

        match key_event.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('n') => {
                self.deny();
            }
            KeyCode::Char('y') => {
                self.confirm(action);
            }
            KeyCode::Char(' ') | KeyCode::Enter => {
                self.select(action);
            }
            KeyCode::Down | KeyCode::Up | KeyCode::Left | KeyCode::Right | 
                KeyCode::Tab |
                KeyCode::Char('h') | KeyCode::Char('j') | 
                KeyCode::Char('k') | KeyCode::Char('l') => {
                self.toggle();
            }
            _ => {}
        }
        true
    }
}


// ====================================================================
//  MOUSE INPUT
// ====================================================================

impl Confirmation {
    pub fn mouse_input(&mut self, 
                       action: &mut Action, 
                       mouse_input: &mut MouseInput) {
        if !self.handle_input { return;}

        if let Some(mouse_event_kind) = mouse_input.kind() {

            match mouse_event_kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    if !self.confirm_rect.contains(mouse_input.get_position()) {
                        self.deny();
                    }
                    if self.yes_rect.contains(mouse_input.get_position()) {
                        self.confirm(action);
                    }
                    if self.no_rect.contains(mouse_input.get_position()) {
                        self.deny();
                    }
                }
                _ => {}
            }
            // Set the mouse event to handled
            mouse_input.click();
        }
    }
}
