use ratatui::{
    prelude::*,
    style::{Color, Style},
    widgets::*,
};
use crossterm::event::{KeyCode, KeyEvent, MouseEvent, MouseEventKind};

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
    pub collapsed: bool,      // if the job list is collapsed
    pub focus: WindowFocus,   // which part of the window is in focus
    pub joblist: Vec<Job>,    // the list of jobs
    pub index: usize,         // the index of the job in focus
    pub state: ListState,     // the state of the job list
}

// ====================================================================
//  CONSTRUCTOR
// ====================================================================

impl JobOverview {
    pub fn new() -> Self {
        let mut state = ListState::default();
        state.select(Some(0));
        Self {
            should_render: true,
            handle_input: true,
            search_args: "-U u301533".to_string(),
            collapsed: false,
            focus: WindowFocus::JobDetails,
            joblist: vec![],
            index: 0,
            state: state,
        }
    }
}


// ====================================================================
//  RENDERING
// ====================================================================

impl JobOverview {
    pub fn render(&mut self, f: &mut Frame, area: &Rect) {
        // only render if the window is active
        if !self.should_render { return; }

        let mut constraints = vec![Constraint::Length(1)];
        if self.collapsed {
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

    // ----------------------------------------------------------------------
    // RENDERING THE JOB LIST
    // ----------------------------------------------------------------------

    fn render_joblist(&mut self, f: &mut Frame, area: &Rect) {
        match self.collapsed {
            true => self.render_joblist_collapsed(f, area),
            false => self.render_joblist_extended(f, area),
        }
    }

    fn render_joblist_collapsed(&self, f: &mut Frame, area: &Rect) {
        let job = &self.joblist[self.index];
        let col = get_job_color(job);

        let content_strings = vec![
            "▶ Job: ".to_string(),
            job.id.to_string(),
            job.name.clone(),
            format_status(job),
            format_time(job),
            job.partition.clone(),
            job.nodes.to_string(),
        ];

        let constraints = content_strings.iter()
            .map(|s| Constraint::Min(s.len() as u16 + 2))
            .collect::<Vec<Constraint>>();

        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints::<Vec<Constraint>>(constraints)
            .split(*area);

        content_strings.iter().enumerate().for_each(|(i, s)| {
            let line = Line::from(s.clone()).
                style(Style::default().fg(col));
            f.render_widget(line, layout[i]);
        });
    }

    fn render_joblist_extended(&mut self, f: &mut Frame, area: &Rect) {
        let title = format!("▼ Job list: squeue {}", self.search_args);
        
        let block = Block::default().title(title)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded);

        f.render_widget(block.clone(), *area);

        let id_list   = map2column(&self.joblist, |job| job.id.to_string());
        let name_list = map2column(&self.joblist, |job| job.name.clone());
        let stat_list = map2column(&self.joblist, |job| format_status(job));
        let time_list = map2column(&self.joblist, |job| format_time(job));
        let part_list = map2column(&self.joblist, |job| job.partition.clone());
        let node_list = map2column(&self.joblist, |job| job.nodes.to_string());

        let constraints = [
            Constraint::Min(get_column_width(&id_list,   8)),
            Constraint::Min(get_column_width(&name_list, 10)),
            Constraint::Min(get_column_width(&stat_list, 8)),
            Constraint::Min(get_column_width(&time_list, 6)),
            Constraint::Min(get_column_width(&part_list, 11)),
            Constraint::Min(get_column_width(&node_list, 7))
        ];

        // create a layout for the job list
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints.as_ref())
            .split(block.inner(*area));

        let hc = get_job_color(&self.joblist[self.index]);
        let state = &mut self.state;

        render_row(state, f, &layout[0], "ID", id_list, hc);
        render_row(state, f, &layout[1], "Name", name_list, hc);
        render_row(state, f, &layout[2], "Status", stat_list, hc);
        render_row(state, f, &layout[3], "Time", time_list, hc);
        render_row(state, f, &layout[4], "Partition", part_list, hc);
        render_row(state, f, &layout[5], "Nodes", node_list, hc);
    }

    // ----------------------------------------------------------------------
    // RENDERING THE JOB DETAILS AND LOG SECTION
    // ----------------------------------------------------------------------

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

fn get_job_color(job: &Job) -> Color {
    match job.status {
        JobStatus::Running => Color::Green,
        JobStatus::Pending => Color::Yellow,
        JobStatus::Completed => Color::Gray,
        JobStatus::Failed => Color::Red,
        JobStatus::Unknown => Color::Red,
    }
}

fn format_status(job: &Job) -> String {
    match job.status {
        JobStatus::Unknown => "Unknown",
        JobStatus::Running => "Running",
        JobStatus::Pending => "Pending",
        JobStatus::Completed => "Completed",
        JobStatus::Failed => "Failed",
    }.to_string()
}

fn format_time(job: &Job) -> String {
    let time = job.time;
    let hours = time / 3600;
    let minutes = (time % 3600) / 60;
    let seconds = time % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}

fn get_column_width(column: &Vec<Line>, minimum: u16) -> u16 {
    column.iter()
        .map(|item| item.width()).max().unwrap_or(0)
        .max(minimum.into()) as u16
}

fn map2column<F>(joblist: &Vec<Job>, map_fn: F) -> Vec<Line>
where
F: Fn(&Job) -> String,
{
    joblist.iter()
        .map(|job| {
            Line::styled(
                map_fn(job), 
                Style::default().fg(get_job_color(job)))
                .alignment(Alignment::Right)
        }).collect()
}

fn render_row(state: &mut ListState, 
              f: &mut Frame, 
              area: &Rect, title: &str, 
              content: Vec<Line>,
              highlight_color: Color) {
    // create a bold styled centered title
    let title = Span::styled(title, 
                             Style::default().add_modifier(Modifier::BOLD));
    let title = Paragraph::new(title)
        .alignment(Alignment::Right);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(*area);

    f.render_widget(title, layout[0]);

    let item_list = List::new(content)
        .highlight_style(Style::default().add_modifier(Modifier::BOLD)
                         .bg(highlight_color).fg(Color::Black));

    // align the list to the right


    f.render_stateful_widget(item_list, layout[1], state);
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
            // Collapsing/Extending the joblist
            KeyCode::Char('m') => {
                self.collapsed = !self.collapsed;
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
        self.state.select(Some(self.index));
    }

    fn prev_job(&mut self) {
        if self.index > 0 {
            self.index -= 1;
        } else {
            self.index = self.joblist.len() - 1;
        }
        self.state.select(Some(self.index));
    }
}

// ====================================================================
//  MOUSE INPUT
// ====================================================================

impl JobOverview {
    pub fn mouse_input(&mut self, _action: &Action, _mouse_event: MouseEvent) { 
        if !self.handle_input { return; }

        // check if the mouse event is a scroll event
        if _mouse_event.kind == MouseEventKind::ScrollDown {
            self.next_job();
        } else if _mouse_event.kind == MouseEventKind::ScrollUp {
            self.prev_job();
        }

    }
}
