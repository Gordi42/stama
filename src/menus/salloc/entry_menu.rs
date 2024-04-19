use ratatui::{
    prelude::*,
    widgets::*,
    layout::Layout,
};
use crossterm::event::{
    KeyCode, KeyEvent, MouseButton, MouseEventKind};

use crate::mouse_input::MouseInput;
use crate::text_field::{TextField, TextFieldType};
use crate::app::Action;

use super::salloc_entry::SallocEntry;



pub struct EntryMenu {
    pub is_active: bool,
    pub entries: Vec<TextField>,
    pub is_new: bool,
    pub index: i32,
    pub state: ListState,
    pub offset: u16,
    pub rect: Rect,
    pub max_height: u16,
    pub rects: Vec<Rect>,
}


// ====================================================================
//  CONSTRUCTOR
// ====================================================================

impl EntryMenu {
    pub fn new(entry: Option<&SallocEntry>) -> Self {
        let is_new: bool;
        let entry = match entry {
            Some(entry) => {
                is_new = false;
                entry.clone()
            },
            None => {
                is_new = true;
                SallocEntry::new()
            }
        };

        let mut entries = vec![
            TextField::new(
                "Preset Name", 
                TextFieldType::Text(entry.preset_name)),
            TextField::new(
                "Account", 
                TextFieldType::Text(entry.account)),
            TextField::new(
                "Partition", 
                TextFieldType::Text(entry.partition)),
            TextField::new(
                "Nodes", 
                TextFieldType::Text(entry.nodes)),
            TextField::new(
                "Tasks per Node", 
                TextFieldType::Text(entry.cpus_per_node)),
            TextField::new(
                "Memory", 
                TextFieldType::Text(entry.memory)),
            TextField::new(
                "Time Limit", 
                TextFieldType::Text(entry.time_limit)),
            TextField::new(
                "Other Options", 
                TextFieldType::Text(entry.other_options)),
        ];

        entries[0].focused = true;

        Self {
            is_active: false,
            entries,
            is_new,
            index: 0,
            state: ListState::default(),
            offset: 0,
            rect: Rect::default(),
            max_height: 0,
            rects: vec![],
        }
    }
}

// ====================================================================
//  METHODS
// ====================================================================

impl EntryMenu {
    pub fn set_index(&mut self, index: i32) {
        self.entries[self.index as usize].active = false;
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

    pub fn get_entry(&self) -> SallocEntry {
        let preset_name = match &self.entries[0].field_type {
            TextFieldType::Text(s) => s.clone(),
            _ => "error".to_string(),
        };
        let account = match &self.entries[1].field_type {
            TextFieldType::Text(s) => s.clone(),
            _ => "error".to_string(),
        };
        let partition = match &self.entries[2].field_type {
            TextFieldType::Text(s) => s.clone(),
            _ => "error".to_string(),
        };
        let nodes = match &self.entries[3].field_type {
            TextFieldType::Text(s) => s.clone(),
            _ => "error".to_string(),
        };
        let cpus_per_node = match &self.entries[4].field_type {
            TextFieldType::Text(s) => s.clone(),
            _ => "error".to_string(),
        };
        let memory = match &self.entries[5].field_type {
            TextFieldType::Text(s) => s.clone(),
            _ => "error".to_string(),
        };
        let time_limit = match &self.entries[6].field_type {
            TextFieldType::Text(s) => s.clone(),
            _ => "error".to_string(),
        };
        let other_options = match &self.entries[7].field_type {
            TextFieldType::Text(s) => s.clone(),
            _ => "error".to_string(),
        };
        SallocEntry {
            preset_name,
            account,
            partition,
            nodes,
            cpus_per_node,
            memory,
            time_limit,
            other_options,
        }
    }
}


// ====================================================================
//  RENDERING
// ====================================================================

impl EntryMenu {
    pub fn render(&mut self, f: &mut Frame, area: &Rect) {
        self.rect = *area;
        self.max_height = area.height.saturating_sub(0);
        self.set_focus(self.index as usize, self.is_active);

        // clear the rect
        f.render_widget(Clear, *area); //this clears out the background

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
            .split(*area);
        self.rects = rects.to_vec();

        for (rect, entry) in rects.iter().zip(&mut self.entries[self.offset as usize..]) {
            entry.render(f, rect);
        }


    }
}

// ====================================================================
//  USER INPUT
// ====================================================================

impl EntryMenu {
    /// Handle user input for the EntryMenu
    /// Returns true if the input was handled, false otherwise.
    pub fn input(&mut self, action: &mut Action, key_event: KeyEvent) -> bool {
        // check if the user is typing in a text field
        let entry = &mut self.entries[self.index as usize];
        if entry.active {
            match key_event.code {
                _ => {
                    entry.input(key_event, action);
                }
            }
            return true;
        }

        match key_event.code {
            KeyCode::Down | KeyCode::Char('j') => {
                self.next();
                return true;
            },
            KeyCode::Up | KeyCode::Char('k') => {
                self.previous();
                return true;
            },
            KeyCode::Enter | KeyCode::Char('i') | KeyCode::Char(' ') => {
                self.entries[self.index as usize].on_enter();
                return true;
            },
            _ => {}
        }
        false
    }
}

// ====================================================================
//  MOUSE INPUT
// ====================================================================

impl EntryMenu {
    pub fn mouse_input(&mut self, _action: &mut Action, mouse_input: &mut MouseInput) {
        // check if the text edit is not active
        if self.entries[self.index as usize].active {
            return;
        }
        // first update the focused window pane
        if let Some(mouse_event_kind) = mouse_input.kind() {
            match mouse_event_kind {
                MouseEventKind::Down(MouseButton::Left) => {
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
                }
                MouseEventKind::ScrollDown => {
                    self.next();
                }
                _ => {}
            }
        };
    }
}
