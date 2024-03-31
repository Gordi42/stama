use std::str::FromStr;
use std::fmt::Display;
use tui_textarea::TextArea;
use std::fmt::Debug;
use ratatui::{
    prelude::*,
    style::{Color, Style},
    widgets::*,
};
use crossterm::event::{
    KeyCode, KeyEvent};

pub enum TextFieldType {
    Text,
    Integer,
    Boolean,
}



pub struct TextField<T: Display + FromStr> {
    pub text_area: TextArea<'static>,
    pub field_type: TextFieldType,
    pub active: bool,
    pub focused: bool,
    pub label: String,
    pub value: T,
}

// ====================================================================
// CONSTRUCTOR
// ====================================================================

impl<T: Display + FromStr> TextField<T> {
    pub fn new(value: T, label: &str) -> Self {
        // match the type of the value
        let field_type = match std::any::type_name::<T>() {
            "bool" => TextFieldType::Boolean,
            "usize" => TextFieldType::Integer,
            "String" => TextFieldType::Text,
            _ => TextFieldType::Text,
        };
        Self {
            text_area: TextArea::from([value.to_string()]),
            field_type: field_type,
            active: false,
            focused: false,
            label: label.to_string(),
            value: value,
        }
    }
}

// ====================================================================
// METHODS
// ====================================================================

impl<T: Display + FromStr> TextField<T> {
    pub fn sync(&mut self) where <T as FromStr>::Err: Debug {
        let lines = self.text_area.lines().join("\n");
        self.value = lines.parse().unwrap();
    }


}

// ====================================================================
//  RENDER
// ====================================================================

impl<T: Display + FromStr> TextField<T> {
    pub fn render(&mut self, f: &mut Frame, area: &Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), 
                         Constraint::Length(2),
                         Constraint::Percentage(50)])
            .split(*area);

        // first render the label
        let mut line = Line::from(self.label.as_str())
            .style(Style::default())
            .alignment(Alignment::Right);

        if self.focused {
            line = line.style(
                Style::default().bg(Color::Blue)
                .fg(Color::Black).add_modifier(Modifier::BOLD));
        };

        f.render_widget(line, chunks[0]);

        if self.focused && self.active {
            self.text_area.set_cursor_style(Style::default().bg(Color::Blue));
            self.text_area.set_cursor_line_style(
                Style::default().fg(Color::Blue)
                .add_modifier(Modifier::BOLD));
        } else if self.focused && !self.active {
            self.text_area.set_cursor_line_style(Style::default());
            self.text_area.set_cursor_style(Style::default());
            self.text_area.set_style(
                Style::default().bg(Color::Blue).fg(Color::Black)
                .add_modifier(Modifier::BOLD));
        } else {
            self.text_area.set_cursor_line_style(Style::default());
            self.text_area.set_cursor_style(Style::default());
            self.text_area.set_style(Style::default());
        }
       

        f.render_widget(self.text_area.widget(), chunks[2]);

    }
}
