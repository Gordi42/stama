use ratatui::{
    prelude::*,
    style::{Color, Style},
    widgets::*,
    layout::Flex,
};
use crossterm::event::{
    KeyCode, KeyEvent, MouseEventKind, MouseButton,};
use tui_textarea::{TextArea, CursorMove};
use std::process::Command;

use crate::app::Action;
use crate::job::{Job, JobStatus};
use crate::mouse_input::MouseInput;
use crate::user_options::UserOptions;
use crate::update_content::{ContentUpdater};


#[derive(Debug, Clone, PartialEq)]
pub enum WindowFocus {
    JobDetails,
    Log,
}

#[derive(Default)]
pub struct MouseAreas {
    pub joblist_title: Rect,
    pub details_title: Rect,
    pub bottom_symbol: Rect,
    pub log_title: Rect,
    pub joblist: Rect,
    pub categories: Vec<Rect>,
}

#[derive(PartialEq)]
pub enum SortCategory {
    Id,
    Name,
    Status,
    Time,
    Partition,
    Nodes,
}

pub struct JobOverview {
    pub should_render: bool,  // if the window should render
    pub handle_input: bool,   // if the window should handle input
    pub sort_category: SortCategory, // the category to sort 
    pub reversed: bool,       // if the sorting is reversed
    pub collapsed_top: bool,  // if the job list is collapsed
    pub collapsed_bot: bool,  // if the job details are collapsed
    pub focus: WindowFocus,   // which part of the window is in focus
    pub joblist: Vec<Job>,    // the list of jobs
    pub index: usize,         // the index of the job in focus
    pub state: TableState,     // the state of the job list
    pub mouse_areas: MouseAreas, // the mouse areas of the window
    pub squeue_command: TextArea<'static>, // the squeue command
    pub edit_squeue: bool,    // if the squeue command is being edited
    pub refresh_rate: usize,    // the refresh rate of the window
    pub job_details: String,  // the job details
    pub log_height: u16,      // the height of the log section
    pub log: String,          // the log of the job
    pub content_updater: ContentUpdater, // the content update
}

// ====================================================================
//  CONSTRUCTOR
// ====================================================================

impl JobOverview {
    pub fn new(refresh_rate: usize) -> Self {
        let mut state = TableState::default();
        state.select(Some(0));
        // create mouse areas with 6 categories
        let mut mouse_areas = MouseAreas::default();
        for _ in 0..6 {
            mouse_areas.categories.push(Rect::default());
        }
        let command = format!("squeue -u {}", whoami());
        let mut textarea = TextArea::from([command]);
        textarea.move_cursor(CursorMove::End);
        Self {
            should_render: true,
            handle_input: true,
            sort_category: SortCategory::Id,
            reversed: false,
            collapsed_top: false,
            collapsed_bot: true,
            focus: WindowFocus::JobDetails,
            joblist: vec![],
            index: 0,
            state: state,
            mouse_areas: mouse_areas,
            squeue_command: textarea,
            edit_squeue: false,
            refresh_rate: refresh_rate,
            job_details: String::new(),
            log_height: 0,
            log: String::new(),
            content_updater: ContentUpdater::new(),
        }
    }
}

// ====================================================================
//  METHODS
// ====================================================================

impl JobOverview {
    pub fn sort(&mut self) {
        // only sort if there are jobs
        if self.joblist.is_empty() { return; }
        // get the id of the job in focus
        let id = self.joblist[self.index].id.clone();
        // sort the job list
        match self.sort_category {
            SortCategory::Id => {
                self.joblist.sort_by(|a, b| b.id.cmp(&a.id));
            },
            SortCategory::Name => {
                self.joblist.sort_by(|a, b| a.name.cmp(&b.name));
            },
            SortCategory::Status => {
                self.joblist.sort_by(|a, b| 
                                     a.status.priority().cmp(
                                     &b.status.priority()));
            },
            SortCategory::Time => {
                self.joblist.sort_by(|a, b| a.time.cmp(&b.time));
            },
            SortCategory::Partition => {
                self.joblist.sort_by(|a, b| a.partition.cmp(&b.partition));
            },
            SortCategory::Nodes => {
                self.joblist.sort_by(|a, b| b.nodes.cmp(&a.nodes));
            },
        }
        // reverse the list if needed
        if self.reversed {
            self.joblist.reverse();
        }
        // get the index of the job in focus
        let index = self.joblist.iter().position(|job| job.id == id);
        self.set_index_raw(index.unwrap_or(0) as i32);
    }

    pub fn get_job(&self) -> Option<&Job> {
        self.joblist.get(self.index)
    }

    pub fn get_squeue_command(&self) -> String {
        self.squeue_command.lines().join("\n")
    }

    pub fn update_joblist(&mut self, user_options: &UserOptions) {
        let id = match self.joblist.is_empty() {
            true => "no_job".to_string(),
            false => self.joblist[self.index].id.clone(),
        };
        let job = self.get_job().cloned();
        let command = self.get_squeue_command();
        match self.content_updater.tick(
            job, command, user_options.clone()) {
            Some(content) => {
                self.joblist = content.job_list;
                self.job_details = content.details_text;
                self.log = content.log_text;
            }
            None => { }
        }
        // check if there is a job with the same id
        let index = self.joblist.iter().position(|job| job.id == id);
        self.set_index_raw(index.unwrap_or(0) as i32);
        self.sort();
    }

        
}

fn whoami() -> String {
    let command = Command::new("whoami")
        .output();
    match command {
        Ok(output) => {
            let output = String::from_utf8_lossy(&output.stdout);
            output.to_string()
        },
        Err(_) => {
            "Error executing whoami".to_string()
        },
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
        if self.collapsed_top && self.collapsed_bot {
            constraints.push(Constraint::Length(1));
            constraints.push(Constraint::Length(1));
        } else if self.collapsed_top && !self.collapsed_bot {
            constraints.push(Constraint::Length(1));
            constraints.push(Constraint::Min(1));
        } else if !self.collapsed_top && self.collapsed_bot {
            constraints.push(Constraint::Min(1));
            constraints.push(Constraint::Length(1));
        } else {
            constraints.push(Constraint::Percentage(30));
            constraints.push(Constraint::Percentage(70));
        }

        // create a layout for the title
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints.as_slice())
            .split(*area);

        // render the title, job list, and job details
        self.render_title(f, &layout[0]);
        self.render_joblist(f, &layout[1]);
        self.render_bottom_section(f, &layout[2]);
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
        match self.collapsed_top {
            true => self.render_joblist_collapsed(f, area),
            false => self.render_joblist_extended(f, area),
        }
    }

    fn render_joblist_collapsed(&mut self, f: &mut Frame, area: &Rect) {
        // update the mouse areas
        self.mouse_areas.joblist_title = area.clone();
        self.mouse_areas.joblist = Rect::default();

        if self.joblist.is_empty() {
            let title = format!("▶ Job list (collapsed)");
            f.render_widget(Line::from(title), *area);
            return;
        }

        let job = &self.joblist[self.index];
        let col = get_job_color(job);

        let content_strings = vec![
            "▶ Job: ".to_string(),
            job.id.clone(),
            job.name.clone(),
            job.status.to_string(),
            job.time.clone(),
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
        
        // update the mouse areas of the categories
        for category in self.mouse_areas.categories.iter_mut() {
            *category = Rect::default();
        }

        content_strings.iter().enumerate().for_each(|(i, s)| {
            let line = Line::from(s.clone()).
                style(Style::default().fg(col));
            f.render_widget(line, layout[i]);
        });
    }

    fn render_joblist_extended(&mut self, f: &mut Frame, area: &Rect) {
        let title = "▼ Job list: ";
        let title_len = title.len() as u16;

        let refresh_rate = format!(
            "{} ms", self.refresh_rate);
        
        let block = Block::default().title(title)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(block::Title::from(refresh_rate)
                   .alignment(Alignment::Right));

        // update the mouse areas
        let mut top_row = area.clone();
        top_row.height = 1;
        self.mouse_areas.joblist_title = top_row;
        let mut joblist_area = block.inner(*area).clone();

        f.render_widget(block.clone(), *area);


        // render the squeue command
        let buffer = self.get_squeue_command();
        let mut squeue_rect = top_row.clone();
        squeue_rect.width = buffer.len() as u16 + 1;
        squeue_rect.x = title_len - 1;
        self.render_squeue_command(f, &squeue_rect);

        if self.joblist.is_empty() {
            self.render_empty_joblist(f, &joblist_area);
            return;
        }

        // ----------------------------------------------
        //  CREATE THE JOB LIST
        // ----------------------------------------------

        // Create the titles for the columns
        let mut title_names = vec![Span::raw("ID"), 
                                   Span::raw("Name"), 
                                   Span::raw("Status"), 
                                   Span::raw("Time"), 
                                   Span::raw("Partition"), 
                                   Span::raw("Nodes")];
        // modify the title names if the category is selected
        let cat_ind = match self.sort_category {
            SortCategory::Id => 0,
            SortCategory::Name => 1,
            SortCategory::Status => 2,
            SortCategory::Time => 3,
            SortCategory::Partition => 4,
            SortCategory::Nodes => 5,
        };
        let title_string: String = title_names[cat_ind].content.clone().into();
        let new_title = format!("{} {}", 
                           title_string,
                           if self.reversed { "▲" } else { "▼" });
        title_names[cat_ind] = Span::styled(
            new_title, Style::default().fg(Color::Blue));

        // Create the rows for the job list
        let rows = self.joblist.iter().map(|job| {
            Row::new(vec![
                job.id.clone(),
                job.name.clone(),
                job.status.to_string(),
                format_time(job),
                job.partition.clone(),
                job.nodes.to_string(),
            ]).style(Style::default().fg(get_job_color(job)))
        }).collect::<Vec<Row>>();

        // Create the widths for the columns
        let widths = [
            Constraint::Min(8),
            Constraint::Min(10),
            Constraint::Min(8),
            Constraint::Min(6),
            Constraint::Min(11),
            Constraint::Min(7),
        ];

        // set the flex and spacing for the columns

        let flex = Flex::SpaceBetween;
        let column_spacing = 1;

        // get the rects for the columnss and update the mouse areas
        let mut rects = Layout::horizontal(widths)
            .flex(flex)
            .spacing(column_spacing)
            .split(joblist_area);
        // set height of each rect to 1
        rects = rects.iter().map(|rect| {
            let mut r = rect.clone();
            r.height = 1;
            r
        }).collect();
        self.mouse_areas.categories = rects.to_vec();

        // create the table

        let table = Table::new(rows, widths)
            .column_spacing(column_spacing)
            .header(
                Row::new(title_names)
                .style(Style::new().bold())
                )
            .flex(flex) 
            .highlight_style(Style::new().reversed());

        // render the table
        f.render_stateful_widget(table, joblist_area.clone(), &mut self.state);

        // update the mouse areas
        joblist_area.y += 1;       // remove the header row
        joblist_area.height = joblist_area.height.saturating_sub(1);
        self.mouse_areas.joblist = joblist_area;
        return;

    }

    fn render_squeue_command(&mut self, f: &mut Frame, area: &Rect) {
        let textarea = &mut self.squeue_command;
        if self.edit_squeue {
            textarea.set_cursor_style(Style::default().bg(Color::Red));
            textarea.set_cursor_line_style(
                Style::default().fg(Color::Red)
                .add_modifier(Modifier::BOLD));
        } else {
            textarea.set_cursor_line_style(Style::default());
            textarea.set_cursor_style(Style::default());
        }
        f.render_widget(textarea.widget(), *area);
    }

    fn render_empty_joblist(&self, f: &mut Frame, area: &Rect) {
        let text = "No jobs found";
        let text = Span::styled(text, Style::default().fg(Color::Gray));
        let paragraph = Paragraph::new(text)
            .alignment(Alignment::Center);
        f.render_widget(paragraph, *area);
    }

    // ----------------------------------------------------------------------
    // RENDERING THE JOB DETAILS AND LOG SECTION
    // ----------------------------------------------------------------------
    fn render_bottom_section(&mut self, f: &mut Frame, area: &Rect) {
        self.log_height = area.height.saturating_sub(2);
        match self.collapsed_bot {
            true => self.render_bottom_collapsed(f, area),
            false => self.render_bottom_extended(f, area),
        }
    }

    fn render_bottom_collapsed(&mut self, f: &mut Frame, area: &Rect) {
        let title = vec!{
            Span::raw("▶ "),
            Span::raw("1. Job details"), 
            Span::raw("  "),
            Span::raw("2. Log"),
        };

        // update the mouse areas
        self.update_bottom_mouse_positions(area, title.clone(), 0);

        let line = Line::from(title).
            style(Style::default().fg(Color::Gray));
        f.render_widget(line, *area);
    }

    fn render_bottom_extended(&mut self, f: &mut Frame, area: &Rect) {
        let mut title = vec!{
            Span::raw("▼ "),
            Span::raw("1. Job details"), 
            Span::raw("  "),
            Span::raw("2. Log"),
        };

        // update the mouse areas
        self.update_bottom_mouse_positions(area, title.clone(), 1);

        match self.focus {
            WindowFocus::JobDetails => {
                title[1] = Span::styled("1. Job details", 
                                        Style::default().fg(Color::Blue));
            },
            WindowFocus::Log => {
                title[3] = Span::styled("2. Log", 
                                        Style::default().fg(Color::Blue));
            },
        }
        
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded);

        f.render_widget(block.clone(), *area);
        let rect = block.inner(*area);
        match self.focus {
            WindowFocus::JobDetails => {
                self.render_job_details(f, &rect);
            },
            WindowFocus::Log => {
                self.render_log(f, &rect);
            },
        }
    }

    fn render_job_details(&self, f: &mut Frame, area: &Rect) {

        let paragraph = Paragraph::new(self.job_details.clone())
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: true });

        f.render_widget(paragraph, *area);
    }

    fn render_log(&self, f: &mut Frame, area: &Rect) {
        let mut paragraph = Paragraph::new(self.log.clone())
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: true });

        // calculate the scroll offset such that the last line is visible
        let lines = paragraph.line_count(area.width);
        let offset = (lines as u16).saturating_sub(self.log_height);
        paragraph = paragraph.scroll((offset, 0));

        f.render_widget(paragraph, *area);
    }

    fn update_bottom_mouse_positions(
        &mut self, area: &Rect, title: Vec<Span>, offset: u16) {
        if title.len() != 4 { return; }
        let mut top_row = area.clone();
        top_row.height = 1;
        let mut symbol = top_row.clone();
        symbol.width = title[0].width() as u16;
        symbol.x += offset;
        let mut details_title = top_row.clone();
        details_title.width = title[1].width() as u16;
        details_title.x += symbol.width + symbol.x;
        let mut log_title = top_row.clone();
        log_title.width = title[3].width() as u16;
        log_title.x += details_title.width + details_title.x + 2;
        self.mouse_areas.bottom_symbol = symbol;
        self.mouse_areas.details_title = details_title;
        self.mouse_areas.log_title = log_title;
    }

}

fn get_job_color(job: &Job) -> Color {
    match job.status {
        JobStatus::Running => Color::Green,
        JobStatus::Pending => Color::Yellow,
        JobStatus::Completing => Color::Yellow,
        JobStatus::Completed => Color::Gray,
        JobStatus::Failed => Color::Red,
        JobStatus::Timeout => Color::Red,
        JobStatus::Cancelled => Color::Red,
        JobStatus::Unknown => Color::Red,
    }
}


fn format_time(job: &Job) -> String {
    let time_str = job.time.clone();
    
    let parts: Vec<&str> = time_str.split('-').collect();
    match parts.len() {
        1 => {
            parts[0].to_string()
        }
        2 => {
            let days = parts[0].parse::<i32>().unwrap_or(0);
            if days > 0 {
                time_str
            } else {
                parts[1].to_string()
            }
        }
        _ => "".to_string(),
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

        if self.edit_squeue {
            match key_event.code {
                KeyCode::Esc | KeyCode::Enter => {
                    self.edit_squeue = false;
                    return true;
                },
                _ => {
                    self.squeue_command.input(key_event);
                    return true;
                },
            }
        }

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
            // Change sorting category
            KeyCode::Tab => {
                self.next_sort_category();
                *action = Action::SortJobList;
            },
            KeyCode::Char('r') => {
                self.reversed = !self.reversed;
                *action = Action::SortJobList;
            },
            // Switching focus between job details and log
            KeyCode::Char('1') => {
                self.select_details();
            },
            KeyCode::Char('2') => {
                self.select_log();
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
            KeyCode::Char('o') => {
                *action = Action::OpenUserOptions;
            },
            // Collapsing/Extending the joblist
            KeyCode::Char('m') => {
                self.collapsed_top = !self.collapsed_top;
            },
            KeyCode::Char('n') => {
                self.collapsed_bot = !self.collapsed_bot;
            },
            // Edit the squeue command
            KeyCode::Char('/') => {
                self.collapsed_top = false;
                self.edit_squeue = true;
            },
            _ => {return false;},
        };
        true
    }

    fn select_details(&mut self) {
        // if the job details are already in focus, toggle collapse
        if self.focus == WindowFocus::JobDetails {
            self.collapsed_bot = !self.collapsed_bot;
        } else {
            self.focus = WindowFocus::JobDetails;
            self.collapsed_bot = false;
        }
    }

    fn select_log(&mut self) {
        // if the log is already in focus, toggle collapse
        if self.focus == WindowFocus::Log {
            self.collapsed_bot = !self.collapsed_bot;
        } else {
            self.focus = WindowFocus::Log;
            self.collapsed_bot = false;
        }
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

    pub fn set_index_raw(&mut self, index: i32) {
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

    pub fn set_index(&mut self, index: i32) {
        let job_len = self.joblist.len() as i32;
        let mut new_index = index;
        if index >= job_len {
            new_index = 0;
        } else if index < 0 {
            new_index = job_len - 1;
        } 
        self.index = new_index as usize;
        self.state.select(Some(self.index));
        self.job_details = "loading...".to_string();
        self.log = "loading...".to_string();
    }

    fn next_sort_category(&mut self) {
        match self.sort_category {
            SortCategory::Id => {
                self.sort_category = SortCategory::Name;
            },
            SortCategory::Name => {
                self.sort_category = SortCategory::Status;
            },
            SortCategory::Status => {
                self.sort_category = SortCategory::Time;
            },
            SortCategory::Time => {
                self.sort_category = SortCategory::Partition;
            },
            SortCategory::Partition => {
                self.sort_category = SortCategory::Nodes;
            },
            SortCategory::Nodes => {
                self.sort_category = SortCategory::Id;
            },
        }
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
            if self.edit_squeue {
                self.edit_squeue = false;
                return;
            }

            match event_kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    // joblist title
                    if self.mouse_areas.joblist_title.contains(mouse_pos) {
                        self.collapsed_top = !self.collapsed_top;
                        mouse_input.click();
                    }
                    // joblist categories
                    for (i, category) in self.mouse_areas
                                             .categories.iter().enumerate() {
                        if category.contains(mouse_pos) {
                            let new_cat = match i {
                                0 => SortCategory::Id,
                                1 => SortCategory::Name,
                                2 => SortCategory::Status,
                                3 => SortCategory::Time,
                                4 => SortCategory::Partition,
                                5 => SortCategory::Nodes,
                                _ => SortCategory::Id,
                            };
                            // reverse sorting if same category is selected
                            if new_cat == self.sort_category {
                                self.reversed = !self.reversed;
                            } else {
                                self.sort_category = new_cat;
                            };
                            *action = Action::SortJobList;
                            mouse_input.click();
                        }
                    }
                    // joblist entries
                    if self.mouse_areas.joblist.contains(mouse_pos) {
                        let rel_y = mouse_pos.y - self.mouse_areas.joblist.y;
                        let mut new_index = rel_y as i32 + self.state.offset() as i32;
                        new_index = new_index.min(self.joblist.len() as i32 - 1);
                        self.set_index(new_index);
                        if mouse_input.is_double_click() {
                            *action = Action::OpenJobAction;
                        }
                        mouse_input.click();
                    }
                    // collapse symbol
                    if self.mouse_areas.bottom_symbol.contains(mouse_pos) {
                        self.collapsed_bot = !self.collapsed_bot;
                        mouse_input.click();
                    }
                    // details title
                    if self.mouse_areas.details_title.contains(mouse_pos) {
                        self.focus = WindowFocus::JobDetails;
                        self.collapsed_bot = false;
                        mouse_input.click();
                    }
                    // log title
                    if self.mouse_areas.log_title.contains(mouse_pos) {
                        self.focus = WindowFocus::Log;
                        self.collapsed_bot = false;
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

// ====================================================================
//  TESTS
// ====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_time() {
        let mut job = Job::new_default();
        job.time = "0-00:00:10".to_string();
        assert_eq!(format_time(&job), "00:00:10");
        job.time = "1-00:00:10".to_string();
        assert_eq!(format_time(&job), "1-00:00:10");
    }
}


