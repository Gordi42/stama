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

#[derive(Debug, Clone)]
pub struct Message {
    pub should_render: bool,
    pub handle_input: bool,
    pub text: String,
    pub color: Color,
    pub height: u16,
    pub rect: Rect,
}

// ====================================================================
//  CONSTRUCTOR
// ====================================================================

impl Message {
    pub fn new(text: &str) -> Self {
        Self {
            should_render: true,
            handle_input: true,
            text: text.to_string(),
            color: Color::Yellow,
            height: 2,
            rect: Rect::default(),
        }
    }

    pub fn new_disabled() -> Self {
        Self {
            should_render: false,
            handle_input: false,
            text: "".to_string(),
            color: Color::Yellow,
            height: 2,
            rect: Rect::default(),
        }
    }
}

// ====================================================================
//  RENDERING
// ====================================================================

impl Message {
    pub fn render(&mut self, f: &mut Frame, _area: &Rect) {
        if !self.should_render { return; }

        let window_width = f.size().width;
        let text_area_width = (0.8 * (window_width as f32)) as u16;

        let horizontal = Layout::horizontal([text_area_width]).flex(Flex::Center);
        let vertical = Layout::vertical([self.height+2]).flex(Flex::Center);
        let [rect] = vertical.areas(f.size());
        let [rect] = horizontal.areas(rect);
        self.rect = rect;

        let paragraph = Paragraph::new(self.text.clone())
            .style(Style::default().fg(self.color))
            .block(Block::default()
                   .borders(Borders::ALL)
                   .title("Message")
                   .border_type(BorderType::Rounded)
                   .title(block::Title::from("<Esc> to close")
                          .alignment(Alignment::Right)));


        f.render_widget(paragraph, rect);
    }
}

// ====================================================================
//  USER INPUT
// ====================================================================

impl Message {
    /// Handle user input for the message window
    /// Always returns true (input is always handled)
    pub fn input(&mut self, _action: &mut Action, key_event: KeyEvent) -> bool {
        if !self.handle_input { return false; }

        match key_event.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.should_render = false;
                self.handle_input = false;
            }
            _ => {}
        }
        true
    }
}

// ====================================================================
//  MOUSE INPUT
// ====================================================================

impl Message {
    pub fn mouse_input(&mut self, 
                       _action: &mut Action, 
                       mouse_input: &mut MouseInput) {
        if !self.handle_input { return;}

        if let Some(mouse_event_kind) = mouse_input.kind() {

            match mouse_event_kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    if !self.rect.contains(mouse_input.get_position()) {
                        self.should_render = false;
                        self.handle_input = false;
                    }
                }
                _ => {}
            }
            // Set the mouse event to handled
            mouse_input.click();
        }
    }
}
