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

use crate::text_field::{TextField, TextFieldType};
use crate::app::Action;
use crate::mouse_input::MouseInput;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct UserOptionsList {
    pub refresh_rate: usize,          // Refresh rate in milliseconds
    pub show_completed_jobs: bool,  // Show completed jobs
    pub confirm_before_quit: bool,  // Confirm before quitting
    pub confirm_before_kill: bool,  // Confirm before killing a job
    pub external_editor: String,    // External editor command (e.g. "vim")
    pub dummy_jobs: bool,            // if dummy jobs should be created
}

impl Default for UserOptionsList {
    fn default() -> Self {
        Self {
            refresh_rate: 250,
            show_completed_jobs: true,
            confirm_before_quit: false,
            confirm_before_kill: true,
            external_editor: "vim".to_string(),
            dummy_jobs: false,
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






pub struct UserOptions {
    pub should_render: bool,
    pub handle_input: bool,
    pub rect: Rect,
    pub entries: Vec<TextField>,
    pub index: i32,
    pub state: ListState,
    pub offset: u16,
    pub max_height: u16,
    pub rects: Vec<Rect>,
}

// ====================================================================
//  CONSTRUCTOR
// ====================================================================

impl UserOptions {
    pub fn from_list(list: UserOptionsList) -> Self {
        let list = list.clone();

        let entries = vec![
            TextField::new(
                "Refresh rate (ms)", 
                TextFieldType::Integer(list.refresh_rate)),
            TextField::new(
                "Show completed jobs", 
                TextFieldType::Boolean(list.show_completed_jobs)),
            TextField::new(
                "Confirm before quitting", 
                TextFieldType::Boolean(list.confirm_before_quit)),
            TextField::new(
                "Confirm before killing a job", 
                TextFieldType::Boolean(list.confirm_before_kill)),
            TextField::new(
                "External editor", 
                TextFieldType::Text(list.external_editor)),
            TextField::new(
                "Create dummy jobs", 
                TextFieldType::Boolean(list.dummy_jobs)),
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
            rects: vec![],
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
        user_options.refresh_rate = match &self.entries[0].field_type {
            TextFieldType::Integer(u) => *u,
            _ => 250,
        };
        user_options.show_completed_jobs = match &self.entries[1].field_type {
            TextFieldType::Boolean(b) => *b,
            _ => true,
        };
        user_options.confirm_before_quit = match &self.entries[2].field_type {
            TextFieldType::Boolean(b) => *b,
            _ => false,
        };
        user_options.confirm_before_kill = match &self.entries[3].field_type {
            TextFieldType::Boolean(b) => *b,
            _ => true,
        };
        user_options.external_editor = match &self.entries[4].field_type {
            TextFieldType::Text(s) => s.clone(),
            _ => "vim".to_string(),
        };
        user_options.dummy_jobs = match &self.entries[5].field_type {
            TextFieldType::Boolean(b) => *b,
            _ => false,
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
        self.entries[index].focused = focus;
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
            .title(block::Title::from("USER SETTINGS:").alignment(Alignment::Center))
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
        self.rects = rects.to_vec();

        for (rect, entry) in rects.iter().zip(&mut self.entries[self.offset as usize..]) {
            entry.render(f, rect);
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

        // check if the user is typing in a text field
        let entry = &mut self.entries[self.index as usize];
        if entry.active {
            match key_event.code {
                _ => {
                    entry.input(key_event);
                }
            }
            return true;
        }

        match key_event.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('o') => {
                self.deactivate();
            },
            KeyCode::Down | KeyCode::Char('j') => {
                self.next();
            },
            KeyCode::Up | KeyCode::Char('k') => {
                self.previous();
            },
            KeyCode::Enter | KeyCode::Char('i') => {
                self.entries[self.index as usize].on_enter();
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

            // check if the user is editing a text field
            let entry = &mut self.entries[self.index as usize];
            if entry.active {
                return;
            }

            match mouse_event_kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    // close the window if the user clicks outside of it
                    if !self.rect.contains(mouse_input.get_position()) {
                        self.deactivate();
                        mouse_input.click();
                        return;
                    } 
                    // check if the user clicked on a text field
                    for (i, rect) in self.rects.iter().enumerate() {
                        if rect.contains(mouse_input.get_position()) {
                            self.set_index(i as i32 + self.offset as i32);
                            // check if the click is a double click
                            if mouse_input.is_double_click() {
                                self.entries[self.index as usize].on_enter();
                            }
                            mouse_input.click();
                            return;
                        }
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
