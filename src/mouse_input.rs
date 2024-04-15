use crossterm::event::{MouseEventKind, MouseEvent};
use std::time::SystemTime;
use ratatui::layout::Position;

pub struct MouseInput {
    pub event: Option<MouseEvent>,
    pub handled: bool,
    last_click_time: SystemTime,
    last_click_pos: Position,
}

impl MouseInput {
    pub fn new() -> Self {
        Self {
            event: None,
            handled: false,
            last_click_time: SystemTime::now(),
            last_click_pos: Position::new(0, 0),
        }
    }

    pub fn is_double_click(&mut self) -> bool {
        // first check if the click is in the same position
        if self.last_click_pos != self.get_position() {
            return false;
        }

        let now = SystemTime::now();
        let duration = now.duration_since(self.last_click_time)
            .unwrap_or(std::time::Duration::from_secs(60));
        if duration.as_millis() < 500 {
            self.last_click_time = now;
            return true;
        }
        false
    }

    pub fn get_position(&self) -> Position {
        if let Some(event) = self.event {
            Position::new(event.column, event.row)
        } else {
            Position::new(0, 0)
        }
    }

    pub fn click(&mut self) {
        self.last_click_time = SystemTime::now();
        self.last_click_pos = self.get_position();
        self.handled = true;
    }

    pub fn kind(&self) -> Option<MouseEventKind> {
        if self.handled {
            None
        } else {
            if let Some(event) = &self.event {
                Some(event.kind)
            } else {
                None
            }
        }
    }
}
