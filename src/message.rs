use ratatui::{
    prelude::*,
    style::{Color, Style},
    widgets::*,
    layout::{Layout, Flex,},
};
use crossterm::event::{
    KeyEvent, MouseButton, MouseEventKind};

use crate::app::Action;
use crate::mouse_input::MouseInput;

#[derive(Debug, Clone, Copy)]
pub enum MessageKind {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone)]
pub struct Message {
    pub should_render: bool,
    pub handle_input: bool,
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
            should_render: true,
            handle_input: true,
            text: text.to_string(),
            rect: Rect::default(),
            kind: MessageKind::Info,
        }
    }

    pub fn new_disabled() -> Self {
        Self {
            should_render: false,
            handle_input: false,
            text: "".to_string(),
            rect: Rect::default(),
            kind: MessageKind::Info,
        }
    }
}

// ====================================================================
//  RENDERING
// ====================================================================

impl Message {
    pub fn render(&mut self, f: &mut Frame, _area: &Rect) {
        if !self.should_render { return; }

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
            .block(Block::default()
                   .borders(Borders::ALL)
                   .title(title)
                   .border_type(BorderType::Rounded)
                   .title(block::Title::from("<Esc> to close")
                          .alignment(Alignment::Right)));

        let window_width = f.size().width;
        let text_area_width = (0.8 * (window_width as f32)) as u16;

        // get the number of lines the text will take
        let text_lines = paragraph.line_count(text_area_width) as u16;

        let horizontal = Layout::horizontal([text_area_width]).flex(Flex::Center);
        let vertical = Layout::vertical([text_lines+2]).flex(Flex::Center);
        let [rect] = vertical.areas(f.size());
        let [rect] = horizontal.areas(rect);
        self.rect = rect;

        f.render_widget(Clear, rect);
        f.render_widget(paragraph, rect);
    }
}

// ====================================================================
//  USER INPUT
// ====================================================================

impl Message {
    /// Handle user input for the message window
    /// Always returns true (input is always handled)
    pub fn input(&mut self, _action: &mut Action, _key_event: KeyEvent) -> bool {
        if !self.handle_input { return false; }

        self.should_render = false;
        self.handle_input = false;
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
