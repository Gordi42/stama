use ratatui::{
    prelude::*,
    style::{Color, Style},
    widgets::*,
    layout::{Layout, Flex,},
};
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEventKind};
use crate::mouse_input::MouseInput;

use crate::app::Action;

#[derive(Clone, Debug)]
pub enum JobActions {
    Kill,
    OpenLog,
    OpenSubmission,
    GoWorkDir,
    SSH,
}

pub struct JobActionsMenu {
    pub should_render: bool,  // if the window should render
    pub handle_input: bool,   // if the window should handle input
    pub index: i32,
    pub state: ListState,
    pub actions: Vec<JobActions>,
    pub labels: Vec<String>,
    pub job_name: String,
    pub rect: Rect,
}

// ========================================================================
//  CONSTRUCTOR
// ========================================================================

impl JobActionsMenu {
    pub fn new() -> Self {
        let actions = vec![
            JobActions::Kill,
            JobActions::OpenLog,
            JobActions::OpenSubmission,
            JobActions::GoWorkDir,
            JobActions::SSH,
        ];
        let mut labels = vec![
            "Kill job".to_string(),
            "Open logfile".to_string(),
            "Open submission script".to_string(),
            "cd to working directory".to_string(),
            "ssh to node".to_string()];
        for (i, label) in labels.iter_mut().enumerate() {
            *label = format!("{}. {}", i + 1, label);
        }
        Self {
            should_render: false,
            handle_input: false,
            index: 0,
            state: ListState::default(),
            actions: actions,
            labels: labels,
            job_name: String::new(),
            rect: Rect::default(),
        }
    }
}

// ========================================================================
//  METHODS
// ========================================================================

impl JobActionsMenu {

    pub fn set_index(&mut self, index: i32) {
        let max_ind = self.actions.len() as i32 - 1;
        let mut new_index = index;
        if index > max_ind {
            new_index = 0;
        } else if index < 0 {
            new_index = max_ind;
        } 
        self.index = new_index;
        self.state.select(Some(self.index as usize));
    }

    fn next(&mut self) {
        self.set_index(self.index + 1);
    }

    fn previous(&mut self) {
        self.set_index(self.index - 1);
    }

    fn get_action(&self) -> JobActions {
        self.actions[self.index as usize].clone()
    }

    fn perform_action(&mut self, action: &mut Action) {
        *action = Action::JobOption(self.get_action());
        self.deactivate();
    }

    pub fn activate(&mut self, job_name: &str) {
        self.job_name = job_name.to_string();
        self.should_render = true;
        self.handle_input = true;
        self.set_index(0);
    }

    pub fn deactivate(&mut self) {
        self.should_render = false;
        self.handle_input = false;
    }

}

// ====================================================================
//  RENDERING
// ====================================================================

impl JobActionsMenu {
    pub fn render(&mut self, f: &mut Frame, _area: &Rect) {
        if !self.should_render { return; }

        let window_width = f.size().width;
        let text_area_width = (0.8 * (window_width as f32)) as u16;

        let horizontal = Layout::horizontal([text_area_width]).flex(Flex::Center);
        let vertical = Layout::vertical([self.labels.len() as u16 + 2])
            .flex(Flex::Center);
        let [rect] = vertical.areas(f.size());
        let [rect] = horizontal.areas(rect);

        self.rect = rect;
        // clear the area
        f.render_widget(Clear, rect);

        let title = format!("JOB ACTION: {}", self.job_name);

        let list = List::new(self.labels.clone())
            .block(Block::default()
                   .borders(Borders::ALL)
                   .title(block::Title::from(title)
                          .alignment(Alignment::Center))
                   .border_type(BorderType::Rounded)
                   .style(Style::default().fg(Color::Blue))
            )
            .highlight_style(Style::default().add_modifier(Modifier::BOLD)
                             .bg(Color::Blue).fg(Color::Black));

        f.render_stateful_widget(list, rect, &mut self.state);
    }
}

// ====================================================================
//  USER INPUT
// ====================================================================

impl JobActionsMenu {
    /// Handle user input for the job actions menu
    /// Always returns true (input is always handled)
    pub fn input(&mut self, _action: &mut Action, key_event: KeyEvent) -> bool {
        if !self.handle_input { return false; }

        match key_event.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('h') => {
                self.deactivate();
            },
            KeyCode::Down | KeyCode::Char('j') => {
                self.next();
            },
            KeyCode::Up | KeyCode::Char('k') => {
                self.previous();
            },
            KeyCode::Enter | KeyCode::Char('l') => {
                self.perform_action(_action);
            },
            KeyCode::Char('1') => {
                self.set_index(0);
            },
            KeyCode::Char('2') => {
                self.set_index(1);
            },
            KeyCode::Char('3') => {
                self.set_index(2);
            },
            KeyCode::Char('4') => {
                self.set_index(3);
            },
            KeyCode::Char('5') => {
                self.set_index(4);
            },
            
            _ => {}
        }
        true
    }
}


// ====================================================================
//  MOUSE INPUT
// ====================================================================

impl JobActionsMenu {
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
                    } else {
                        // find the index of the clicked item
                        let y = mouse_input.get_position().y - self.rect.y;
                        let y_min = 1;
                        let y_max = self.labels.len() as u16 + 1;
                        if y >= y_min && y < y_max{
                            self.set_index(y as i32 - 1);
                        }
                        if mouse_input.is_double_click() {
                            self.perform_action(_action);
                        }
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
