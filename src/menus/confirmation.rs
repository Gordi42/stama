use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEventKind};
use ratatui::{
    prelude::*,
    style::{Color, Style},
    widgets::*,
};

use crate::app::Action;
use crate::menus::{centered_popup, Menu, PopupSize};
use crate::mouse_input::MouseInput;

pub struct Confirmation {
    open: bool,
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
            open: true,
            action,
            select_yes: false,
            message: message.to_string(),
            confirm_rect: Rect::default(),
            yes_rect: Rect::default(),
            no_rect: Rect::default(),
        }
    }

    pub fn new_disabled() -> Self {
        Self {
            open: false,
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
        self.open = false;
        *action = self.action.clone();
    }

    pub fn deny(&mut self) {
        self.open = false;
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
//  MENU TRAIT (RENDERING + INPUT)
// ====================================================================

impl Menu for Confirmation {
    fn is_open(&self) -> bool {
        self.open
    }

    fn render(&mut self, f: &mut Frame, _area: &Rect) {
        let rect = centered_popup(f.area(), PopupSize::Fixed(40), PopupSize::Fixed(9));
        self.confirm_rect = rect;

        // clear the area
        f.render_widget(Clear, rect);

        // render the border
        let border = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Thick)
            .title_top(Line::from("CONFIRM:").alignment(Alignment::Center))
            .style(
                Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            );

        f.render_widget(border.clone(), rect);

        let outer_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(1),
                Constraint::Length(3),
            ])
            .split(border.inner(rect));

        let text = Paragraph::new(self.message.clone())
            .style(Style::default().fg(Color::White))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });

        f.render_widget(text, outer_layout[1]);

        let buttons_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(11),
                Constraint::Length(3),
                Constraint::Length(10),
                Constraint::Min(0),
            ])
            .split(outer_layout[2]);

        self.yes_rect = buttons_layout[1];
        self.no_rect = buttons_layout[3];

        let mut yes_button = Paragraph::new("Yes")
            .style(Style::default().fg(Color::White))
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            );

        if self.select_yes {
            yes_button = yes_button.style(
                Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            )
        }

        let mut no_button = Paragraph::new("No")
            .style(Style::default().fg(Color::White))
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            );

        if !self.select_yes {
            no_button = no_button.style(
                Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            )
        }

        f.render_widget(yes_button, buttons_layout[1]);
        f.render_widget(no_button, buttons_layout[3]);
    }

    /// Handle user input for the confirmation window
    /// Always returns true (input is always consumed)
    fn input(&mut self, action: &mut Action, key_event: KeyEvent) -> bool {
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
            KeyCode::Down
            | KeyCode::Up
            | KeyCode::Left
            | KeyCode::Right
            | KeyCode::Tab
            | KeyCode::Char('h')
            | KeyCode::Char('j')
            | KeyCode::Char('k')
            | KeyCode::Char('l') => {
                self.toggle();
            }
            _ => {}
        }
        true
    }

    fn mouse_input(&mut self, action: &mut Action, mouse_input: &mut MouseInput) {
        if let Some(mouse_event_kind) = mouse_input.kind() {
            if let MouseEventKind::Down(MouseButton::Left) = mouse_event_kind {
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
            // Set the mouse event to handled
            mouse_input.click();
        }
    }
}
