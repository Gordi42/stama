use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEventKind};
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
    /// Vertical scroll offset in wrapped text lines. Always 0 for a
    /// freshly constructed message (setting a new message replaces the
    /// whole struct, so the offset resets automatically).
    offset: u16,
    /// The number of wrapped text lines at the last render
    total_lines: u16,
    /// The text height (popup height without borders) at the last render
    visible_height: u16,
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
            offset: 0,
            total_lines: 0,
            visible_height: 0,
        }
    }

    pub fn new_disabled() -> Self {
        Self {
            open: false,
            ..Self::new("")
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

    /// Whether the text did not fit the popup at the last render.
    /// Before the first render both values are 0, so an un-rendered
    /// message keeps the close-on-any-key behavior.
    fn overflows(&self) -> bool {
        self.total_lines > self.visible_height
    }

    /// The largest useful scroll offset (bottom of the text)
    fn max_offset(&self) -> u16 {
        self.total_lines.saturating_sub(self.visible_height)
    }

    /// Half of the visible text height, at least one line
    fn half_page(&self) -> u16 {
        (self.visible_height / 2).max(1)
    }

    fn scroll_down(&mut self, lines: u16) {
        self.offset = (self.offset + lines).min(self.max_offset());
    }

    fn scroll_up(&mut self, lines: u16) {
        self.offset = self.offset.saturating_sub(lines);
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

        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_type(BorderType::Rounded)
            .title_top(Line::from("<Esc> to close").alignment(Alignment::Right));

        let paragraph = Paragraph::new(self.text.clone())
            .style(Style::default().fg(color))
            .wrap(Wrap { trim: true })
            .block(block.clone());

        // the height depends on the text: get the number of lines the
        // text takes at the popup width, plus 2 for the border
        let text_area_width = (0.8 * (f.area().width as f32)) as u16;
        let text_lines = paragraph.line_count(text_area_width) as u16;

        let rect = centered_popup(
            f.area(),
            PopupSize::Fixed(text_area_width),
            PopupSize::Fixed(text_lines.saturating_add(2)),
        );
        self.rect = rect;

        // remember the text geometry so that the input handlers know
        // whether the message needs scrolling, and clamp the offset
        self.total_lines = text_lines;
        self.visible_height = rect.height.saturating_sub(2);
        self.offset = self.offset.min(self.max_offset());

        // when the text overflows the popup, scroll it and show
        // indicators for the hidden lines above/below
        let paragraph = if self.overflows() {
            let mut block = block;
            if self.offset > 0 {
                block = block.title_bottom(Line::from(" ▲ ").alignment(Alignment::Left));
            }
            if self.offset < self.max_offset() {
                block = block.title_bottom(Line::from(" ▼ more ").alignment(Alignment::Right));
            }
            paragraph.scroll((self.offset, 0)).block(block)
        } else {
            paragraph
        };

        f.render_widget(Clear, rect);
        f.render_widget(paragraph, rect);
    }

    /// Handle user input for the message window.
    /// When the text fits the popup, any key closes it. When it
    /// overflows, the scrolling keys scroll and any other key
    /// (including Esc/q/Enter) closes.
    /// Always returns true (input is always consumed)
    fn input(&mut self, _action: &mut Action, key_event: KeyEvent) -> bool {
        if !self.overflows() {
            self.close();
            return true;
        }
        match key_event.code {
            KeyCode::Down | KeyCode::Char('j') => self.scroll_down(1),
            KeyCode::Up | KeyCode::Char('k') => self.scroll_up(1),
            KeyCode::PageDown | KeyCode::Char('d') => self.scroll_down(self.half_page()),
            KeyCode::PageUp | KeyCode::Char('u') => self.scroll_up(self.half_page()),
            KeyCode::Char('g') => self.offset = 0,
            KeyCode::Char('G') => self.offset = self.max_offset(),
            _ => self.close(),
        }
        true
    }

    fn mouse_input(&mut self, _action: &mut Action, mouse_input: &mut MouseInput) {
        if let Some(mouse_event_kind) = mouse_input.kind() {
            match mouse_event_kind {
                // a click outside always closes; a click inside only
                // closes when the text fits (a scrolling message stays
                // open for the scroll wheel)
                MouseEventKind::Down(MouseButton::Left)
                    if !self.rect.contains(mouse_input.get_position()) || !self.overflows() =>
                {
                    self.close();
                }
                MouseEventKind::ScrollDown => self.scroll_down(1),
                MouseEventKind::ScrollUp => self.scroll_up(1),
                _ => {}
            }
            // Set the mouse event to handled
            mouse_input.click();
        }
    }
}

// ====================================================================
//  TESTS
// ====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyModifiers, MouseEvent};
    use ratatui::backend::TestBackend;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    /// Render the message once on a test terminal of the given size so
    /// that the overflow state (total_lines/visible_height) is computed
    fn render(message: &mut Message, width: u16, height: u16) {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let area = f.area();
                message.render(f, &area);
            })
            .unwrap();
    }

    /// A message with many more lines than a 10-row terminal can show
    fn long_message() -> Message {
        let text = (1..=50)
            .map(|i| format!("line {}", i))
            .collect::<Vec<_>>()
            .join("\n");
        Message::new(&text)
    }

    #[test]
    fn test_short_message_closes_on_any_key() {
        let mut message = Message::new("all good");
        render(&mut message, 40, 20);
        assert!(!message.overflows());

        let mut action = Action::None;
        assert!(message.input(&mut action, key(KeyCode::Char('j'))));
        assert!(!message.is_open());
    }

    #[test]
    fn test_long_message_scrolls_and_closes_on_esc() {
        let mut message = long_message();
        render(&mut message, 30, 10);
        assert!(message.overflows());

        let mut action = Action::None;
        // scrolling keys scroll and keep the popup open
        message.input(&mut action, key(KeyCode::Char('j')));
        message.input(&mut action, key(KeyCode::Down));
        assert_eq!(message.offset, 2);
        assert!(message.is_open());
        message.input(&mut action, key(KeyCode::Char('k')));
        assert_eq!(message.offset, 1);

        // any non-scrolling key closes
        message.input(&mut action, key(KeyCode::Esc));
        assert!(!message.is_open());
    }

    #[test]
    fn test_offset_clamps_at_top_and_bottom() {
        let mut message = long_message();
        render(&mut message, 30, 10);
        let max = message.max_offset();
        assert!(max > 0);

        let mut action = Action::None;
        // G jumps to the bottom, further scrolling stays clamped
        message.input(&mut action, key(KeyCode::Char('G')));
        assert_eq!(message.offset, max);
        message.input(&mut action, key(KeyCode::Char('j')));
        assert_eq!(message.offset, max);
        message.input(&mut action, key(KeyCode::PageDown));
        assert_eq!(message.offset, max);

        // g jumps back to the top, scrolling up stays clamped
        message.input(&mut action, key(KeyCode::Char('g')));
        assert_eq!(message.offset, 0);
        message.input(&mut action, key(KeyCode::Char('k')));
        assert_eq!(message.offset, 0);
        assert!(message.is_open());
    }

    #[test]
    fn test_half_page_scrolling() {
        let mut message = long_message();
        render(&mut message, 30, 10);
        let half = message.half_page();
        assert!(half >= 1);

        let mut action = Action::None;
        message.input(&mut action, key(KeyCode::Char('d')));
        assert_eq!(message.offset, half);
        message.input(&mut action, key(KeyCode::PageUp));
        assert_eq!(message.offset, 0);
        assert!(message.is_open());
    }

    #[test]
    fn test_new_message_resets_offset() {
        let mut message = long_message();
        render(&mut message, 30, 10);
        let mut action = Action::None;
        message.input(&mut action, key(KeyCode::Char('G')));
        assert!(message.offset > 0);

        // setting a new message replaces the whole struct (as the
        // MenuContainer does), which resets the scroll state
        message = Message::new("fresh message");
        assert_eq!(message.offset, 0);
        assert!(message.is_open());
    }

    #[test]
    fn test_mouse_scroll_and_clicks() {
        let mut message = long_message();
        render(&mut message, 30, 10);
        let inside = Position::new(message.rect.x + 1, message.rect.y + 1);

        let mouse = |kind: MouseEventKind, pos: Position| {
            let mut input = MouseInput::new();
            input.event = Some(MouseEvent {
                kind,
                column: pos.x,
                row: pos.y,
                modifiers: KeyModifiers::NONE,
            });
            input
        };

        let mut action = Action::None;

        // the scroll wheel scrolls an overflowing message
        let mut input = mouse(MouseEventKind::ScrollDown, inside);
        message.mouse_input(&mut action, &mut input);
        assert_eq!(message.offset, 1);
        assert!(input.handled);
        let mut input = mouse(MouseEventKind::ScrollUp, inside);
        message.mouse_input(&mut action, &mut input);
        assert_eq!(message.offset, 0);

        // a click inside an overflowing message keeps it open
        let mut input = mouse(MouseEventKind::Down(MouseButton::Left), inside);
        message.mouse_input(&mut action, &mut input);
        assert!(message.is_open());

        // a click outside closes it
        let outside = Position::new(0, 0);
        assert!(!message.rect.contains(outside));
        let mut input = mouse(MouseEventKind::Down(MouseButton::Left), outside);
        message.mouse_input(&mut action, &mut input);
        assert!(!message.is_open());

        // a click inside a message that fits closes it
        let mut message = Message::new("all good");
        render(&mut message, 40, 20);
        let inside = Position::new(message.rect.x + 1, message.rect.y + 1);
        let mut input = mouse(MouseEventKind::Down(MouseButton::Left), inside);
        message.mouse_input(&mut action, &mut input);
        assert!(!message.is_open());
    }
}
