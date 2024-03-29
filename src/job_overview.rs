use ratatui::{
    prelude::*,
    style::{Color, Style},
    widgets::*,
};
use crossterm::event::{KeyCode, KeyEvent};

use crate::app::Action;
use crate::message::Message;


pub enum WindowFocus {
    JobDetails,
    Log,
}

pub struct JobOverview {
    pub should_render: bool,  // if the window should render
    pub handle_input: bool,   // if the window should handle input
    pub search_args: String,  // the search arguments for squeue
    pub minimized: bool,      // if the job list is minimized
    pub focus: WindowFocus,   // which part of the window is in focus
}

// ====================================================================
//  CONSTRUCTOR
// ====================================================================

impl JobOverview {
    pub fn new() -> Self {
        Self {
            should_render: true,
            handle_input: true,
            search_args: "-U u301533".to_string(),
            minimized: false,
            focus: WindowFocus::JobDetails,
        }
    }
}


// ====================================================================
//  RENDERING
// ====================================================================

impl JobOverview {
    pub fn render(&self, f: &mut Frame, area: &Rect) {
        // only render if the window is active
        if !self.should_render { return; }

        let mut constraints = vec![Constraint::Length(1)];
        if self.minimized {
            constraints.push(Constraint::Length(1));
        } else {
            constraints.push(Constraint::Percentage(30));
        }
        constraints.push(Constraint::Min(1));

        // create a layout for the title
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints.as_slice())
            .split(*area);

        // render the title, job list, and job details
        self.render_title(f, &layout[0]);
        self.render_joblist(f, &layout[1]);
        self.render_details(f, &layout[2]);
    }

    fn render_title(&self, f: &mut Frame, area: &Rect) {
        f.render_widget(
            Paragraph::new("JOB OVERVIEW")
                .style(Style::default().fg(Color::Red))
                .alignment(Alignment::Center),
            *area,
        );
    }

    fn render_joblist(&self, f: &mut Frame, area: &Rect) {
        if self.minimized {
            let paragraph = Paragraph::new("▶ Job: ")
                .style(Style::default().fg(Color::Gray));
            f.render_widget(paragraph, *area);
            return;
        }

        let title = format!("▼ Job list: squeue {}", self.search_args);
        
        let block = Block::default().title(title)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded);

        f.render_widget(block.clone(), *area);
        let _ = block.inner(*area);
    }

    fn render_details(&self, f: &mut Frame, area: &Rect) {
        let mut title = vec!{
            Span::raw("1. Job details"), 
            Span::raw("  "),
            Span::raw("2. Log"),
        };

        match self.focus {
            WindowFocus::JobDetails => {
                title[0] = Span::styled("1. Job details", 
                                        Style::default().fg(Color::Blue));
            },
            WindowFocus::Log => {
                title[2] = Span::styled("2. Log", 
                                        Style::default().fg(Color::Blue));
            },
        }
        
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded);

        f.render_widget(block.clone(), *area);
        let _ = block.inner(*area);
    }
}


// ====================================================================
//  USER INPUT
// ====================================================================

impl JobOverview {
    /// Handle user input for the job overview window
    /// Returns true if the input was handled
    /// Returns false if the input was not handled
    pub fn input(&mut self, action: &mut Action, key_event: KeyEvent)
        -> bool {
        if !self.handle_input { return false; }
        match key_event.code {
            // Escaping the program
            KeyCode::Char('q') => {
                *action = Action::Quit;
            },
            // Next / Previous job
            KeyCode::Down | KeyCode::Char('j') => {
                let message = Message::new("Next job not implemented yet");
                *action = Action::OpenMessage(message);
                self.next_job();
            },
            KeyCode::Up | KeyCode::Char('k') => {
                let message = Message::new("Previous job not implemented yet");
                *action = Action::OpenMessage(message);
                self.prev_job();
            },
            // Open job action menu
            KeyCode::Enter | KeyCode::Char('l') => {
                *action = Action::OpenJobAction;
            },
            // Switching focus between job details and log
            KeyCode::Char('1') => {
                self.focus = WindowFocus::JobDetails;
            },
            KeyCode::Char('2') => {
                self.focus = WindowFocus::Log;
            },
            KeyCode::Right => {
                self.next_focus();
            },
            KeyCode::Left => {
                self.prev_focus();
            },
            // Open job allocation menu
            KeyCode::Char('a') => {
                *action = Action::OpenJobAllocation;
            },
            // Minimizing/Maximizing the joblist
            KeyCode::Char('m') => {
                self.minimized = !self.minimized;
            },
            _ => {return false;},
        };
        true
    }

    fn next_focus(&mut self) {
        match self.focus {
            WindowFocus::JobDetails => {
                self.focus = WindowFocus::Log;
            },
            WindowFocus::Log => {
                self.focus = WindowFocus::JobDetails;
            },
        }
    }

    fn prev_focus(&mut self) {
        match self.focus {
            WindowFocus::JobDetails => {
                self.focus = WindowFocus::Log;
            },
            WindowFocus::Log => {
                self.focus = WindowFocus::JobDetails;
            },
        }
    }

    fn next_job(&mut self) {
    }

    fn prev_job(&mut self) {
    }
}
