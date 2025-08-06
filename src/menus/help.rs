use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEventKind};
use ratatui::{
    layout::{Flex, Layout},
    prelude::*,
    style::{Color, Style},
    widgets::*,
};

use crate::app::Action;
use crate::mouse_input::MouseInput;

// ====================================================================
//                         HELP MENU
// ====================================================================
// # Category
//   - short | long

#[derive(Debug, Clone)]
pub struct HelpEntry {
    pub short: String,
    pub long: String,
}

impl HelpEntry {
    pub fn new(short: &str, long: &str) -> Self {
        Self {
            short: short.to_string(),
            long: long.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct HelpCategory {
    pub title: String,
    pub entries: Vec<HelpEntry>,
}

impl HelpCategory {
    pub fn new(title: &str, entries: Vec<HelpEntry>) -> Self {
        Self {
            title: title.to_string(),
            entries,
        }
    }
}

#[derive(Debug, Clone)]
pub struct HelpMenu {
    pub should_render: bool,
    pub handle_input: bool,
    pub rect: Rect,
    pub categories: Vec<HelpCategory>,
    pub offset: usize,
}

// ====================================================================
//  CONSTRUCTOR
// ====================================================================

impl HelpMenu {
    pub fn new() -> Self {
        // job overview category
        let job_overview_entries = vec![
            HelpEntry::new("Down/Up (j/k)", "Next/Previous job"),
            HelpEntry::new("Enter (l)", "Open job actions menu"),
            HelpEntry::new("tab", "Select next sorting category"),
            HelpEntry::new("r", "Reverse sorting order"),
            HelpEntry::new("1", "Toggle job details"),
            HelpEntry::new("2", "Toggle log"),
            HelpEntry::new("a", "Open allocation menu"),
            HelpEntry::new("o", "Open stama settings menu"),
            HelpEntry::new("/", "Modify job list filter"),
            HelpEntry::new("m", "Minimize/Maximize top section"),
        ];
        let job_overview = HelpCategory::new("Job Overview", job_overview_entries);
        // job actions category
        let job_actions_entries = vec![
            HelpEntry::new("Down/Up (j/k)", "Next/Previous action"),
            HelpEntry::new("Enter (l)", "Execute action"),
            HelpEntry::new("Esc", "Close action menu"),
            HelpEntry::new("1-5", "Select action"),
        ];
        let job_actions = HelpCategory::new("Job Actions", job_actions_entries);
        // allocation menu category
        let allocation_menu_entries = vec![
            HelpEntry::new("Esc (q)", "Close allocation menu"),
            HelpEntry::new("Tab", "Switch focus between presets and settings panes"),
            HelpEntry::new("Down/Up (j/k)", "Next/Previous entry"),
            HelpEntry::new(
                "Enter",
                "If Presets is focused => Execute salloc command
                           If Settings is focused => Edit setting",
            ),
            HelpEntry::new("d", "Delete the selected preset"),
        ];
        let allocation_menu = HelpCategory::new("Allocation Menu", allocation_menu_entries);
        // stama settings category
        let stama_settings_entries = vec![
            HelpEntry::new("Down/Up (j/k)", "Next/Previous setting"),
            HelpEntry::new("Enter (l)", "Open setting actions menu"),
            HelpEntry::new("Esc", "Close setting menu"),
        ];
        let stama_settings = HelpCategory::new("Stama Settings", stama_settings_entries);
        // info category
        let version: &str = env!("CARGO_PKG_VERSION");
        let info_entries = vec![
            HelpEntry::new("name", "Slurm Task Manager (stama)"),
            HelpEntry::new("version", version),
            HelpEntry::new("author", "Silvano Rosenau"),
        ];
        let info = HelpCategory::new("Info", info_entries);

        let categories = vec![
            job_overview,
            job_actions,
            allocation_menu,
            stama_settings,
            info,
        ];

        Self {
            should_render: false,
            handle_input: false,
            rect: Rect::default(),
            categories,
            offset: 0,
        }
    }
}

// ====================================================================
//  METHODS
// ====================================================================

impl HelpMenu {
    pub fn open(&mut self, _selected_category: usize) {
        self.should_render = true;
        self.handle_input = true;
        // set offset
        let category_lengths = self
            .categories
            .iter()
            .map(|c| c.entries.len())
            .collect::<Vec<usize>>();
        self.offset = 0;
        for (index, length) in category_lengths.iter().enumerate() {
            if index == _selected_category {
                break;
            }
            self.offset += length;
        }
    }

    pub fn close(&mut self) {
        self.should_render = false;
        self.handle_input = false;
    }

    pub fn scroll_down(&mut self) {
        self.offset += 1;
        // check if the offset is out of bounds
        let total_entries = self
            .categories
            .iter()
            .map(|c| c.entries.len())
            .sum::<usize>();
        self.offset = self.offset.min(total_entries);
    }

    pub fn scroll_up(&mut self) {
        self.offset = self.offset.saturating_sub(1);
    }
}

// ====================================================================
//  RENDERING
// ====================================================================

impl HelpMenu {
    pub fn render(&mut self, f: &mut Frame, _area: &Rect) {
        if !self.should_render {
            return;
        }

        let window_width = f.area().width;
        let text_area_width = (0.8 * (window_width as f32)) as u16;

        let window_height = f.area().height;
        let text_area_height = (1.0 * (window_height as f32)) as u16;

        let horizontal = Layout::horizontal([text_area_width]).flex(Flex::Center);
        let vertical = Layout::vertical([text_area_height]).flex(Flex::Center);
        let [rect] = vertical.areas(f.area());
        let [rect] = horizontal.areas(rect);
        self.rect = rect;

        // clear the rect
        f.render_widget(Clear, rect); //this clears out the background

        // Render the border
        let block = Block::default()
            .title_top(Line::from("HELP:").alignment(Alignment::Left))
            .title_top(Line::from("<esc> to close").alignment(Alignment::Right))
            .borders(Borders::ALL)
            .border_type(BorderType::Thick)
            .border_style(Style::default().fg(Color::Blue))
            .title_style(
                Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            );

        f.render_widget(block.clone(), rect);

        // Render the categories
        // Get the width of the description column
        let desc_width = text_area_width * 2 / 3;
        // create paragraphs for desctiptions
        let mut index = 0;
        let mut render_rect = block.inner(rect);
        for category in self.categories.iter() {
            if render_rect.height == 0 {
                break;
            }

            // check if the category title should be rendered
            if index >= self.offset {
                let block = Block::default()
                    .title_top(Line::from(category.title.clone()).alignment(Alignment::Center))
                    .borders(Borders::TOP)
                    .border_style(Style::default().fg(Color::Yellow))
                    .title_style(Style::default().fg(Color::Yellow));

                let mut title_rect = render_rect.clone();
                title_rect.height = 1;
                f.render_widget(block, title_rect);
                render_rect.y += 1;
                render_rect.height = render_rect.height.saturating_sub(1);
            }

            for entry in category.entries.iter() {
                if render_rect.height == 0 {
                    break;
                }
                if index < self.offset {
                    index += 1;
                    continue;
                }

                let key = Paragraph::new(format!("{}: ", entry.short.clone()))
                    .style(Style::default())
                    .alignment(Alignment::Right);

                let description = Paragraph::new(entry.long.clone())
                    .style(Style::default())
                    .alignment(Alignment::Left)
                    .wrap(Wrap { trim: true });

                let desc_height: u16 =
                    (description.line_count(desc_width) as u16).min(render_rect.height);
                let mut entry_rect = render_rect.clone();
                entry_rect.height = desc_height;
                render_rect.y += desc_height;
                render_rect.height = render_rect.height.saturating_sub(desc_height);

                let layout = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Min(1), Constraint::Length(desc_width)].as_ref())
                    .split(entry_rect);

                f.render_widget(key, layout[0]);
                f.render_widget(description, layout[1]);

                index += 1;
            }
        }
    }
}

// ====================================================================
//  USER INPUT
// ====================================================================

impl HelpMenu {
    /// Handle user input for the message window
    /// Always returns true (input is always handled)
    pub fn input(&mut self, _action: &mut Action, _key_event: KeyEvent) -> bool {
        if !self.handle_input {
            return false;
        }

        match _key_event.code {
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') | KeyCode::Char('?') => {
                self.close();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.scroll_down();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.scroll_up();
            }
            _ => {}
        }

        true
    }
}

// ====================================================================
//  MOUSE INPUT
// ====================================================================

impl HelpMenu {
    pub fn mouse_input(&mut self, _action: &mut Action, mouse_input: &mut MouseInput) {
        if !self.handle_input {
            return;
        }

        if let Some(mouse_event_kind) = mouse_input.kind() {
            match mouse_event_kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    if !self.rect.contains(mouse_input.get_position()) {
                        self.should_render = false;
                        self.handle_input = false;
                    }
                }
                MouseEventKind::ScrollDown => {
                    self.scroll_down();
                }
                MouseEventKind::ScrollUp => {
                    self.scroll_up();
                }
                _ => {}
            }
            // Set the mouse event to handled
            mouse_input.click();
        }
    }
}
