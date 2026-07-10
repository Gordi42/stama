use crate::mouse_input::MouseInput;
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEventKind};
use ratatui::{
    prelude::*,
    style::{Color, Style},
    widgets::*,
};

use crate::app::Action;
use crate::job::Job;
use crate::menus::help::HelpContext;
use crate::menus::{centered_popup, wrap_index, Menu, OpenMenu, PopupSize};

#[derive(Clone, Debug)]
pub enum JobActions {
    Kill(Job),
    KillConfirmed(Job),
    OpenLog(Job),
    OpenSubmission(Job),
    GoWorkDir(Job),
    SSH(Job),
}

pub struct JobActionsMenu {
    open: bool, // if the menu is open (rendered and handling input)
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

impl Default for JobActionsMenu {
    fn default() -> Self {
        Self::new()
    }
}

impl JobActionsMenu {
    pub fn new() -> Self {
        let job = Job::default();
        let actions = vec![
            JobActions::Kill(job.clone()),
            JobActions::OpenLog(job.clone()),
            JobActions::OpenSubmission(job.clone()),
            JobActions::GoWorkDir(job.clone()),
            JobActions::SSH(job.clone()),
        ];
        let mut labels = vec![
            "Kill job".to_string(),
            "Open logfile".to_string(),
            "Open submission script".to_string(),
            "cd to working directory".to_string(),
            "ssh to node".to_string(),
        ];
        for (i, label) in labels.iter_mut().enumerate() {
            *label = format!("{}. {}", i + 1, label);
        }
        Self {
            open: false,
            index: 0,
            state: ListState::default(),
            actions,
            labels,
            job_name: String::new(),
            rect: Rect::default(),
        }
    }
}

// ========================================================================
//  METHODS
// ========================================================================

impl JobActionsMenu {
    pub fn set_job(&mut self, job: Job) {
        self.actions = vec![
            JobActions::Kill(job.clone()),
            JobActions::OpenLog(job.clone()),
            JobActions::OpenSubmission(job.clone()),
            JobActions::GoWorkDir(job.clone()),
            JobActions::SSH(job.clone()),
        ];
        self.job_name = job.get_jobname();
    }

    pub fn set_index(&mut self, index: i32) {
        self.index = wrap_index(index as isize, 0, self.actions.len()) as i32;
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
        *action = Action::JobOption(Box::new(self.get_action()));
        self.deactivate();
    }

    pub fn activate(&mut self, job: &Job) {
        self.set_job(job.clone());
        self.open = true;
        self.set_index(0);
    }

    pub fn deactivate(&mut self) {
        self.open = false;
    }
}

// ====================================================================
//  MENU TRAIT (RENDERING + INPUT)
// ====================================================================

impl Menu for JobActionsMenu {
    fn is_open(&self) -> bool {
        self.open
    }

    fn render(&mut self, f: &mut Frame, _area: &Rect) {
        let rect = centered_popup(
            f.area(),
            PopupSize::Fraction(0.8),
            PopupSize::Fixed(self.labels.len() as u16 + 2),
        );
        self.rect = rect;
        // clear the area
        f.render_widget(Clear, rect);

        let title = format!("JOB ACTION: {}", self.job_name);

        let list = List::new(self.labels.clone())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title_top(Line::from(title).alignment(Alignment::Center))
                    .border_type(BorderType::Rounded)
                    .style(Style::default().fg(Color::Blue)),
            )
            .highlight_style(
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .bg(Color::Blue)
                    .fg(Color::Black),
            );

        f.render_stateful_widget(list, rect, &mut self.state);
    }

    /// Handle user input for the job actions menu
    /// Always returns true (input is always consumed)
    fn input(&mut self, action: &mut Action, key_event: KeyEvent) -> bool {
        match key_event.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('h') => {
                self.deactivate();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.next();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.previous();
            }
            KeyCode::Enter | KeyCode::Char('l') => {
                self.perform_action(action);
            }
            KeyCode::Char('?') => {
                *action = Action::OpenMenu(OpenMenu::Help(HelpContext::JobActions));
            }
            KeyCode::Char('1') => {
                self.set_index(0);
                self.perform_action(action);
            }
            KeyCode::Char('2') => {
                self.set_index(1);
                self.perform_action(action);
            }
            KeyCode::Char('3') => {
                self.set_index(2);
                self.perform_action(action);
            }
            KeyCode::Char('4') => {
                self.set_index(3);
                self.perform_action(action);
            }
            KeyCode::Char('5') => {
                self.set_index(4);
                self.perform_action(action);
            }

            _ => {}
        }
        true
    }

    fn mouse_input(&mut self, _action: &mut Action, mouse_input: &mut MouseInput) {
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
                        if y >= y_min && y < y_max {
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
