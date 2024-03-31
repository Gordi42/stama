use color_eyre::eyre::{self, Result};
use std::fs::{self, File};
use std::io::prelude::*;
use serde::{Deserialize, Serialize};
use ratatui::{
    prelude::*,
    style::{Color, Style},
    widgets::*,
    layout::{Layout, Flex,},
};
use crossterm::event::{
    KeyCode, KeyEvent, MouseButton, MouseEventKind};

use crate::text_field::{TextField};
use crate::app::Action;
use crate::mouse_input::MouseInput;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct UserOptionsList {
    pub refresh_rate: usize,          // Refresh rate in milliseconds
    pub show_completed_jobs: bool,  // Show completed jobs
    pub confirm_before_quit: bool,  // Confirm before quitting
    pub confirm_before_kill: bool,  // Confirm before killing a job
    pub external_editor: String,    // External editor command (e.g. "vim")
    pub _placeholder1: String,        // Placeholder for future options
    pub _placeholder2: String,        // Placeholder for future options
    pub _placeholder3: String,        // Placeholder for future options
    pub _placeholder4: String,        // Placeholder for future options
    pub _placeholder5: String,        // Placeholder for future options
    pub _placeholder6: String,        // Placeholder for future options
    pub _placeholder7: String,        // Placeholder for future options
}

impl Default for UserOptionsList {
    fn default() -> Self {
        Self {
            refresh_rate: 250,
            show_completed_jobs: true,
            confirm_before_quit: false,
            confirm_before_kill: true,
            external_editor: "vim".to_string(),
            _placeholder1: "dummy".to_string(),
            _placeholder2: "dummy".to_string(),
            _placeholder3: "dummy".to_string(),
            _placeholder4: "dummy".to_string(),
            _placeholder5: "dummy".to_string(),
            _placeholder6: "dummy".to_string(),
            _placeholder7: "dummy".to_string(),
        }
    }
}

// ====================================================================
//  LOADING AND SAVING
// ====================================================================

impl UserOptionsList {
    pub fn load_from_file() -> Result<Self> {
        let file_dir = get_file_dir()?;
        let file_path = get_file_path(&file_dir);

        let contents = fs::read_to_string(file_path)?;
        let user_options_list = toml::from_str(&contents)?;
        Ok(user_options_list)
    }


    pub fn load() -> Self {
        if !file_exists() {
            return Self::default();
        }
        match Self::load_from_file() {
            Ok(user_options_list) => user_options_list,
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) {
        match self.try_save() {
            Ok(_) => (),
            Err(_) => (),
        }
    }

    pub fn try_save(&self) -> Result<()> {
        let file_dir = get_file_dir()?;
        touch_dir(&file_dir)?;
        let file_path = get_file_path(&file_dir);

        let toml = toml::to_string(self)?;
        let mut file = File::create(file_path)?;
        file.write_all(toml.as_bytes())?;
        Ok(())
    }

}

fn get_file_dir() -> Result<String> {
    let home = std::env::var("HOME");
    let home = match home {
        Ok(path) => path,
        Err(_) => return Err(eyre::eyre!(
                "Could not find HOME environment variable")),
    };
    Ok(format!("{}/.config/stama", home))
}

fn get_file_path(file_dir: &str) -> String {
    format!("{}/config.toml", file_dir)
}

fn file_exists() -> bool {
    let file_dir = match get_file_dir() {
        Ok(file_dir) => file_dir,
        Err(_) => return false,
    };
    let file_path = get_file_path(&file_dir);
    std::path::Path::new(&file_path).exists()
}

fn touch_dir(file_dir: &str) -> Result<()> {
    // create the directory
    match fs::create_dir_all(file_dir) {
        Ok(_) => Ok(()),
        Err(_) => Err(eyre::eyre!("Could not create directory")),
    }
}





pub enum Entry {
    Integer(TextField<usize>),
    Boolean(TextField<bool>),
    Text(TextField<String>),
}

pub struct UserOptions {
    pub should_render: bool,
    pub handle_input: bool,
    pub rect: Rect,
    pub entries: Vec<Entry>,
    pub index: i32,
    pub state: ListState,
    pub offset: u16,
    pub max_height: u16,
}

// ====================================================================
//  CONSTRUCTOR
// ====================================================================

impl UserOptions {
    pub fn from_list(list: UserOptionsList) -> Self {
        let list = list.clone();

        let entries = vec![
            Entry::Integer(TextField::new(
                list.refresh_rate, 
                "Refresh rate (ms)")),
            Entry::Boolean(TextField::new(
                list.show_completed_jobs, 
                "Show completed jobs")),
            Entry::Boolean(TextField::new(
                list.confirm_before_quit, 
                "Confirm before quitting")),
            Entry::Boolean(TextField::new(
                list.confirm_before_kill, 
                "Confirm before killing a job")),
            Entry::Text(TextField::new(
                list.external_editor, 
                "External editor")),
            Entry::Text(TextField::new(
                list._placeholder1, 
                "Placeholder1")),
            Entry::Text(TextField::new(
                list._placeholder2, 
                "Placeholder2")),
            Entry::Text(TextField::new(
                list._placeholder3, 
                "Placeholder3")),
            Entry::Text(TextField::new(
                list._placeholder4, 
                "Placeholder4")),
            Entry::Text(TextField::new(
                list._placeholder5, 
                "Placeholder5")),
            Entry::Text(TextField::new(
                list._placeholder6, 
                "Placeholder6")),
            Entry::Text(TextField::new(
                list._placeholder7, 
                "Placeholder7")),
        ];

        Self {
            should_render: false,
            handle_input: false,
            rect: Rect::default(),
            entries: entries,
            index: 0,
            state: ListState::default(),
            offset: 0,
            max_height: 0,
        }
    }

    pub fn load() -> Self {
        let list = UserOptionsList::load();
        let mut user_options = Self::from_list(list);
        user_options.set_focus(0, true);
        user_options
    }
}

// ====================================================================
//  METHODS
// ====================================================================

impl UserOptions {
    pub fn save(&self) {
        let list = self.to_list();
        list.save();
    }

    pub fn to_list(&self) -> UserOptionsList {
        let mut user_options = UserOptionsList::default();
        user_options.refresh_rate = match &self.entries[0] {
            Entry::Integer(text_field) => text_field.value,
            _ => 0,
        };
        user_options
    }

    pub fn activate(&mut self) {
        self.should_render = true;
        self.handle_input = true;
    }

    pub fn deactivate(&mut self) {
        self.should_render = false;
        self.handle_input = false;
        self.save();
    }

    pub fn set_index(&mut self, index: i32) {
        self.set_focus(self.index as usize, false);
        let max_ind = self.entries.len() as i32 - 1;
        let mut new_index = index;
        if index > max_ind {
            new_index = 0;
        } else if index < 0 {
            new_index = max_ind;
        } 
        self.index = new_index;
        self.set_focus(self.index as usize, true);
        self.state.select(Some(self.index as usize));
    }

    fn next(&mut self) {
        self.set_index(self.index + 1);
    }

    fn previous(&mut self) {
        self.set_index(self.index - 1);
    }

    fn set_focus(&mut self, index: usize, focus: bool) {
        match &mut self.entries[index] {
            Entry::Integer(text_field) => text_field.focused = focus,
            Entry::Boolean(text_field) => text_field.focused = focus,
            Entry::Text(text_field) => text_field.focused = focus,
        }
    }
}


// ====================================================================
//  RENDERING
// ====================================================================

impl UserOptions {
    pub fn render(&mut self, f: &mut Frame, _area: &Rect) {
        if !self.should_render { return; }

        let window_width = f.size().width;
        let text_area_width = (0.8 * (window_width as f32)) as u16;

        let window_height = f.size().height;
        let text_area_height = (0.8 * (window_height as f32)) as u16;

        let horizontal = Layout::horizontal([text_area_width]).flex(Flex::Center);
        let vertical = Layout::vertical([text_area_height]).flex(Flex::Center);
        let [rect] = vertical.areas(f.size());
        let [rect] = horizontal.areas(rect);
        self.rect = rect;
        self.max_height = rect.height - 2;

        // clear the rect
        f.render_widget(Clear, rect); //this clears out the background

        let block = Block::default()
            .title("User Settings:")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue))
            .title_style(Style::default().fg(Color::Blue)
                         .add_modifier(Modifier::BOLD));

        f.render_widget(block.clone(), rect);

        // update the offset
        while self.index < self.offset as i32 {
            self.offset -= 1;
        }
        while self.index > self.offset as i32 + self.max_height as i32 - 1 {
            self.offset += 1;
        }
        let mut num_rows = self.entries.len() - self.offset as usize;
        if num_rows > self.max_height as usize {
            num_rows = self.max_height as usize;
        }
        let constraints = (0..num_rows)
            .map(|_| Constraint::Length(1)).collect::<Vec<_>>();
        let rects = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(block.inner(rect));

        for (rect, entry) in rects.iter().zip(&mut self.entries[self.offset as usize..]) {
            match entry {
                Entry::Integer(text_field) => {
                    text_field.render(f, rect);
                },
                Entry::Boolean(text_field) => {
                    text_field.render(f, rect);
                },
                Entry::Text(text_field) => {
                    text_field.render(f, rect);
                },
            }
        }


    }
}


// ====================================================================
//  USER INPUT
// ====================================================================

impl UserOptions {
    /// Handle user input for the user settings window
    /// Always returns true (input is always handled)
    pub fn input(&mut self, _action: &mut Action, key_event: KeyEvent) -> bool {
        if !self.handle_input { return false; }

        match key_event.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter => {
                self.deactivate();
            },
            KeyCode::Down | KeyCode::Char('j') => {
                self.next();
            },
            KeyCode::Up | KeyCode::Char('k') => {
                self.previous();
            },
            
            _ => {}
        }
        true
    }
}


// ====================================================================
//  MOUSE INPUT
// ====================================================================

impl UserOptions {
    pub fn mouse_input(&mut self, 
                       _action: &mut Action, 
                       mouse_input: &mut MouseInput) {
        if !self.handle_input { return;}

        if let Some(mouse_event_kind) = mouse_input.kind() {

            match mouse_event_kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    // close the window if the user clicks outside of it
                    if !self.rect.contains(mouse_input.get_position()) {
                        self.deactivate();
                        mouse_input.click();
                    } 
                }
                // scrolling
                MouseEventKind::ScrollUp => {
                    self.previous();
                },
                MouseEventKind::ScrollDown => {
                    self.next();
                },
                _ => {}
            }
            // Set the mouse event to handled
            mouse_input.handled = true;
        }
    }
}
