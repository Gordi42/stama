use ratatui::{
    prelude::*,
    style::{Color, Style},
    widgets::*,
};
use crossterm::event::{KeyCode, KeyEvent};

use crate::app::Action;
use crate::job::{Job, JobStatus};


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
    pub joblist: Vec<Job>,    // the list of jobs
    pub index: usize,         // the index of the job in focus
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
            joblist: vec![],
            index: 0,
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

        let id_list = self.joblist.iter()
            .map(|job| job.id.to_string()).collect::<Vec<String>>();
        let id_len: u16 = id_list.iter()
            .map(|id| id.len()).max().unwrap_or(0)
            .max(8) as u16;

        let name_list = self.joblist.iter()
            .map(|job| job.name.clone()).collect::<Vec<String>>();
        let name_len: u16 = name_list.iter()
            .map(|name| name.len()).max().unwrap_or(0)
            .max(10) as u16;

        let status_list = self.joblist.iter()
            .map(|job| match job.status {
                JobStatus::Unknown => "Unknown",
                JobStatus::Running => "Running",
                JobStatus::Pending => "Pending",
                JobStatus::Completed => "Completed",
                JobStatus::Failed => "Failed",
            }).collect::<Vec<&str>>();
        let status_len: u16 = status_list.iter()
            .map(|status| status.len()).max().unwrap_or(0)
            .max(11) as u16;

        let time_list = self.joblist.iter()
            .map(|job| {
                let hours = job.time / 3600;
                let minutes = (job.time % 3600) / 60;
                let seconds = job.time % 60;
                format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
            }).collect::<Vec<String>>();
        let time_len: u16 = time_list.iter()
            .map(|time| time.len()).max().unwrap_or(0)
            .max(10) as u16;

        let partition_list = self.joblist.iter()
            .map(|job| job.partition.clone()).collect::<Vec<String>>();
        let partition_len: u16 = partition_list.iter()
            .map(|partition| partition.len()).max().unwrap_or(0)
            .max(11) as u16;

        let nodes_list = self.joblist.iter()
            .map(|job| job.nodes.to_string()).collect::<Vec<String>>();
        let nodes_len: u16 = nodes_list.iter()
            .map(|nodes| nodes.len()).max().unwrap_or(0)
            .max(7) as u16;

        // create a layout for the job list
        let list_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(id_len),
                          Constraint::Min(name_len),
                          Constraint::Min(status_len),
                          Constraint::Min(time_len),
                          Constraint::Min(partition_len),
                          Constraint::Min(nodes_len)
            ].as_ref())
            .split(block.inner(*area));

        // render the job list
        let id_column = List::new(id_list)
            .highlight_style(Style::default().add_modifier(Modifier::BOLD)
                             .bg(Color::Blue).fg(Color::Black));
        let name_column = List::new(name_list)
            .highlight_style(Style::default().add_modifier(Modifier::BOLD)
                             .bg(Color::Blue).fg(Color::Black));
        let status_column = List::new(status_list)
            .highlight_style(Style::default().add_modifier(Modifier::BOLD)
                             .bg(Color::Blue).fg(Color::Black));
        let time_column = List::new(time_list)
            .highlight_style(Style::default().add_modifier(Modifier::BOLD)
                             .bg(Color::Blue).fg(Color::Black));
        let partition_column = List::new(partition_list)
            .highlight_style(Style::default().add_modifier(Modifier::BOLD)
                             .bg(Color::Blue).fg(Color::Black));
        let nodes_column = List::new(nodes_list)
            .highlight_style(Style::default().add_modifier(Modifier::BOLD)
                             .bg(Color::Blue).fg(Color::Black));

        // create the list state
        let mut state = ListState::default();
        state.select(Some(self.index));

        f.render_stateful_widget(id_column, list_layout[0], &mut state);
        f.render_stateful_widget(name_column, list_layout[1], &mut state);
        f.render_stateful_widget(status_column, list_layout[2], &mut state);
        f.render_stateful_widget(time_column, list_layout[3], &mut state);
        f.render_stateful_widget(partition_column, list_layout[4], &mut state);
        f.render_stateful_widget(nodes_column, list_layout[5], &mut state);

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
                self.next_job();
            },
            KeyCode::Up | KeyCode::Char('k') => {
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
        self.index += 1;
        if self.index >= self.joblist.len() {
            self.index = 0;
        }
    }

    fn prev_job(&mut self) {
        if self.index > 0 {
            self.index -= 1;
        } else {
            self.index = self.joblist.len() - 1;
        }
    }
}
