use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEventKind};
use ratatui::{
    layout::{Flex, Layout},
    prelude::*,
    style::{Color, Style},
    widgets::*,
};

use crate::{app::Action, mouse_input::MouseInput};

use crate::menus::OpenMenu;

use super::{entry_menu::EntryMenu, salloc_entry::SallocEntry};
use super::salloc_list::SallocList;

/// Which part of the menu is in focus
pub enum Focus {
    List,
    Entry,
}

/// Job Allocation Menu
///
/// Contains a list of editable presets
pub struct SallocMenu {
    should_render: bool,
    handle_input: bool,
    /// The rectangle where to render the menu (for mouse input)
    rect: Rect,
    /// The list of salloc entries
    salloc_list: SallocList<SallocEntry>,
    /// The entry info menu
    entry_menu: EntryMenu,
    /// The selection state
    state: ListState,
    /// Which part of the menu is in Focus
    focus: Focus,
}

// ====================================================================
//  CONSTRUCTOR
// ====================================================================

impl SallocMenu {
    pub fn new() -> SallocMenu {
        let salloc_list = SallocList::load(None)
            .unwrap_or_else(|_| SallocList::new());
        let mut salloc_menu = SallocMenu {
            should_render: false,
            handle_input: false,
            salloc_list,
            rect: Rect::default(),
            entry_menu: EntryMenu::new(None),
            state: ListState::default(),
            focus: Focus::List,
        };
        salloc_menu.set_index(0);
        salloc_menu
    }
}

// ====================================================================
//  METHODS
// ====================================================================

impl SallocMenu {
    /// Activate the menu
    pub fn activate(&mut self) {
        self.should_render = true;
        self.handle_input = true;
    }

    /// Deactivate the menu
    pub fn deactivate(&mut self) {
        self.should_render = false;
        self.handle_input = false;
        let _ = self.salloc_list.save(None);
    }

    /// Set the index of list state
    pub fn set_index(&mut self, index: i32) {
        let max_ind = self.salloc_list.len() as i32;
        let mut new_index = index;
        if index > max_ind {
            new_index = 0;
        } else if index < 0 {
            new_index = max_ind;
        }
        self.state.select(Some(new_index as usize));
        self.entry_menu = EntryMenu::new(self.get_salloc_entry());
    }

    /// Select the next salloc entry
    fn next(&mut self) {
        let index = self.state.selected();
        match index {
            Some(ind) => self.set_index(ind as i32 + 1),
            None => {}
        };
    }

    /// Select the previous salloc entry
    fn previous(&mut self) {
        let index = self.state.selected();
        match index {
            Some(ind) => self.set_index(ind as i32 - 1),
            None => {}
        };
    }

    /// Switch focus between list and entry
    fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::List => {
                self.entry_menu.is_active = true;
                Focus::Entry
            },
            Focus::Entry => {
                self.entry_menu.is_active = false;
                // save the entry (ignore errors)
                let _ = self.salloc_list.save(None);
                Focus::List
            },
        }
    }

    /// Get the currently selected Salloc Entry
    /// Returns None if no entry is selected
    fn get_salloc_entry(&self) -> Option<&SallocEntry> {
        let index = match self.state.selected() {
            Some(ind) => ind,
            None => return None
        };
        self.salloc_list.entries.get(index)
    }

    /// Create a new salloc entry
    fn create_new_salloc_entry(&mut self) {
        self.salloc_list.entries.push(SallocEntry::new());
        self.toggle_focus();
    }

    /// set the current entry to the given entry
    fn set_entry(&mut self, entry: SallocEntry) {
        let index = match self.state.selected() {
            Some(ind) => ind,
            None => return
        };
        self.salloc_list.entries[index] = entry;
    }

}

// ====================================================================
//  RENDERING
// ====================================================================

impl SallocMenu {
    /// Render the full salloc menu
    /// This is the main render function
    pub fn render(&mut self, f: &mut Frame, _area: &Rect) {
        if !self.should_render {
            return;
        }

        let window_width = f.size().width;
        let text_area_width = (0.8 * (window_width as f32)) as u16;

        let window_height = f.size().height;
        let text_area_height = (0.8 * (window_height as f32)) as u16;

        let horizontal = Layout::horizontal([text_area_width]).flex(Flex::Center);
        let vertical = Layout::vertical([text_area_height]).flex(Flex::Center);
        let [rect] = vertical.areas(f.size());
        let [rect] = horizontal.areas(rect);
        self.rect = rect;

        // clear the rect
        f.render_widget(Clear, rect); //this clears out the background

        let block = Block::default()
            .title(block::Title::from(" SALLOC ").alignment(Alignment::Center))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue))
            .title_style(
                Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            );

        f.render_widget(block.clone(), rect);

        // create two columns:
        //  - left column: list of salloc entries
        //  - right column: the selected salloc entry
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(block.inner(rect));

        self.render_list(f, &layout[0]);
        self.render_entry(f, &layout[1]);
    }

    /// Render the list of salloc entries
    /// This renders the left column
    fn render_list(&mut self, f: &mut Frame, area: &Rect) {
        let mut items: Vec<ListItem> = self
            .salloc_list
            .entries
            .iter()
            .map(|entry| ListItem::new(entry.preset_name.clone()))
            .collect();
        items.push(ListItem::new("Create new".to_string()));

        let highlight_style = match self.focus {
            Focus::List => Style::default().fg(Color::Blue),
            Focus::Entry => Style::default(),
        };
        let control_hint = match self.focus {
            Focus::List => "",
            Focus::Entry => "<tab>"
        };


        let list = List::new(items)
            .block(
                Block::default()
                    .title("Presets:")
                    .title(block::Title::from(control_hint)
                           .alignment(Alignment::Right))
                    .borders(Borders::ALL)
                    .border_style(highlight_style),
            )
            .highlight_style(Style::default().add_modifier(Modifier::BOLD))
            .highlight_style(highlight_style.reversed());


        f.render_stateful_widget(list, *area, &mut self.state);
    }

    /// Render the selected salloc entry
    /// This renders the right column
    fn render_entry(&mut self, f: &mut Frame, area: &Rect) {
        let highlight_style = match self.focus {
            Focus::List => Style::default(),
            Focus::Entry => Style::default().fg(Color::Blue),
        };
        let control_hint = match self.focus {
            Focus::List => "<tab>",
            Focus::Entry => ""
        };

        let block = Block::default()
                    .title("Settings:")
                    .title(block::Title::from(control_hint)
                           .alignment(Alignment::Right))
                    .borders(Borders::ALL)
                    .border_style(highlight_style);

        f.render_widget(block.clone(), *area);
        self.entry_menu.render(f, &block.inner(*area));


    }

}

// ====================================================================
//  USER INPUT
// ====================================================================

impl SallocMenu {
    /// Handle user input for the user settings window
    /// Always return true (no input is passed to windows below)
    pub fn input(&mut self, action: &mut Action, key_event: KeyEvent) -> bool {
        if !self.handle_input {
            return false;
        }
        
        match key_event.code {
            KeyCode::Tab => {
                self.toggle_focus();
                return true
            }
            _ => {}
        }

        match self.focus {
            Focus::List => self.input_list(action, key_event),
            Focus::Entry => self.input_entry(action, key_event),
        }
    }

    /// Handle user input for the list window
    /// Always return true (no input is passed to windows below)
    pub fn input_list(&mut self, action: &mut Action, key_event: KeyEvent) -> bool {
        match key_event.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                *action = Action::UpdateUserOptions;
                self.deactivate();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.next();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.previous();
            }
            KeyCode::Enter => {
                match self.get_salloc_entry() {
                    Some(entry) => {
                        let cmd = entry.start();
                        *action = Action::StartSalloc(cmd);
                        self.deactivate();
                    }
                    None => {
                        self.create_new_salloc_entry();
                    }
                }
            }
            KeyCode::Char('?') => {
                *action = Action::OpenMenu(OpenMenu::Help(2));
            }

            _ => {}
        }
        true
    }

    /// Handle user input for the list window
    /// Always return true (no input is passed to windows below)
    pub fn input_entry(&mut self, action: &mut Action, key_event: KeyEvent) -> bool {
        // first handle the input for the entry menu
        let status = self.entry_menu.input(action, key_event);
        // if the input was handled, return true
        if status {
            let new_entry = self.entry_menu.get_entry();
            self.set_entry(new_entry);
            return true;
        }
        // else check for other key events
        match key_event.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                *action = Action::UpdateUserOptions;
                self.deactivate();
                return true;
            }
            KeyCode::Char('?') => {
                *action = Action::OpenMenu(OpenMenu::Help(2));
                return true;
            }

            _ => {}
        }

        true
    }

}

// ====================================================================
//  MOUSE INPUT
// ====================================================================

impl SallocMenu {
    pub fn mouse_input(&mut self, _action: &mut Action, mouse_input: &mut MouseInput) {
        if !self.handle_input {
            return;
        }

        if let Some(mouse_event_kind) = mouse_input.kind() {
            match mouse_event_kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    // close the window if the user clicks outside of it
                    if !self.rect.contains(mouse_input.get_position()) {
                        self.deactivate();
                        mouse_input.click();
                        return;
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
            // Set the mouse event to handled
            mouse_input.handled = true;
        }
    }
}
