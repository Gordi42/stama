use crossterm::event::{KeyEvent, MouseButton, MouseEventKind};
use ratatui::{
    prelude::*,
    style::{Color, Style},
    widgets::*,
};

use crate::app::Action;
use crate::menus::{centered_popup, Menu, PopupSize};
use crate::mouse_input::MouseInput;

#[derive(Debug, Clone, Copy)]
pub enum MessageKind {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone)]
pub struct Message {
    open: bool,
    pub text: String,
    pub rect: Rect,
    pub kind: MessageKind,
}

// ====================================================================
//  CONSTRUCTOR
// ====================================================================

impl Message {
    pub fn new(text: &str) -> Self {
        Self {
            open: true,
            text: text.to_string(),
            rect: Rect::default(),
            kind: MessageKind::Info,
        }
    }

    pub fn new_disabled() -> Self {
        Self {
            open: false,
            text: "".to_string(),
            rect: Rect::default(),
            kind: MessageKind::Info,
        }
    }
}

// ====================================================================
//  METHODS
// ====================================================================

impl Message {
    fn close(&mut self) {
        self.open = false;
    }
}

// ====================================================================
//  MENU TRAIT (RENDERING + INPUT)
// ====================================================================

impl Menu for Message {
    fn is_open(&self) -> bool {
        self.open
    }

    fn render(&mut self, f: &mut Frame, _area: &Rect) {
        let color = match self.kind {
            MessageKind::Info => Color::Blue,
            MessageKind::Warning => Color::Yellow,
            MessageKind::Error => Color::Red,
        };

        let title = match self.kind {
            MessageKind::Info => "Info",
            MessageKind::Warning => "Warning",
            MessageKind::Error => "Error",
        };

        let paragraph = Paragraph::new(self.text.clone())
            .style(Style::default().fg(color))
            .wrap(Wrap { trim: true })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .border_type(BorderType::Rounded)
                    .title_top(Line::from("<Esc> to close").alignment(Alignment::Right)),
            );

        // the height depends on the text: get the number of lines the
        // text takes at the popup width, plus 2 for the border
        let text_area_width = (0.8 * (f.area().width as f32)) as u16;
        let text_lines = paragraph.line_count(text_area_width) as u16;

        let rect = centered_popup(
            f.area(),
            PopupSize::Fixed(text_area_width),
            PopupSize::Fixed(text_lines + 2),
        );
        self.rect = rect;

        f.render_widget(Clear, rect);
        f.render_widget(paragraph, rect);
    }

    /// Handle user input for the message window: any key closes it.
    /// (notes.md specifies Down/Up scrolling, but the popup is sized
    /// to fit the whole text and has no scroll state.)
    /// Always returns true (input is always consumed)
    fn input(&mut self, _action: &mut Action, _key_event: KeyEvent) -> bool {
        self.close();
        true
    }

    fn mouse_input(&mut self, _action: &mut Action, mouse_input: &mut MouseInput) {
        if let Some(mouse_event_kind) = mouse_input.kind() {
            if let MouseEventKind::Down(MouseButton::Left) = mouse_event_kind {
                if !self.rect.contains(mouse_input.get_position()) {
                    self.close();
                }
            }
            // Set the mouse event to handled
            mouse_input.click();
        }
    }
}
