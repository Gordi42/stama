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
    pub fn render(&self, f: &mut Frame, area: &Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), 
                         Constraint::Length(2),
                         Constraint::Percentage(50)])
            .split(*area);
        let line = Line::from(self.label.as_str())
            .style(Style::default())
            .alignment(Alignment::Right);

        f.render_widget(line, chunks[0]);

        f.render_widget(self.text_area.widget(), chunks[2]);

        //
        // let text = match self.field_type {
        //     TextFieldType::Text => "Text",
        //     TextFieldType::Integer => "Integer",
        //     TextFieldType::Boolean => "Boolean",
        // };
        //
        // let text = format!("{}: {}", self.label, text);
        // let text = Paragraph::new(text)
        //     .block(Block::default().borders(Borders::ALL).title("Field Type"))
        //     .wrap(Wrap { trim: true });
        //
        // f.render_widget(text, chunks[0]);
        //
        // let text_area = self.text_area.render(area);
        // f.render_widget(text_area, chunks[1]);
    }
}
