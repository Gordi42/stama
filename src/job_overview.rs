use ratatui::{
    prelude::*,
    style::{Color, Style},
    widgets::*,
};
use crossterm::event::{
    KeyCode, KeyEvent, MouseEventKind, MouseButton,};

use crate::app::Action;
use crate::job::{Job, JobStatus};
use crate::mouse_input::MouseInput;


pub enum WindowFocus {
    JobDetails,
    Log,
}

#[derive(Default)]
pub struct MouseAreas {
    pub joblist_title: Rect,
    pub details_title: Rect,
    pub log_title: Rect,
    pub joblist: Rect,
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
    pub mouse_areas: MouseAreas, // the mouse areas of the window
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
            mouse_areas: MouseAreas::default(),
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

    fn render_joblist_collapsed(&mut self, f: &mut Frame, area: &Rect) {
        // update the mouse areas
        self.mouse_areas.joblist_title = area.clone();
        self.mouse_areas.joblist = Rect::default();

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

        // update the mouse areas
        let mut top_row = area.clone();
        top_row.height = 1;
        self.mouse_areas.joblist_title = top_row;
        let mut joblist_area = block.inner(*area).clone();
        joblist_area.y += 1;       // remove the header row
        joblist_area.height -= 1;
        self.mouse_areas.joblist = joblist_area;

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
        self.set_index(self.index as i32 + 1);
    }

    fn prev_job(&mut self) {
        self.set_index(self.index as i32 - 1);
    }

    fn set_index(&mut self, index: i32) {
        let job_len = self.joblist.len() as i32;
        let mut new_index = index;
        if index >= job_len {
            new_index = 0;
        } else if index < 0 {
            new_index = job_len - 1;
        } 
        self.index = new_index as usize;
        self.state.select(Some(self.index));
    }
}

// ====================================================================
//  MOUSE INPUT
// ====================================================================

impl JobOverview {
    pub fn mouse_input(
        &mut self, action: &mut Action, mouse_input: &mut MouseInput,) { 

        if !self.handle_input { return; }
        let mouse_pos = mouse_input.get_position();

        if let Some(event_kind) = mouse_input.kind() {
            match event_kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    if self.mouse_areas.joblist_title.contains(mouse_pos) {
                        self.collapsed = !self.collapsed;
                    }
                    if self.mouse_areas.joblist.contains(mouse_pos) {
                        let rel_y = mouse_pos.y - self.mouse_areas.joblist.y;
                        let new_index = rel_y as i32 + self.state.offset() as i32;
                        self.set_index(new_index);
                        if mouse_input.is_double_click() {
                            *action = Action::OpenJobAction;
                        }
                        mouse_input.click();
                    }
                },
                MouseEventKind::ScrollDown => {
                    self.next_job();
                },
                MouseEventKind::ScrollUp => {
                    self.prev_job();
                },
                _ => {},
            }
        }



    }
}

