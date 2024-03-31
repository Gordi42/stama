use tui_textarea::{TextArea, CursorMove};
use ratatui::{
    prelude::*,
    style::{Color, Style},
};
use crossterm::event::{
    KeyCode, KeyEvent};

use crate::app::Action;

pub enum TextFieldType {
    Text(String),
    Integer(usize),
    Boolean(bool),
}



pub struct TextField {
    pub text_area: TextArea<'static>,
    pub field_type: TextFieldType,
    pub active: bool,
    pub focused: bool,
    pub label: String,
}

// ====================================================================
// CONSTRUCTOR
// ====================================================================

impl TextField {
    pub fn new(label: &str, field_type: TextFieldType) -> Self {
        let mut text_field = Self {
            text_area: TextArea::from([""]),
            field_type: field_type,
            active: false,
            focused: false,
            label: label.to_string(),
        };
        text_field.sync_v2t();
        text_field
    }
}

// ====================================================================
// METHODS
// ====================================================================

impl TextField {
    pub fn sync_t2v(&mut self) {
        let lines = self.text_area.lines().join("\n");
        match self.field_type {
            TextFieldType::Boolean(_) => {
                let b = string_to_bool(lines.as_str());
                self.field_type = TextFieldType::Boolean(b);
            },
            TextFieldType::Integer(_) => {
                let u = lines.parse::<usize>().unwrap_or(0);
                self.field_type = TextFieldType::Integer(u);
            },
            TextFieldType::Text(_) => {
                self.field_type = TextFieldType::Text(lines);
            },
        }
    }

    pub fn sync_v2t(&mut self) {
        let text_content = match self.field_type {
            TextFieldType::Boolean(b) => bool_to_string(b),
            TextFieldType::Integer(i) => i.to_string(),
            TextFieldType::Text(ref s) => s.clone(),
        };
        self.text_area = TextArea::from([text_content]);
        self.text_area.move_cursor(CursorMove::End);
    }

    pub fn on_enter(&mut self) {
        match self.field_type {
            TextFieldType::Boolean(old_value) => {
                self.field_type = TextFieldType::Boolean(!old_value);
                self.sync_v2t();
            },
            TextFieldType::Integer(_) => {
                self.active = true;
            },
            TextFieldType::Text(_) => {
                self.active = true;
            },
        }
    }

    pub fn reset(&mut self) {
        self.sync_v2t();
        self.active = false;
    }

    pub fn apply(&mut self) {
        // check if the value is valid
        let is_valid = match self.field_type {
            TextFieldType::Integer(_) => {
                let lines = self.text_area.lines().join("\n");
                !lines.parse::<usize>().is_err()
            },
            _ => true,
        };
        if is_valid {
            self.sync_t2v();
            self.active = false;
        } else {
            self.reset();
        }
    }
}

fn bool_to_string(b: bool) -> String {
    match b {
        true => "true".to_string(),
        false => "false".to_string(),
    }
}

fn string_to_bool(s: &str) -> bool {
    match s {
        "true" => true,
        "false" => false,
        _ => false,
    }
}

// ====================================================================
//  RENDER
// ====================================================================

impl TextField {
    pub fn render(&mut self, f: &mut Frame, area: &Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), 
                         Constraint::Length(1),
                         Constraint::Percentage(50)])
            .split(*area);

        // first render the label
        let label = format!("{}: ", self.label);
        let mut line = Line::from(label)
            .style(Style::default())
            .alignment(Alignment::Right);

        if self.focused {
            line = line.style(
                Style::default().bg(Color::Blue)
                .fg(Color::Black).add_modifier(Modifier::BOLD));
        };

        f.render_widget(line, chunks[0]);

        // render the middle part
        let mut line = Line::from(" ")
            .style(Style::default());
        if self.focused && !self.active {
            line = line.style(
                Style::default().bg(Color::Blue)
                .fg(Color::Black).add_modifier(Modifier::BOLD));
        };
        f.render_widget(line, chunks[1]);

        // set default style of the text area
        self.text_area.set_cursor_line_style(Style::default());
        self.text_area.set_cursor_style(Style::default());
        self.text_area.set_style(Style::default());

        if self.focused && self.active {
            self.text_area.set_cursor_style(Style::default().bg(Color::Blue));
            self.text_area.set_cursor_line_style(
                Style::default().fg(Color::Blue)
                .add_modifier(Modifier::BOLD));
        } else if self.focused && !self.active {
            self.text_area.set_style(
                Style::default().bg(Color::Blue).fg(Color::Black)
                .add_modifier(Modifier::BOLD));
        } else {
        }

        f.render_widget(self.text_area.widget(), chunks[2]);

    }
}

// ====================================================================
//  USER INPUT
// ====================================================================

impl TextField {
    /// Handle user input for the user settings window
    /// Always returns true (input is always handled)
    pub fn input(&mut self, key_event: KeyEvent, action: &mut Action) -> bool {

        match key_event.code {
            KeyCode::Esc => {
                self.reset();
            },
            KeyCode::Enter => {
                self.apply();
                *action = Action::UpdateUserOptions;
            },
            _ => {
                self.text_area.input(key_event);
            }
        }
        true
    }
}
